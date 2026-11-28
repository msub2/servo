/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::ffi::CStr;
use std::num::NonZeroU32;
use std::os::raw::c_void;

use euclid::{Size2D, UnknownUnit};
use glow::{RGBA8, SRGB8_ALPHA8};
use log::warn;
use openxr::opengl::{OpenGL, SessionCreateInfo};
use openxr::{ExtensionSet, FrameStream, FrameWaiter, Graphics, Instance, Session, SystemId};
use surfman::{
    Context as SurfmanContext, Device as SurfmanDevice, EglDevice, Error as SurfmanError,
    lookup_egl_extension,
};
use webxr_api::Error::{self, BackendSpecific};

use crate::openxr::graphics::{GraphicsProvider, GraphicsProviderMethods, SwapchainTexture};

pub type Backend = OpenGL;

impl GraphicsProviderMethods<OpenGL> for GraphicsProvider {
    fn enable_graphics_extensions(exts: &mut ExtensionSet) {
        exts.khr_opengl_enable = true;
        exts.mndx_egl_enable = true;
    }

    fn pick_format(formats: &[u32]) -> u32 {
        // TODO: extract the format from surfman's device and pick a matching
        // valid format based on that. For now, assume that eglChooseConfig will
        // gravitate to RGBA8.
        warn!("Available formats: {:?}", formats);
        for format in formats {
            match *format {
                RGBA8 => return *format,
                SRGB8_ALPHA8 => return *format,
                f => warn!("Backend requested unsupported format {:?}", f),
            }
        }

        panic!("No formats supported amongst {:?}", formats);
    }

    fn create_session(
        instance: &Instance,
        system: SystemId,
        device: &SurfmanDevice,
        context: Option<&SurfmanContext>,
    ) -> Result<(Session<OpenGL>, FrameWaiter, FrameStream<OpenGL>), Error> {
        // FIXME: We should ensure that the OpenXR runtime's texture will be shareable with
        // surfman's surfaces by validating the requirements of the XR runtime and ensuring that
        // the WebGL context can use the same device when `makeXRCompatible` is called.

        let _ = OpenGL::requirements(instance, system)
            .map_err(|_| BackendSpecific("Failed OpenGL requirements".to_string()))?;
        let Some(context) = context else {
            return Err(BackendSpecific(
                "Existing OpenGL context is required to start EGL-backed OpenXR session"
                    .to_string(),
            ));
        };

        let display = device.egl_display();
        let config = device.egl_config(context);
        let context = device.egl_context(context);

        unsafe {
            instance
                .create_session::<OpenGL>(
                    system,
                    &SessionCreateInfo::Egl {
                        display: display as *mut c_void,
                        config: config as *mut c_void,
                        context: context as *mut c_void,
                        get_proc_address,
                    },
                )
                .map_err(|e| Error::BackendSpecific(format!("Instance::create_session {:?}", e)))
        }
    }

    fn swapchain_texture(
        image: <OpenGL as Graphics>::SwapchainImage,
        _device: &SurfmanDevice,
        _context: &mut SurfmanContext,
        _size: &Size2D<i32, UnknownUnit>,
    ) -> Result<SwapchainTexture, SurfmanError> {
        let texture = NonZeroU32::new(image)
            .map(glow::NativeTexture)
            .ok_or(SurfmanError::Failed)?;
        Ok(SwapchainTexture::Gl(texture))
    }
}

unsafe extern "system" fn get_proc_address(name: *const i8) -> Option<unsafe extern "system" fn()> {
    unsafe {
        let proc_addr = lookup_egl_extension(CStr::from_ptr(name));
        (!proc_addr.is_null()).then(|| std::mem::transmute(proc_addr))
    }
}
