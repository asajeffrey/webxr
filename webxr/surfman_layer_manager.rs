/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! An implementation of layer management using surfman

use crate::gl_utils::GlClearer;

use euclid::Point2D;
use euclid::Rect;
use euclid::Size2D;

use sparkle::gl;
use sparkle::gl::GLuint;
use sparkle::gl::Gl;

use std::collections::HashMap;

use surfman::Context as SurfmanContext;
use surfman::Device as SurfmanDevice;
use surfman::SurfaceAccess;

use surfman_chains::SwapChains;
use surfman_chains::SwapChainsAPI;

use webxr_api::ContextId;
use webxr_api::Error;
use webxr_api::GLContexts;
use webxr_api::GLTypes;
use webxr_api::LayerId;
use webxr_api::LayerInit;
use webxr_api::LayerManagerAPI;
use webxr_api::SubImage;
use webxr_api::SubImages;
use webxr_api::Viewport;
use webxr_api::Viewports;

#[derive(Copy, Clone, Debug)]
pub enum SurfmanGL {}

impl GLTypes for SurfmanGL {
    type Device = SurfmanDevice;
    type Context = SurfmanContext;
    type Bindings = Gl;
}

pub struct SurfmanLayerManager {
    layer_ids: Vec<(ContextId, LayerId)>,
    layers: HashMap<LayerId, SurfmanLayer>,
    swap_chains: SwapChains<LayerId, SurfmanDevice>,
    viewports: Viewports,
    clearer: GlClearer,
}

pub struct SurfmanLayer {
    texture_size: Size2D<i32, Viewport>,
    color_texture: GLuint,
    depth_stencil_texture: Option<GLuint>,
}

impl SurfmanLayerManager {
    pub fn new(
        viewports: Viewports,
        swap_chains: SwapChains<LayerId, SurfmanDevice>,
    ) -> SurfmanLayerManager {
        let layer_ids = Vec::new();
        let layers = HashMap::new();
        let clearer = GlClearer::new();
        SurfmanLayerManager {
            layer_ids,
            layers,
            swap_chains,
            viewports,
            clearer,
        }
    }
}

impl LayerManagerAPI<SurfmanGL> for SurfmanLayerManager {
    fn create_layer(
        &mut self,
        device: &mut SurfmanDevice,
        contexts: &mut dyn GLContexts<SurfmanGL>,
        context_id: ContextId,
        init: LayerInit,
    ) -> Result<LayerId, Error> {
        let gl = contexts
            .bindings(device, context_id)
            .ok_or(Error::NoMatchingDevice)?;
        let texture_size = init.texture_size(&self.viewports);
        let layer_id = LayerId::new();
        let access = SurfaceAccess::GPUOnly;
        let size = texture_size.to_untyped();

        // TODO: save/restore GL state
        let color_texture = gl.gen_textures(1)[0];
        gl.bind_texture(gl::TEXTURE_2D, color_texture);
        gl.tex_image_2d(
            gl::TEXTURE_2D,
            0,
            gl::RGBA as _,
            size.width,
            size.height,
            0,
            gl::RGBA,
            gl::BYTE,
            gl::TexImageSource::Pixels(None),
        );
        debug_assert_eq!(gl.get_error(), gl::NO_ERROR);

        // TODO: Treat depth and stencil separately?
        let has_depth_stencil = match init {
            LayerInit::WebGLLayer { stencil, depth, .. } => stencil | depth,
            LayerInit::ProjectionLayer { stencil, depth, .. } => stencil | depth,
        };
        let depth_stencil_texture = if has_depth_stencil {
            let depth_stencil_texture = gl.gen_textures(1)[0];
            gl.bind_texture(gl::TEXTURE_2D, depth_stencil_texture);
            gl.tex_image_2d(
                gl::TEXTURE_2D,
                0,
                gl::DEPTH24_STENCIL8 as _,
                size.width,
                size.height,
                0,
                gl::DEPTH_STENCIL,
                gl::UNSIGNED_INT_24_8,
                gl::TexImageSource::Pixels(None),
            );
            debug_assert_eq!(gl.get_error(), gl::NO_ERROR);
            Some(depth_stencil_texture)
        } else {
            None
        };

        let context = contexts
            .context(device, context_id)
            .ok_or(Error::NoMatchingDevice)?;
        self.swap_chains
            .create_detached_swap_chain(layer_id, size, device, context, access)
            .map_err(|err| Error::BackendSpecific(format!("{:?}", err)))?;

        let layer = SurfmanLayer {
            texture_size,
            color_texture,
            depth_stencil_texture,
        };
        self.layer_ids.push((context_id, layer_id));
        self.layers.insert(layer_id, layer);
        self.layer_ids.push((context_id, layer_id));

        log::debug!(
            "Created color/depth {:?}/{:?} for layer {:?} ({:?})",
            color_texture,
            depth_stencil_texture,
            layer_id,
            texture_size
        );

        Ok(layer_id)
    }

    fn destroy_layer(
        &mut self,
        device: &mut SurfmanDevice,
        contexts: &mut dyn GLContexts<SurfmanGL>,
        context_id: ContextId,
        layer_id: LayerId,
    ) {
        self.clearer
            .destroy_layer(device, contexts, context_id, layer_id);
        let context = match contexts.context(device, context_id) {
            Some(context) => context,
            None => return,
        };
        self.layer_ids.retain(|&ids| ids != (context_id, layer_id));
        let _ = self.swap_chains.destroy(layer_id, device, context);
        if let Some(layer) = self.layers.remove(&layer_id) {
            let gl = contexts.bindings(device, context_id).unwrap();
            gl.delete_textures(&[layer.color_texture]);
            if let Some(depth_stencil_texture) = layer.depth_stencil_texture {
                gl.delete_textures(&[depth_stencil_texture]);
            }
        }
    }

