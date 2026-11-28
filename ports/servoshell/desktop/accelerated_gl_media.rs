/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::RefMut;

use surfman::{Context, Device};

#[cfg(not(any(
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
)))]
pub(crate) fn setup_gl_accelerated_media(_: RefMut<'_, Device>, _: RefMut<'_, Context>) {}

#[cfg(all(target_os = "linux", not(target_env = "ohos")))]
pub(crate) fn setup_gl_accelerated_media(device: RefMut<'_, Device>, context: RefMut<'_, Context>) {
    use servo::{MediaGlContext, MediaNativeDisplay, Servo};
    use surfman::EglDevice;

    let api = api(&device, &context);
    let media_context = MediaGlContext::Egl(device.egl_context(&context) as usize);

    let media_display = MediaNativeDisplay::Egl(device.egl_display() as usize);
    // TODO: Check on the X11 branch later
    Servo::initialize_gl_accelerated_media(media_display, api, media_context);
}

#[cfg(target_os = "windows")]
pub(crate) fn setup_gl_accelerated_media(device: RefMut<'_, Device>, context: RefMut<'_, Context>) {
    use servo::{MediaGlContext, MediaNativeDisplay, Servo};
    use surfman::EglDevice;

    let api = api(&device, &context);
    let context = MediaGlContext::Egl(device.egl_context(&context) as usize);
    let display = MediaNativeDisplay::Egl(device.egl_display() as usize);
    Servo::initialize_gl_accelerated_media(display, api, context);
}

#[cfg(any(
    all(target_os = "linux", not(target_env = "ohos")),
    target_os = "windows"
))]
fn api(device: &RefMut<Device>, context: &RefMut<Context>) -> servo::MediaGlApi {
    use servo::MediaGlApi;
    use surfman::GLApi;

    let descriptor = device.context_descriptor(context);
    let attributes = device.context_descriptor_attributes(&descriptor);
    let major = attributes.version.major;
    let minor = attributes.version.minor;
    match device.gl_api() {
        GLApi::GL if major >= 3 && minor >= 2 => MediaGlApi::OpenGL3,
        GLApi::GL => MediaGlApi::OpenGL,
        GLApi::GLES if major > 1 => MediaGlApi::Gles2,
        GLApi::GLES => MediaGlApi::Gles1,
    }
}
