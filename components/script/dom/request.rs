/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::rc::Rc;
use std::str::FromStr;

use cssparser::match_ignore_ascii_case;
use dom_struct::dom_struct;
use http::Method as HttpMethod;
use http::header::{HeaderName, HeaderValue};
use http::method::InvalidMethod;
use js::rust::HandleObject;
use net_traits::ReferrerPolicy as MsgReferrerPolicy;
use net_traits::fetch::headers::is_forbidden_method;
use net_traits::request::{
    CacheMode as NetTraitsRequestCache, CredentialsMode as NetTraitsRequestCredentials,
    Destination as NetTraitsRequestDestination, Origin, RedirectMode as NetTraitsRequestRedirect,
    Referrer as NetTraitsRequestReferrer, Request as NetTraitsRequest, RequestBuilder,
    RequestMode as NetTraitsRequestMode, Window,
};
use servo_url::ServoUrl;

use crate::body::{BodyMixin, BodyType, Extractable, consume_body};
use crate::conversions::Convert;
use crate::dom::bindings::cell::DomRefCell;
use crate::dom::bindings::codegen::Bindings::HeadersBinding::{HeadersInit, HeadersMethods};
use crate::dom::bindings::codegen::Bindings::RequestBinding::{
    ReferrerPolicy, RequestCache, RequestCredentials, RequestDestination, RequestInfo, RequestInit,
    RequestMethods, RequestMode, RequestRedirect,
};
use crate::dom::bindings::error::{Error, Fallible};
use crate::dom::bindings::reflector::{DomGlobal, Reflector, reflect_dom_object_with_proto};
use crate::dom::bindings::root::{DomRoot, MutNullableDom};
use crate::dom::bindings::str::{ByteString, DOMString, USVString};
use crate::dom::bindings::trace::RootedTraceableBox;
use crate::dom::globalscope::GlobalScope;
use crate::dom::headers::{Guard, Headers};
use crate::dom::promise::Promise;
use crate::dom::readablestream::ReadableStream;
use crate::script_runtime::CanGc;

#[dom_struct]
pub(crate) struct Request {
    reflector_: Reflector,
    #[no_trace]
    request: DomRefCell<NetTraitsRequest>,
    body_stream: MutNullableDom<ReadableStream>,
    headers: MutNullableDom<Headers>,
}

impl Request {
    fn new_inherited(global: &GlobalScope, url: ServoUrl) -> Request {
        Request {
            reflector_: Reflector::new(),
            request: DomRefCell::new(net_request_from_global(global, url)),
            body_stream: MutNullableDom::new(None),
            headers: Default::default(),
        }
    }

    fn new(
        global: &GlobalScope,
        proto: Option<HandleObject>,
        url: ServoUrl,
        can_gc: CanGc,
    ) -> DomRoot<Request> {
        reflect_dom_object_with_proto(
            Box::new(Request::new_inherited(global, url)),
            global,
            proto,
            can_gc,
        )
    }

    fn from_net_request(
        global: &GlobalScope,
        proto: Option<HandleObject>,
        net_request: NetTraitsRequest,
        can_gc: CanGc,
    ) -> DomRoot<Request> {
        let r = Request::new(global, proto, net_request.current_url(), can_gc);
        *r.request.borrow_mut() = net_request;
        r
    }

    fn clone_from(r: &Request, can_gc: CanGc) -> Fallible<DomRoot<Request>> {
        let req = r.request.borrow();
        let url = req.url();
        let headers_guard = r.Headers(can_gc).get_guard();
        let r_clone = Request::new(&r.global(), None, url, can_gc);
        r_clone.request.borrow_mut().pipeline_id = req.pipeline_id;
        {
            let mut borrowed_r_request = r_clone.request.borrow_mut();
            borrowed_r_request.origin = req.origin.clone();
        }
        *r_clone.request.borrow_mut() = req.clone();
        r_clone
            .Headers(can_gc)
            .copy_from_headers(r.Headers(can_gc))?;
        r_clone.Headers(can_gc).set_guard(headers_guard);
        Ok(r_clone)
    }

    pub(crate) fn get_request(&self) -> NetTraitsRequest {
        self.request.borrow().clone()
    }
}