    fn layers(&self) -> &[(ContextId, LayerId)] {
        &self.layer_ids[..]
    }

    fn begin_frame(
        &mut self,
        device: &mut SurfmanDevice,
        contexts: &mut dyn GLContexts<SurfmanGL>,
        layers: &[(ContextId, LayerId)],
    ) -> Result<Vec<SubImages>, Error> {
        layers
            .iter()
            .map(|&(context_id, layer_id)| {
                let layer = self.layers.get(&layer_id).ok_or(Error::NoMatchingDevice)?;
                let color_texture = layer.color_texture;
                let depth_stencil_texture = layer.depth_stencil_texture;
                let texture_array_index = None;
                let origin = Point2D::new(0, 0);
                let sub_image = Some(SubImage {
                    color_texture,
                    depth_stencil_texture,
                    texture_array_index,
                    viewport: Rect::new(origin, layer.texture_size),
                });
                let view_sub_images = self
                    .viewports
                    .viewports
                    .iter()
                    .map(|&viewport| SubImage {
                        color_texture,
                        depth_stencil_texture,
                        texture_array_index,
                        viewport,
                    })
                    .collect();
                self.clearer.clear(
                    device,
                    contexts,
                    context_id,
                    layer_id,
                    color_texture,
                    depth_stencil_texture,
                );
                Ok(SubImages {
                    layer_id,
                    sub_image,
                    view_sub_images,
                })
            })
            .collect()
    }

    fn end_frame(
        &mut self,
        device: &mut SurfmanDevice,
        contexts: &mut dyn GLContexts<SurfmanGL>,
        layers: &[(ContextId, LayerId)],
    ) -> Result<(), Error> {
        for &(context_id, layer_id) in layers {
            let context = contexts
                .context(device, context_id)
                .ok_or(Error::NoMatchingDevice)?;
            let swap_chain = self
                .swap_chains
                .get(layer_id)
                .ok_or(Error::NoMatchingDevice)?;
            let layer = self.layers.get(&layer_id).ok_or(Error::NoMatchingDevice)?;

            // Save the context's current attached surface
            let old_surface = device
                .unbind_surface_from_context(context)
                .map_err(|err| Error::BackendSpecific(format!("{:?}", err)))?;

            // Attach the swap chain to the context
            swap_chain
                .attach(device, context)
                .map_err(|err| Error::BackendSpecific(format!("{:?}", err)))?;

            // Get the GL state for the blit
            let draw_fbo = device
                .context_surface_info(context)
                .map_err(|err| Error::BackendSpecific(format!("{:?}", err)))?
                .map(|info| info.framebuffer_object)
                .unwrap_or(0);
            let gl = contexts
                .bindings(device, context_id)
                .ok_or(Error::NoMatchingDevice)?;
            let read_fbo = self.clearer.fbo(
                gl,
                layer_id,
                layer.color_texture,
                layer.depth_stencil_texture,
            );
            debug_assert_eq!(gl.get_error(), gl::NO_ERROR);

            // Save the current GL state
            let mut bound_fbos = [0, 0];
            unsafe {
                gl.get_integer_v(gl::DRAW_FRAMEBUFFER_BINDING, &mut bound_fbos[0..]);
                gl.get_integer_v(gl::READ_FRAMEBUFFER_BINDING, &mut bound_fbos[1..]);
            }
            debug_assert_eq!(gl.get_error(), gl::NO_ERROR);

            //TODO: Avoid this blit!
            // Blit the texture into the swap chain
            log::debug!("Binding FBOs {} and {}", draw_fbo, read_fbo);
            gl.bind_framebuffer(gl::DRAW_FRAMEBUFFER, draw_fbo);
            gl.bind_framebuffer(gl::READ_FRAMEBUFFER, read_fbo);
            debug_assert_eq!(
                (
                    gl.get_error(),
                    gl.check_framebuffer_status(gl::DRAW_FRAMEBUFFER),
                    gl.check_framebuffer_status(gl::READ_FRAMEBUFFER),
                ),
                (
                    gl::NO_ERROR,
                    gl::FRAMEBUFFER_COMPLETE,
                    gl::FRAMEBUFFER_COMPLETE
                )
            );
            gl.blit_framebuffer(
                0,
                0,
                layer.texture_size.width,
                layer.texture_size.height,
                0,
                0,
                swap_chain.size().width,
                swap_chain.size().height,
                gl::COLOR_BUFFER_BIT,
                gl::NEAREST,
            );
            debug_assert_eq!(gl.get_error(), gl::NO_ERROR);

            // Restore the GL state
            gl.bind_framebuffer(gl::DRAW_FRAMEBUFFER, bound_fbos[0] as GLuint);
            gl.bind_framebuffer(gl::READ_FRAMEBUFFER, bound_fbos[1] as GLuint);
            debug_assert_eq!(gl.get_error(), gl::NO_ERROR);

            // Restore the attached surface
            let context = contexts
                .context(device, context_id)
                .ok_or(Error::NoMatchingDevice)?;
            swap_chain
                .detach(device, context)
                .map_err(|err| Error::BackendSpecific(format!("{:?}", err)))?;
            if let Some(old_surface) = old_surface {
                device
                    .bind_surface_to_context(context, old_surface)
                    .map_err(|err| Error::BackendSpecific(format!("{:?}", err)))?;
            }
        }
        Ok(())
    }
}
