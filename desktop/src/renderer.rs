use std::ffi::c_void;

use objc2::rc::Retained;
use objc2_quartz_core::CAMetalLayer;
use rsx_redux::cpu::bus::gpu::Polygon;

pub struct Renderer {
    pub metal_view: *mut c_void,
    pub metal_layer: Retained<CAMetalLayer>
}

impl Renderer {
    pub fn render_polygons(&mut self, polygons: &mut Vec<Polygon>) {
        println!("{:x?}", polygons);

        polygons.clear();
        todo!("not done yet!");
    }
}