fn net_request_from_global(global: &GlobalScope, url: ServoUrl) -> NetTraitsRequest {
    RequestBuilder::new(global.webview_id(), url, global.get_referrer())
        .origin(global.get_url().origin())
        .pipeline_id(Some(global.pipeline_id()))
        .https_state(global.get_https_state())
        .insecure_requests_policy(global.insecure_requests_policy())
        .has_trustworthy_ancestor_origin(global.has_trustworthy_ancestor_or_current_origin())
        .build()
}

// https://fetch.spec.whatwg.org/#concept-method-normalize
fn normalize_method(m: &str) -> Result<HttpMethod, InvalidMethod> {
    match_ignore_ascii_case! { m,
        "delete" => return Ok(HttpMethod::DELETE),
        "get" => return Ok(HttpMethod::GET),
        "head" => return Ok(HttpMethod::HEAD),
        "options" => return Ok(HttpMethod::OPTIONS),
        "post" => return Ok(HttpMethod::POST),
        "put" => return Ok(HttpMethod::PUT),
        _ => (),
    }
    debug!("Method: {:?}", m);
    HttpMethod::from_str(m)
}

// https://fetch.spec.whatwg.org/#concept-method
fn is_method(m: &ByteString) -> bool {
    m.as_str().is_some()
}

// https://fetch.spec.whatwg.org/#cors-safelisted-method
fn is_cors_safelisted_method(m: &HttpMethod) -> bool {
    m == HttpMethod::GET || m == HttpMethod::HEAD || m == HttpMethod::POST
}

// https://url.spec.whatwg.org/#include-credentials
fn includes_credentials(input: &ServoUrl) -> bool {
    !input.username().is_empty() || input.password().is_some()
}

// https://fetch.spec.whatwg.org/#concept-body-disturbed
fn request_is_disturbed(input: &Request) -> bool {
    input.is_disturbed()
}

// https://fetch.spec.whatwg.org/#concept-body-locked
fn request_is_locked(input: &Request) -> bool {
    input.is_locked()
}

