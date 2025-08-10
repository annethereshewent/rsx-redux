use std::{ffi::c_void, ops::Deref, ptr::NonNull};

use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_metal::{MTLCommandQueue, MTLDevice, MTLPrimitiveType, MTLRenderCommandEncoder, MTLRenderPipelineState, MTLResourceOptions};
use objc2_quartz_core::CAMetalLayer;
use rsx_redux::cpu::bus::gpu::Polygon;

pub struct Renderer {
    pub metal_view: *mut c_void,
    pub metal_layer: Retained<CAMetalLayer>,
    pub command_queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    pub device: Retained<ProtocolObject<dyn MTLDevice>>,
    pub pipeline_state: Retained<ProtocolObject<dyn MTLRenderPipelineState>>
}

impl Renderer {
    pub fn render_polygons(
        &mut self, polygons:
        &mut Vec<Polygon>,
        encoder: &mut Retained<ProtocolObject<dyn MTLRenderCommandEncoder>>
    ) {
        for polygon in polygons.drain(..) {
            let mut vertices: Vec<[f32; 8]> = vec![[0.0; 8]; polygon.vertices.len()];

            for i in 0..polygon.vertices.len() {
                let vertex = &polygon.vertices[i];

                let u = vertex.u.unwrap_or(0);
                let v = vertex.v.unwrap_or(0);

                let normalized_x = (vertex.x as f32 / 640.0) * 2.0 - 1.0;
                let normalized_y = (vertex.y as f32 / 480.0) * 2.0 - 1.0;

                let r = vertex.color.r as f32 / 255.0;
                let g = vertex.color.g as f32 / 255.0;
                let b = vertex.color.b as f32 / 255.0;

                vertices[i] = [normalized_x, normalized_y, u as f32, v as f32, r, g, b, 1.0];
            }

            let byte_len = vertices.len() * std::mem::size_of::<[f32; 8]>();
            let buffer = unsafe { self.device.newBufferWithBytes_length_options(NonNull::new(vertices.as_ptr() as *mut c_void).unwrap(), byte_len, MTLResourceOptions::empty()) }.unwrap();

            unsafe { encoder.setVertexBuffer_offset_atIndex(Some(buffer.deref()), 0, 0) };

            let primitive_type = if vertices.len() == 3 {
                MTLPrimitiveType::Triangle
            } else {
                MTLPrimitiveType::TriangleStrip
            };

            encoder.setRenderPipelineState(&self.pipeline_state);

            unsafe { encoder.drawPrimitives_vertexStart_vertexCount(primitive_type, 0, vertices.len()) };
        }
    }
}