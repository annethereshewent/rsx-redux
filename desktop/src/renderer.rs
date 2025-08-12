use std::{ffi::c_void, ops::Deref, ptr::NonNull};

use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_metal::{
    MTLCommandQueue, MTLDevice, MTLOrigin, MTLPrimitiveType, MTLRegion, MTLRenderCommandEncoder, MTLRenderPipelineState, MTLResourceOptions, MTLSize, MTLTexture
};
use objc2_quartz_core::CAMetalLayer;
use rsx_redux::cpu::bus::gpu::{TexturePageColors, GPU};
use std::cmp;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MetalVertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
    page: [u32; 2],
    depth: u32,
    clut: [u32; 2]
}

impl MetalVertex {
    pub fn new() -> Self {
        Self {
            position: [0.0; 2],
            uv: [0.0; 2],
            color: [0.0; 4],
            page: [0; 2],
            depth: 0,
            clut: [0; 2]
        }
    }
}

pub struct Renderer {
    pub metal_view: *mut c_void,
    pub metal_layer: Retained<CAMetalLayer>,
    pub command_queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    pub device: Retained<ProtocolObject<dyn MTLDevice>>,
    pub pipeline_state: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    pub texture: Option<Retained<ProtocolObject<dyn MTLTexture>>>
}

impl Renderer {
    pub fn render_polygons(
        &mut self,
        gpu: &mut GPU,
        encoder: &mut Retained<ProtocolObject<dyn MTLRenderCommandEncoder>>
    ) {
        let drawing_width = gpu.x2 - gpu.x1 + 1;
        let drawing_height = gpu.y2 - gpu.y1 + 1;

        for polygon in gpu.polygons.drain(..) {
            let mut vertices: Vec<MetalVertex> = vec![MetalVertex::new(); polygon.vertices.len()];
            let mut fragment_uniform = [false];

            if let Some(_) = polygon.texpage {
                fragment_uniform[0] = true;
                if gpu.vram_dirty {
                    self.upload_vram(&gpu.vram);
                    gpu.vram_dirty = false;
                }
            }

            unsafe { encoder.setFragmentBytes_length_atIndex(NonNull::new(fragment_uniform.as_ptr() as *mut c_void).unwrap() , 1, 1) };

            let cross_product = GPU::cross_product(&polygon.vertices);
            let v = &polygon.vertices;

            if cross_product == 0 {
                continue;
            }

            let min_x = cmp::min(v[0].x, cmp::min(v[1].x, v[2].x));
            let min_y = cmp::min(v[0].y, cmp::min(v[1].y, v[2].y));

            let max_x = cmp::max(v[0].x, cmp::max(v[1].x, v[2].x));
            let max_y = cmp::max(v[0].y, cmp::max(v[1].y, v[2].y));

            if (max_x >= 1024 && min_x >= 1024) || (max_x < 0 && min_x < 0) {
                continue;
            }

            if (max_y >= 512 && min_y >= 512) || (max_y < 0 && min_y < 0) {
                continue;
            }

            if (max_x - min_x) >= 1024 {
                continue;
            }

            if (max_y - min_y) >= 512 {
                continue;
            }

            for i in 0..polygon.vertices.len() {
                let vertex = &polygon.vertices[i];

                let u = vertex.u.unwrap_or(0);
                let v = vertex.v.unwrap_or(0);

                let metal_vert = &mut vertices[i];

                metal_vert.position[0] = (vertex.x as f32 / drawing_width as f32) * 2.0 - 1.0;
                metal_vert.position[1] = 1.0 - (vertex.y as f32 / drawing_height as f32) * 2.0;

                metal_vert.color[0] = vertex.color.r as f32 / 255.0;
                metal_vert.color[1] = vertex.color.g as f32 / 255.0;
                metal_vert.color[2] = vertex.color.b as f32 / 255.0;
                metal_vert.color[3] = vertex.color.a as f32 / 255.0;


                let normalized_u = u as f32;
                let normalized_v = v as f32;

                metal_vert.uv[0] = normalized_u;
                metal_vert.uv[1] = normalized_v;
                if let Some(texpage) = polygon.texpage {
                    metal_vert.clut = [polygon.clut.0, polygon.clut.1];
                    metal_vert.depth = match texpage.texture_page_colors {
                        TexturePageColors::Bit4 => 0,
                        TexturePageColors::Bit8 => 1,
                        TexturePageColors::Bit15 => 2
                    };

                    metal_vert.page = [texpage.x_base as u32 * 64, texpage.y_base1 as u32 * 256];
                }
            }

            let byte_len = vertices.len() * std::mem::size_of::<MetalVertex>();

            let buffer = unsafe {
                self.device.newBufferWithBytes_length_options(
                    NonNull::new(
                        vertices.as_ptr() as *mut c_void).unwrap(),
                        byte_len,
                        MTLResourceOptions::empty())

            }.unwrap();

            unsafe { encoder.setVertexBuffer_offset_atIndex(Some(buffer.deref()), 0, 0) };

            let primitive_type = MTLPrimitiveType::Triangle;

            encoder.setRenderPipelineState(&self.pipeline_state);
            unsafe { encoder.setFragmentTexture_atIndex(self.texture.as_deref(), 0) };

            unsafe { encoder.drawPrimitives_vertexStart_vertexCount(primitive_type, 0, vertices.len()) };
        }
    }

    pub fn upload_vram(&mut self, vram: &[u8]) {
        let bytes_per_row = 2048 as usize;
        let region = MTLRegion {
            origin: MTLOrigin { x: 0, y:0, z: 0},
            size: MTLSize { width: 1024, height: 512, depth: 1}
        };

        println!("uploading vram");

        unsafe {
            if let Some (texture) = &mut self.texture {
                println!("found a texture!");
                texture.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
                    region,
                    0,
                    NonNull::new(vram.as_ptr() as *mut c_void).unwrap(),
                    bytes_per_row
                )
            }
        }
    }
}