impl RequestMethods<crate::DomTypeHolder> for Request {
    // https://fetch.spec.whatwg.org/#dom-request
    fn Constructor(
        global: &GlobalScope,
        proto: Option<HandleObject>,
        can_gc: CanGc,
        mut input: RequestInfo,
        init: RootedTraceableBox<RequestInit>,
    ) -> Fallible<DomRoot<Request>> {
        // Step 1
        let temporary_request: NetTraitsRequest;

        // Step 2
        let mut fallback_mode: Option<NetTraitsRequestMode> = None;

        // Step 3
        let base_url = global.api_base_url();

        // Step 4 TODO: "Let signal be null."

        match input {
            // Step 5
            RequestInfo::USVString(USVString(ref usv_string)) => {
                // Step 5.1
                let parsed_url = base_url.join(usv_string);
                // Step 5.2
                if parsed_url.is_err() {
                    return Err(Error::Type("Url could not be parsed".to_string()));
                }
                // Step 5.3
                let url = parsed_url.unwrap();
                if includes_credentials(&url) {
                    return Err(Error::Type("Url includes credentials".to_string()));
                }
                // Step 5.4
                temporary_request = net_request_from_global(global, url);
                // Step 5.5
                fallback_mode = Some(NetTraitsRequestMode::CorsMode);
            },
            // Step 6
            RequestInfo::Request(ref input_request) => {
                // This looks like Step 38
                // TODO do this in the right place to not mask other errors
                if request_is_disturbed(input_request) || request_is_locked(input_request) {
                    return Err(Error::Type("Input is disturbed or locked".to_string()));
                }
                // Step 6.1
                temporary_request = input_request.request.borrow().clone();
                // Step 6.2 TODO: "Set signal to input's signal."
            },
        }

        // Step 7
        // TODO: `entry settings object` is not implemented yet.
        let origin = base_url.origin();

        // Step 8
        let mut window = Window::Client;

        // Step 9
        // TODO: `environment settings object` is not implemented in Servo yet.

        // Step 10
        if !init.window.handle().is_null_or_undefined() {
            return Err(Error::Type("Window is present and is not null".to_string()));
        }

        // Step 11
        if !init.window.handle().is_undefined() {
            window = Window::NoWindow;
        }

        // Step 12
        let mut request: NetTraitsRequest;
        request = net_request_from_global(global, temporary_request.current_url());
        request.method = temporary_request.method;
        request.headers = temporary_request.headers.clone();
        request.unsafe_request = true;
        request.window = window;
        // TODO: `entry settings object` is not implemented in Servo yet.
        request.origin = Origin::Client;
        request.referrer = temporary_request.referrer;
        request.referrer_policy = temporary_request.referrer_policy;
        request.mode = temporary_request.mode;
        request.credentials_mode = temporary_request.credentials_mode;
        request.cache_mode = temporary_request.cache_mode;
        request.redirect_mode = temporary_request.redirect_mode;
        request.integrity_metadata = temporary_request.integrity_metadata;

        // Step 13
        if init.body.is_some() ||
            init.cache.is_some() ||
            init.credentials.is_some() ||
            init.integrity.is_some() ||
            init.headers.is_some() ||
            init.method.is_some() ||
            init.mode.is_some() ||
            init.redirect.is_some() ||
            init.referrer.is_some() ||
            init.referrerPolicy.is_some() ||
            !init.window.handle().is_undefined()
        {
            // Step 13.1
            if request.mode == NetTraitsRequestMode::Navigate {
                request.mode = NetTraitsRequestMode::SameOrigin;
            }
            // Step 13.2 TODO: "Unset request's reload-navigation flag."
            // Step 13.3 TODO: "Unset request's history-navigation flag."
            // Step 13.4
            request.referrer = global.get_referrer();
            // Step 13.5
            request.referrer_policy = MsgReferrerPolicy::EmptyString;
        }

        // Step 14
        if let Some(init_referrer) = init.referrer.as_ref() {
            // Step 14.1
            let referrer = &init_referrer.0;
            // Step 14.2
            if referrer.is_empty() {
                request.referrer = NetTraitsRequestReferrer::NoReferrer;
            } else {
                // Step 14.3.1
                let parsed_referrer = base_url.join(referrer);
                // Step 14.3.2
                if parsed_referrer.is_err() {
                    return Err(Error::Type("Failed to parse referrer url".to_string()));
                }
                // Step 14.3.3
                if let Ok(parsed_referrer) = parsed_referrer {
                    if (parsed_referrer.cannot_be_a_base() &&
                        parsed_referrer.scheme() == "about" &&
                        parsed_referrer.path() == "client") ||
                        parsed_referrer.origin() != origin
                    {
                        request.referrer = global.get_referrer();
                    } else {
                        // Step 14.3.4
                        request.referrer = NetTraitsRequestReferrer::ReferrerUrl(parsed_referrer);
                    }
                }
            }
        }

        // Step 15
        if let Some(init_referrerpolicy) = init.referrerPolicy.as_ref() {
            let init_referrer_policy = (*init_referrerpolicy).convert();
            request.referrer_policy = init_referrer_policy;
        }

        // Step 16
        let mode = init.mode.as_ref().map(|m| (*m).convert()).or(fallback_mode);

        // Step 17
        if let Some(NetTraitsRequestMode::Navigate) = mode {
            return Err(Error::Type("Request mode is Navigate".to_string()));
        }

        // Step 18
        if let Some(m) = mode {
            request.mode = m;
        }

        // Step 19
        if let Some(init_credentials) = init.credentials.as_ref() {
            let credentials = (*init_credentials).convert();
            request.credentials_mode = credentials;
        }

        // Step 20
        if let Some(init_cache) = init.cache.as_ref() {
            let cache = (*init_cache).convert();
            request.cache_mode = cache;
        }

        // Step 21
        if request.cache_mode == NetTraitsRequestCache::OnlyIfCached &&
            request.mode != NetTraitsRequestMode::SameOrigin
        {
            return Err(Error::Type(
                "Cache is 'only-if-cached' and mode is not 'same-origin'".to_string(),
            ));
        }

        // Step 22
        if let Some(init_redirect) = init.redirect.as_ref() {
            let redirect = (*init_redirect).convert();
            request.redirect_mode = redirect;
        }

        // Step 23
        if let Some(init_integrity) = init.integrity.as_ref() {
            let integrity = init_integrity.clone().to_string();
            request.integrity_metadata = integrity;
        }

        // Step 24 TODO: "If init["keepalive"] exists..."

        // Step 25.1
        if let Some(init_method) = init.method.as_ref() {
            if !is_method(init_method) {
                return Err(Error::Type("Method is not a method".to_string()));
            }
            // Step 25.2
            if is_forbidden_method(init_method) {
                return Err(Error::Type("Method is forbidden".to_string()));
            }
            // Step 25.3
            let method = match init_method.as_str() {
                Some(s) => normalize_method(s)
                    .map_err(|e| Error::Type(format!("Method is not valid: {:?}", e)))?,
                None => return Err(Error::Type("Method is not a valid UTF8".to_string())),
            };
            // Step 25.4
            request.method = method;
        }

        // Step 26 TODO: "If init["signal"] exists..."
        // Step 27 TODO: "If init["priority"] exists..."

        // Step 28
        let r = Request::from_net_request(global, proto, request, can_gc);

        // Step 29 TODO: "Set this's signal to new AbortSignal object..."
        // Step 30 TODO: "If signal is not null..."

        // Step 31
        // "or_init" looks unclear here, but it always enters the block since r
        // hasn't had any other way to initialize its headers
        r.headers
            .or_init(|| Headers::for_request(&r.global(), can_gc));

        // Step 33 - but spec says this should only be when non-empty init?
        let headers_copy = init
            .headers
            .as_ref()
            .map(|possible_header| match possible_header {
                HeadersInit::ByteStringSequenceSequence(init_sequence) => {
                    HeadersInit::ByteStringSequenceSequence(init_sequence.clone())
                },
                HeadersInit::ByteStringByteStringRecord(init_map) => {
                    HeadersInit::ByteStringByteStringRecord(init_map.clone())
                },
            });

        // Step 33.3
        // We cannot empty `r.Headers().header_list` because
        // we would undo the Step 25 above.  One alternative is to set
        // `headers_copy` as a deep copy of `r.Headers()`. However,
        // `r.Headers()` is a `DomRoot<T>`, and therefore it is difficult
        // to obtain a mutable reference to `r.Headers()`. Without the
        // mutable reference, we cannot mutate `r.Headers()` to be the
        // deep copied headers in Step 25.

        // Step 32
        if r.request.borrow().mode == NetTraitsRequestMode::NoCors {
            let borrowed_request = r.request.borrow();
            // Step 32.1
            if !is_cors_safelisted_method(&borrowed_request.method) {
                return Err(Error::Type(
                    "The mode is 'no-cors' but the method is not a cors-safelisted method"
                        .to_string(),
                ));
            }
            // Step 32.2
            r.Headers(can_gc).set_guard(Guard::RequestNoCors);
        }

        // Step 33.5
        match headers_copy {
            None => {
                // This is equivalent to the specification's concept of
                // "associated headers list". If an init headers is not given,
                // but an input with headers is given, set request's
                // headers as the input's Headers.
                if let RequestInfo::Request(ref input_request) = input {
                    r.Headers(can_gc)
                        .copy_from_headers(input_request.Headers(can_gc))?;
                }
            },
            Some(headers_copy) => r.Headers(can_gc).fill(Some(headers_copy))?,
        }

        // Step 33.5 depending on how we got here
        // Copy the headers list onto the headers of net_traits::Request
        r.request.borrow_mut().headers = r.Headers(can_gc).get_headers_list();

        // Step 34
        let mut input_body = if let RequestInfo::Request(ref mut input_request) = input {
            let mut input_request_request = input_request.request.borrow_mut();
            input_request_request.body.take()
        } else {
            None
        };

        // Step 35
        if let Some(init_body_option) = init.body.as_ref() {
            if init_body_option.is_some() || input_body.is_some() {
                let req = r.request.borrow();
                let req_method = &req.method;
                match *req_method {
                    HttpMethod::GET => {
                        return Err(Error::Type(
                            "Init's body is non-null, and request method is GET".to_string(),
                        ));
                    },
                    HttpMethod::HEAD => {
                        return Err(Error::Type(
                            "Init's body is non-null, and request method is HEAD".to_string(),
                        ));
                    },
                    _ => {},
                }
            }
        }

        // Step 36-37
        if let Some(Some(ref init_body)) = init.body {
            // Step 37.1 TODO "If init["keepalive"] exists and is true..."

            // Step 37.2
            let mut extracted_body = init_body.extract(global, can_gc)?;

            // Step 37.3
            if let Some(contents) = extracted_body.content_type.take() {
                let ct_header_name = b"Content-Type";
                if !r
                    .Headers(can_gc)
                    .Has(ByteString::new(ct_header_name.to_vec()))
                    .unwrap()
                {
                    let ct_header_val = contents.as_bytes();
                    r.Headers(can_gc).Append(
                        ByteString::new(ct_header_name.to_vec()),
                        ByteString::new(ct_header_val.to_vec()),
                    )?;

                    // Step 37.4
                    // In Servo r.Headers's header list isn't a pointer to
                    // the same actual list as r.request's, and so we need to
                    // append to both lists to keep them in sync.
                    if let Ok(v) = HeaderValue::from_bytes(ct_header_val) {
                        r.request
                            .borrow_mut()
                            .headers
                            .insert(HeaderName::from_bytes(ct_header_name).unwrap(), v);
                    }
                }
            }

            let (net_body, stream) = extracted_body.into_net_request_body();
            r.body_stream.set(Some(&*stream));
            input_body = Some(net_body);
        }

        // Step 38 is done earlier

        // Step 39 "TODO if body is non-null and body's source is null..."
        // This looks like where we need to set the use-preflight flag
        // if the request has a body and nothing else has set the flag.

        // Step 40 is done earlier

        // Step 41
        r.request.borrow_mut().body = input_body;

        // Step 42
        Ok(r)
    }

