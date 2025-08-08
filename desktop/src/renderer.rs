use std::ffi::c_void;

use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_metal::{MTLCommandBuffer, MTLCommandQueue, MTLRenderCommandEncoder};
use objc2_quartz_core::CAMetalLayer;
use rsx_redux::cpu::bus::gpu::Polygon;

pub struct Renderer {
    pub metal_view: *mut c_void,
    pub metal_layer: Retained<CAMetalLayer>,
    pub command_queue: Retained<ProtocolObject<dyn MTLCommandQueue>>
}

impl Renderer {
    pub fn render_polygons(
        &mut self, polygons:
        &mut Vec<Polygon>,
        command_buffer: &mut Retained<ProtocolObject<dyn MTLCommandBuffer>>,
        encoder: &mut Retained<ProtocolObject<dyn MTLRenderCommandEncoder>>
    ) {
        println!("{:x?}", polygons);

        polygons.clear();

    }
}