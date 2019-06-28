/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The WebGL functionality needed by WebXR.

use crate::Error;
use euclid::Size2D;
use gleam::gl::GLsync;
use gleam::gl::GLuint;
use gleam::gl::Gl;
use std::rc::Rc;

/// A trait to get access a GL texture from a WebGL context.
// TODO: refactor the Servo canvas crate to support sharing this trait.
pub trait WebGLExternalImageApi: Send {
    /// Lock the WebGL context, and get back a texture id, the size of the texture,
    /// and a sync object for the texture.
    fn lock(&self) -> Result<(GLuint, Size2D<i32>, GLsync), Error>;

    /// Unlock the WebGL context.
    fn unlock(&self);
}

/// A factory for building GL objects.
// TODO refactor the Servo canvas crate to support sharing this type
#[derive(Clone)]
pub struct GLFactory(());

impl GLFactory {
    pub fn build(&mut self) -> Rc<dyn Gl> {
        unimplemented!()
    }
}