    // https://fetch.spec.whatwg.org/#dom-request-method
    fn Method(&self) -> ByteString {
        let r = self.request.borrow();
        ByteString::new(r.method.as_ref().as_bytes().into())
    }

    // https://fetch.spec.whatwg.org/#dom-request-url
    fn Url(&self) -> USVString {
        let r = self.request.borrow();
        USVString(r.url_list.first().map_or("", |u| u.as_str()).into())
    }

    // https://fetch.spec.whatwg.org/#dom-request-headers
    fn Headers(&self, can_gc: CanGc) -> DomRoot<Headers> {
        self.headers
            .or_init(|| Headers::new(&self.global(), can_gc))
    }

    // https://fetch.spec.whatwg.org/#dom-request-destination
    fn Destination(&self) -> RequestDestination {
        self.request.borrow().destination.convert()
    }

    // https://fetch.spec.whatwg.org/#dom-request-referrer
    fn Referrer(&self) -> USVString {
        let r = self.request.borrow();
        USVString(match r.referrer {
            NetTraitsRequestReferrer::NoReferrer => String::from(""),
            NetTraitsRequestReferrer::Client(_) => String::from("about:client"),
            NetTraitsRequestReferrer::ReferrerUrl(ref u) => {
                let u_c = u.clone();
                u_c.into_string()
            },
        })
    }

