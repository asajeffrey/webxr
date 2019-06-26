/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Traits to be implemented by backends

use crate::Error;
use crate::Floor;
use crate::Frame;
use crate::Native;
use crate::Session;
use crate::SessionBuilder;
use crate::SessionMode;
use crate::Views;

use euclid::Size2D;
use euclid::TypedRigidTransform3D;

/// A trait for discovering XR devices
pub trait Discovery: 'static + Send {
    fn request_session(&mut self, mode: SessionMode, xr: SessionBuilder) -> Result<Session, Error>;
    fn supports_session(&self, mode: SessionMode) -> bool;
}

/// A trait for using an XR device
pub trait Device {
    /// The transform from native coordinates to the floor.
    fn floor_transform(&self) -> TypedRigidTransform3D<f32, Native, Floor>;

    /// The transforms from viewer coordinates to the eyes.
    fn views(&self) -> Views;

    /// This method should block waiting for the next frame,
    /// and return the information for it.
    fn wait_for_animation_frame(&mut self) -> Frame;

    /// This method should render a GL texture to the device.
    /// While this method is being called, the device has unique access
    /// to the texture.
    fn render_animation_frame(&mut self, texture_id: u32, size: Size2D<i32>);
}
