use std::{env, fs};

use frontend::Frontend;
use objc2::{rc::Retained, runtime::ProtocolObject};
use rsx_redux::cpu::CPU;

pub mod frontend;
pub mod renderer;

use objc2_metal::{
    MTLClearColor, MTLCommandBuffer, MTLCommandEncoder, MTLCommandQueue, MTLRenderCommandEncoder, MTLRenderPassDescriptor
};
use objc2_quartz_core::CAMetalDrawable;

fn main() {
    let mut cpu = CPU::new();

    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        panic!("syntax: ./psx-redux <path_to_game>");
    }

    let bios = fs::read("SCPH1001.bin").unwrap();

    cpu.bus.load_bios(bios);

    let mut frontend = Frontend::new();

    let mut encoder: Option<Retained<ProtocolObject<dyn MTLRenderCommandEncoder>>> = None;
    let mut drawable: Option<Retained<ProtocolObject<dyn CAMetalDrawable>>> = None;
    let mut command_buffer: Option<Retained<ProtocolObject<dyn MTLCommandBuffer>>> = None;

    loop {
        while !cpu.bus.gpu.frame_finished {
            cpu.step();

            if cpu.bus.gpu.commands_ready {
                cpu.bus.gpu.commands_ready = false;
                if encoder.is_none() {
                    let rpd = unsafe { MTLRenderPassDescriptor::new() };
                    drawable = unsafe { frontend.renderer.metal_layer.nextDrawable() };
                    command_buffer = frontend.renderer.command_queue.commandBuffer();

                    let color_attachment = unsafe { rpd.colorAttachments().objectAtIndexedSubscript(0) };

                    unsafe {
                        color_attachment.setTexture(Some(&drawable.as_ref().unwrap().texture()));
                    }

                    encoder = command_buffer.as_ref().unwrap().renderCommandEncoderWithDescriptor(&rpd);
                }

                if let Some(encoder_ref) = &mut encoder {
                    frontend.renderer.render_polygons(&mut cpu.bus.gpu.polygons, encoder_ref);
                }

            }
        }

        if let (Some(encoder), Some(command_buffer), Some(drawable)) = (encoder.take(), command_buffer.take(), drawable.take()) {
            encoder.endEncoding();
            command_buffer.presentDrawable(drawable.as_ref());
            command_buffer.commit();
        }

        cpu.bus.gpu.frame_finished = false;
        cpu.bus.gpu.cap_fps();

        frontend.handle_events();
    }
}