    // https://fetch.spec.whatwg.org/#dom-request-referrerpolicy
    fn ReferrerPolicy(&self) -> ReferrerPolicy {
        self.request.borrow().referrer_policy.convert()
    }

    // https://fetch.spec.whatwg.org/#dom-request-mode
    fn Mode(&self) -> RequestMode {
        self.request.borrow().mode.clone().convert()
    }

    // https://fetch.spec.whatwg.org/#dom-request-credentials
    fn Credentials(&self) -> RequestCredentials {
        let r = self.request.borrow().clone();
        r.credentials_mode.convert()
    }

    // https://fetch.spec.whatwg.org/#dom-request-cache
    fn Cache(&self) -> RequestCache {
        let r = self.request.borrow().clone();
        r.cache_mode.convert()
    }

    // https://fetch.spec.whatwg.org/#dom-request-redirect
    fn Redirect(&self) -> RequestRedirect {
        let r = self.request.borrow().clone();
        r.redirect_mode.convert()
    }

    // https://fetch.spec.whatwg.org/#dom-request-integrity
    fn Integrity(&self) -> DOMString {
        let r = self.request.borrow();
        DOMString::from_string(r.integrity_metadata.clone())
    }

    /// <https://fetch.spec.whatwg.org/#dom-body-body>
    fn GetBody(&self) -> Option<DomRoot<ReadableStream>> {
        self.body()
    }

