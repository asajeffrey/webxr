/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! This crate defines the Rust implementation of WebXR for various devices.

#[cfg(feature = "glwindow")]
pub mod glwindow;

#[cfg(feature = "headless")]
pub mod headless;

#[cfg(feature = "googlevr")]
pub mod googlevr;

#[cfg(feature = "gles")]
mod gles;

#[cfg(all(feature = "googlevr", target_os = "android"))]
pub(crate) mod jni_utils;

#[cfg(feature = "magicleap")]
pub mod magicleap;

#[cfg(feature = "egl")]
mod egl;

#[cfg(feature = "openxr-api")]
pub mod openxr;

pub mod surfman_layer_manager;
pub use surfman_layer_manager::SurfmanGL;
pub use surfman_layer_manager::SurfmanLayerManager;
pub mod gl_layer_manager;
pub use gl_layer_manager::GlLayerManager;
pub type MainThreadRegistry = webxr_api::MainThreadRegistry<surfman_layer_manager::SurfmanGL>;
pub type Discovery = Box<dyn webxr_api::DiscoveryAPI<SurfmanGL>>;
