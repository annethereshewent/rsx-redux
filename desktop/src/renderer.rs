use std::{ffi::c_void, ops::Deref, ptr::NonNull};

use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_core_foundation::CGSize;
use objc2_metal::{MTLCommandQueue, MTLDevice, MTLPrimitiveType, MTLRenderCommandEncoder, MTLRenderPipelineState, MTLResourceOptions};
use objc2_quartz_core::CAMetalLayer;
use rsx_redux::cpu::bus::gpu::GPU;
use std::cmp;

pub struct Renderer {
    pub metal_view: *mut c_void,
    pub metal_layer: Retained<CAMetalLayer>,
    pub command_queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    pub device: Retained<ProtocolObject<dyn MTLDevice>>,
    pub pipeline_state: Retained<ProtocolObject<dyn MTLRenderPipelineState>>
}

impl Renderer {
    pub fn render_polygons(
        &mut self,
        gpu: &mut GPU,
        width: f64,
        height: f64,
        encoder: &mut Retained<ProtocolObject<dyn MTLRenderCommandEncoder>>
    ) {
        let drawing_width = gpu.x2 - gpu.x1 + 1;
        let drawing_height = gpu.y2 - gpu.y1 + 1;

        'outer: for polygon in gpu.polygons.drain(..) {
            let mut vertices: Vec<[f32; 8]> = vec![[0.0; 8]; polygon.vertices.len()];
            for i in 0..polygon.vertices.len() {
                let cross_product = GPU::cross_product(&polygon.vertices);
                let v = &polygon.vertices;

                if cross_product == 0 {
                    continue 'outer;
                }

                let min_x = cmp::min(v[0].x, cmp::min(v[1].x, v[2].x));
                let min_y = cmp::min(v[0].y, cmp::min(v[1].y, v[2].y));

                let max_x = cmp::max(v[0].x, cmp::max(v[1].x, v[2].x));
                let max_y = cmp::max(v[0].y, cmp::max(v[1].y, v[2].y));

                if (max_x >= 1024 && min_x >= 1024) || (max_x < 0 && min_x < 0) {
                    continue 'outer;
                }

                if (max_y >= 512 && min_y >= 512) || (max_y < 0 && min_y < 0) {
                    continue 'outer;
                }

                if (max_x - min_x) >= 1024 {
                    continue 'outer;
                }

                if (max_y - min_y) >= 512 {
                    continue 'outer;
                }

                let vertex = &polygon.vertices[i];

                let u = vertex.u.unwrap_or(0);
                let v = vertex.v.unwrap_or(0);

                let normalized_x = (vertex.x as f32 / drawing_width as f32) * 2.0 - 1.0;
                let normalized_y = 1.0 - (vertex.y as f32 / drawing_height as f32) * 2.0;

                let r = vertex.color.r as f32 / 255.0;
                let g = vertex.color.g as f32 / 255.0;
                let b = vertex.color.b as f32 / 255.0;

                vertices[i] = [normalized_x, normalized_y, u as f32, v as f32, r, g, b, 1.0];
            }

            let byte_len = vertices.len() * std::mem::size_of::<[f32; 8]>();
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

            unsafe { encoder.drawPrimitives_vertexStart_vertexCount(primitive_type, 0, vertices.len()) };
        }
    }
}