    // https://fetch.spec.whatwg.org/#dom-body-bodyused
    fn BodyUsed(&self) -> bool {
        self.is_disturbed()
    }

    // https://fetch.spec.whatwg.org/#dom-request-clone
    fn Clone(&self, can_gc: CanGc) -> Fallible<DomRoot<Request>> {
        // Step 1
        if request_is_locked(self) {
            return Err(Error::Type("Request is locked".to_string()));
        }
        if request_is_disturbed(self) {
            return Err(Error::Type("Request is disturbed".to_string()));
        }

        // Step 2
        Request::clone_from(self, can_gc)
    }

    // https://fetch.spec.whatwg.org/#dom-body-text
    fn Text(&self, can_gc: CanGc) -> Rc<Promise> {
        consume_body(self, BodyType::Text, can_gc)
    }

    // https://fetch.spec.whatwg.org/#dom-body-blob
    fn Blob(&self, can_gc: CanGc) -> Rc<Promise> {
        consume_body(self, BodyType::Blob, can_gc)
    }

    // https://fetch.spec.whatwg.org/#dom-body-formdata
    fn FormData(&self, can_gc: CanGc) -> Rc<Promise> {
        consume_body(self, BodyType::FormData, can_gc)
    }

    // https://fetch.spec.whatwg.org/#dom-body-json
    fn Json(&self, can_gc: CanGc) -> Rc<Promise> {
        consume_body(self, BodyType::Json, can_gc)
    }

    // https://fetch.spec.whatwg.org/#dom-body-arraybuffer
    fn ArrayBuffer(&self, can_gc: CanGc) -> Rc<Promise> {
        consume_body(self, BodyType::ArrayBuffer, can_gc)
    }

    /// <https://fetch.spec.whatwg.org/#dom-body-bytes>
    fn Bytes(&self, can_gc: CanGc) -> std::rc::Rc<Promise> {
        consume_body(self, BodyType::Bytes, can_gc)
    }
}

impl BodyMixin for Request {
    fn is_disturbed(&self) -> bool {
        let body_stream = self.body_stream.get();
        body_stream
            .as_ref()
            .is_some_and(|stream| stream.is_disturbed())
    }

    fn is_locked(&self) -> bool {
        let body_stream = self.body_stream.get();
        body_stream.is_some_and(|stream| stream.is_locked())
    }

    fn body(&self) -> Option<DomRoot<ReadableStream>> {
        self.body_stream.get()
    }

    fn get_mime_type(&self, can_gc: CanGc) -> Vec<u8> {
        let headers = self.Headers(can_gc);
        headers.extract_mime_type()
    }
}

impl Convert<NetTraitsRequestCache> for RequestCache {
    fn convert(self) -> NetTraitsRequestCache {
        match self {
            RequestCache::Default => NetTraitsRequestCache::Default,
            RequestCache::No_store => NetTraitsRequestCache::NoStore,
            RequestCache::Reload => NetTraitsRequestCache::Reload,
            RequestCache::No_cache => NetTraitsRequestCache::NoCache,
            RequestCache::Force_cache => NetTraitsRequestCache::ForceCache,
            RequestCache::Only_if_cached => NetTraitsRequestCache::OnlyIfCached,
        }
    }
}

impl Convert<RequestCache> for NetTraitsRequestCache {
    fn convert(self) -> RequestCache {
        match self {
            NetTraitsRequestCache::Default => RequestCache::Default,
            NetTraitsRequestCache::NoStore => RequestCache::No_store,
            NetTraitsRequestCache::Reload => RequestCache::Reload,
            NetTraitsRequestCache::NoCache => RequestCache::No_cache,
            NetTraitsRequestCache::ForceCache => RequestCache::Force_cache,
            NetTraitsRequestCache::OnlyIfCached => RequestCache::Only_if_cached,
        }
    }
}

