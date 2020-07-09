/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! An implementation of layer management using surfman

use crate::SurfmanGL;

use euclid::Point2D;
use euclid::Rect;
use euclid::Size2D;

use sparkle::gl;
use sparkle::gl::Gl;
use sparkle::gl::GLuint;

use std::collections::HashMap;

use surfman::Context as SurfmanContext;
use surfman::Device as SurfmanDevice;
use surfman::SurfaceAccess;
use surfman::SurfaceTexture;

use surfman_chains::SwapChain;

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

pub struct GlLayerManager {
    layer_map: HashMap<LayerId, GlLayer>,
    layers: Vec<(ContextId, LayerId)>,
    viewports: Viewports,
}

struct GlLayer {
    size: Size2D<i32, Viewport>,
    color_texture: GLuint,
    depth_stencil_texture: Option<GLuint>,
}

impl GlLayerManager {
    pub fn new(viewports: Viewports) -> GlLayerManager {
        let layer_map = HashMap::new();
        let layers = Vec::new();
        GlLayerManager {
            layers,
	    layer_map,
            viewports,
        }
    }
}

impl LayerManagerAPI<SurfmanGL> for GlLayerManager {
    fn create_layer(
        &mut self,
        device: &mut SurfmanDevice,
        contexts: &mut dyn GLContexts<SurfmanGL>,
        context_id: ContextId,
        init: LayerInit,
    ) -> Result<LayerId, Error> {
        let gl = contexts.bindings(device, context_id).ok_or(Error::NoMatchingDevice)?;
        let size = init.texture_size(&self.viewports);
        let layer_id = LayerId::new();
	// TODO: Treat depth and stencil separately?
	let depth_stencil = match init {
	    LayerInit::WebGLLayer { depth, stencil, .. } => depth | stencil,
	    LayerInit::ProjectionLayer { depth, stencil, .. } => depth | stencil,
	};
	// TODO save/restore the TEXTURE_2D binding
	// Create the color texture
	let color_texture = gl.gen_textures(1)[0];
	gl.bind_texture(gl::TEXTURE_2D, color_texture);
	gl.tex_image_2d(gl::TEXTURE_2D, 0, gl::RGBA as _, size.width, size.height, 0, gl::RGBA, gl::UNSIGNED_BYTE, gl::TexImageSource::Pixels(None));
	// Create the depth/stencil texture
  	let depth_stencil_texture = if depth_stencil {
 	  let depth_stencil_texture = gl.gen_textures(1)[0];
	  gl.bind_texture(gl::TEXTURE_2D, depth_stencil_texture);
	  gl.tex_image_2d(gl::TEXTURE_2D, 0, gl::DEPTH_STENCIL as _, size.width, size.height, 0, gl::DEPTH_STENCIL, gl::UNSIGNED_BYTE, gl::TexImageSource::Pixels(None));
	  Some(depth_stencil_texture)
	} else {
	    None
	};
	// TODO: check for GL errors
	let layer = GlLayer { size, color_texture, depth_stencil_texture };
	self.layer_map.insert(layer_id, layer);
	self.layers.push((context_id, layer_id));
        Ok(layer_id)
    }

    fn destroy_layer(
        &mut self,
        device: &mut SurfmanDevice,
        contexts: &mut dyn GLContexts<SurfmanGL>,
        context_id: ContextId,
        layer_id: LayerId,
    ) {
        let gl = match contexts.bindings(device, context_id) {
	    Some(gl) => gl,
	    None => return,
	};
        let layer = match self.layer_map.remove(&layer_id) {
	    Some(layer) => layer,
	    None => return,
	};
	gl.delete_textures(&[layer.color_texture]);
	if let Some(depth_stencil_texture) = layer.depth_stencil_texture {
	    gl.delete_textures(&[depth_stencil_texture]);
	}
        self.layers.retain(|&ids| ids != (context_id, layer_id));
    }

    fn layers(&self) -> &[(ContextId, LayerId)] {
        &self.layers[..]
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
	        let layer = self.layer_map.get(&layer_id).ok_or(Error::NoMatchingDevice)?;
                let color_texture = layer.color_texture;
                let depth_stencil_texture = layer.depth_stencil_texture;
                let texture_array_index = None;
                let viewport = Rect::from_size(layer.size);
                let sub_image = Some(SubImage {
                    color_texture,
                    depth_stencil_texture,
                    texture_array_index,
                    viewport,
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
            let gl = contexts
                .bindings(device, context_id)
                .ok_or(Error::NoMatchingDevice)?;
            gl.flush();
        }
        Ok(())
    }
}
