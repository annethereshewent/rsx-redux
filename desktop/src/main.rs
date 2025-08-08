use std::{env, fs};

use frontend::Frontend;
use objc2::rc::Retained;
use rsx_redux::cpu::CPU;

pub mod frontend;
pub mod renderer;

use objc2_metal::{
    MTLCommandBuffer,
    MTLCommandEncoder,
    MTLCommandQueue,
    MTLRenderPassDescriptor,
    MTLRenderCommandEncoder,
    MTLDrawable
};

fn main() {
    let mut cpu = CPU::new();

    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        panic!("syntax: ./psx-redux <path_to_game>");
    }

    let bios = fs::read("SCPH1001.bin").unwrap();

    cpu.bus.load_bios(bios);

    let mut frontend = Frontend::new();

    let mut has_rendered = false;

    loop {
        let mut command_buffer = frontend.renderer.command_queue.commandBuffer().unwrap();
        let rpd: Retained<MTLRenderPassDescriptor> = unsafe { MTLRenderPassDescriptor::new() };
        let mut encoder = command_buffer.renderCommandEncoderWithDescriptor(&rpd).unwrap();
        let drawable = unsafe { frontend.renderer.metal_layer.nextDrawable().unwrap() };

        while !cpu.bus.gpu.frame_finished {
            cpu.step();

            if cpu.bus.gpu.commands_ready {
                has_rendered = true;
                cpu.bus.gpu.commands_ready = false;

                frontend.renderer.render_polygons(&mut cpu.bus.gpu.polygons, &mut command_buffer, &mut encoder);
            }
        }

        if has_rendered {
            encoder.endEncoding();

            command_buffer.presentDrawable(drawable.as_ref());
            command_buffer.commit();
        }

        cpu.bus.gpu.frame_finished = false;

        frontend.handle_events();
    }
}