impl Convert<NetTraitsRequestCredentials> for RequestCredentials {
    fn convert(self) -> NetTraitsRequestCredentials {
        match self {
            RequestCredentials::Omit => NetTraitsRequestCredentials::Omit,
            RequestCredentials::Same_origin => NetTraitsRequestCredentials::CredentialsSameOrigin,
            RequestCredentials::Include => NetTraitsRequestCredentials::Include,
        }
    }
}

impl Convert<RequestCredentials> for NetTraitsRequestCredentials {
    fn convert(self) -> RequestCredentials {
        match self {
            NetTraitsRequestCredentials::Omit => RequestCredentials::Omit,
            NetTraitsRequestCredentials::CredentialsSameOrigin => RequestCredentials::Same_origin,
            NetTraitsRequestCredentials::Include => RequestCredentials::Include,
        }
    }
}

impl Convert<NetTraitsRequestDestination> for RequestDestination {
    fn convert(self) -> NetTraitsRequestDestination {
        match self {
            RequestDestination::_empty => NetTraitsRequestDestination::None,
            RequestDestination::Audio => NetTraitsRequestDestination::Audio,
            RequestDestination::Document => NetTraitsRequestDestination::Document,
            RequestDestination::Embed => NetTraitsRequestDestination::Embed,
            RequestDestination::Font => NetTraitsRequestDestination::Font,
            RequestDestination::Frame => NetTraitsRequestDestination::Frame,
            RequestDestination::Iframe => NetTraitsRequestDestination::IFrame,
            RequestDestination::Image => NetTraitsRequestDestination::Image,
            RequestDestination::Manifest => NetTraitsRequestDestination::Manifest,
            RequestDestination::Json => NetTraitsRequestDestination::Json,
            RequestDestination::Object => NetTraitsRequestDestination::Object,
            RequestDestination::Report => NetTraitsRequestDestination::Report,
            RequestDestination::Script => NetTraitsRequestDestination::Script,
            RequestDestination::Sharedworker => NetTraitsRequestDestination::SharedWorker,
            RequestDestination::Style => NetTraitsRequestDestination::Style,
            RequestDestination::Track => NetTraitsRequestDestination::Track,
            RequestDestination::Video => NetTraitsRequestDestination::Video,
            RequestDestination::Worker => NetTraitsRequestDestination::Worker,
            RequestDestination::Xslt => NetTraitsRequestDestination::Xslt,
        }
    }
}

impl Convert<RequestDestination> for NetTraitsRequestDestination {
    fn convert(self) -> RequestDestination {
        match self {
            NetTraitsRequestDestination::None => RequestDestination::_empty,
            NetTraitsRequestDestination::Audio => RequestDestination::Audio,
            NetTraitsRequestDestination::Document => RequestDestination::Document,
            NetTraitsRequestDestination::Embed => RequestDestination::Embed,
            NetTraitsRequestDestination::Font => RequestDestination::Font,
            NetTraitsRequestDestination::Frame => RequestDestination::Frame,
            NetTraitsRequestDestination::IFrame => RequestDestination::Iframe,
            NetTraitsRequestDestination::Image => RequestDestination::Image,
            NetTraitsRequestDestination::Manifest => RequestDestination::Manifest,
            NetTraitsRequestDestination::Json => RequestDestination::Json,
            NetTraitsRequestDestination::Object => RequestDestination::Object,
            NetTraitsRequestDestination::Report => RequestDestination::Report,
            NetTraitsRequestDestination::Script => RequestDestination::Script,
            NetTraitsRequestDestination::ServiceWorker |
            NetTraitsRequestDestination::AudioWorklet |
            NetTraitsRequestDestination::PaintWorklet => {
                panic!("ServiceWorker request destination should not be exposed to DOM")
            },
            NetTraitsRequestDestination::SharedWorker => RequestDestination::Sharedworker,
            NetTraitsRequestDestination::Style => RequestDestination::Style,
            NetTraitsRequestDestination::Track => RequestDestination::Track,
            NetTraitsRequestDestination::Video => RequestDestination::Video,
            NetTraitsRequestDestination::Worker => RequestDestination::Worker,
            NetTraitsRequestDestination::Xslt => RequestDestination::Xslt,
            NetTraitsRequestDestination::WebIdentity => RequestDestination::_empty,
        }
    }
}

impl Convert<NetTraitsRequestMode> for RequestMode {
    fn convert(self) -> NetTraitsRequestMode {
        match self {
            RequestMode::Navigate => NetTraitsRequestMode::Navigate,
            RequestMode::Same_origin => NetTraitsRequestMode::SameOrigin,
            RequestMode::No_cors => NetTraitsRequestMode::NoCors,
            RequestMode::Cors => NetTraitsRequestMode::CorsMode,
        }
    }
}

impl Convert<RequestMode> for NetTraitsRequestMode {
    fn convert(self) -> RequestMode {
        match self {
            NetTraitsRequestMode::Navigate => RequestMode::Navigate,
            NetTraitsRequestMode::SameOrigin => RequestMode::Same_origin,
            NetTraitsRequestMode::NoCors => RequestMode::No_cors,
            NetTraitsRequestMode::CorsMode => RequestMode::Cors,
            NetTraitsRequestMode::WebSocket { .. } => {
                unreachable!("Websocket request mode should never be exposed to Dom")
            },
        }
    }
}

impl Convert<MsgReferrerPolicy> for ReferrerPolicy {
    fn convert(self) -> MsgReferrerPolicy {
        match self {
            ReferrerPolicy::_empty => MsgReferrerPolicy::EmptyString,
            ReferrerPolicy::No_referrer => MsgReferrerPolicy::NoReferrer,
            ReferrerPolicy::No_referrer_when_downgrade => {
                MsgReferrerPolicy::NoReferrerWhenDowngrade
            },
            ReferrerPolicy::Origin => MsgReferrerPolicy::Origin,
            ReferrerPolicy::Origin_when_cross_origin => MsgReferrerPolicy::OriginWhenCrossOrigin,
            ReferrerPolicy::Unsafe_url => MsgReferrerPolicy::UnsafeUrl,
            ReferrerPolicy::Same_origin => MsgReferrerPolicy::SameOrigin,
            ReferrerPolicy::Strict_origin => MsgReferrerPolicy::StrictOrigin,
            ReferrerPolicy::Strict_origin_when_cross_origin => {
                MsgReferrerPolicy::StrictOriginWhenCrossOrigin
            },
        }
    }
}

impl Convert<ReferrerPolicy> for MsgReferrerPolicy {
    fn convert(self) -> ReferrerPolicy {
        match self {
            MsgReferrerPolicy::EmptyString => ReferrerPolicy::_empty,
            MsgReferrerPolicy::NoReferrer => ReferrerPolicy::No_referrer,
            MsgReferrerPolicy::NoReferrerWhenDowngrade => {
                ReferrerPolicy::No_referrer_when_downgrade
            },
            MsgReferrerPolicy::Origin => ReferrerPolicy::Origin,
            MsgReferrerPolicy::OriginWhenCrossOrigin => ReferrerPolicy::Origin_when_cross_origin,
            MsgReferrerPolicy::UnsafeUrl => ReferrerPolicy::Unsafe_url,
            MsgReferrerPolicy::SameOrigin => ReferrerPolicy::Same_origin,
            MsgReferrerPolicy::StrictOrigin => ReferrerPolicy::Strict_origin,
            MsgReferrerPolicy::StrictOriginWhenCrossOrigin => {
                ReferrerPolicy::Strict_origin_when_cross_origin
            },
        }
    }
}

impl Convert<NetTraitsRequestRedirect> for RequestRedirect {
    fn convert(self) -> NetTraitsRequestRedirect {
        match self {
            RequestRedirect::Follow => NetTraitsRequestRedirect::Follow,
            RequestRedirect::Error => NetTraitsRequestRedirect::Error,
            RequestRedirect::Manual => NetTraitsRequestRedirect::Manual,
        }
    }
}

impl Convert<RequestRedirect> for NetTraitsRequestRedirect {
    fn convert(self) -> RequestRedirect {
        match self {
            NetTraitsRequestRedirect::Follow => RequestRedirect::Follow,
            NetTraitsRequestRedirect::Error => RequestRedirect::Error,
            NetTraitsRequestRedirect::Manual => RequestRedirect::Manual,
        }
    }
}
