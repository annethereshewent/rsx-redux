use std::{env, fs, fs::File};

use frontend::Frontend;
use memmap2::Mmap;
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_core_foundation::CGSize;
use rsx_redux::cpu::CPU;

pub mod frontend;
pub mod renderer;

use objc2_metal::{
    MTLClearColor,
    MTLCommandBuffer,
    MTLCommandEncoder,
    MTLCommandQueue,
    MTLCullMode,
    MTLLoadAction,
    MTLRenderCommandEncoder,
    MTLRenderPassDescriptor,
    MTLStoreAction,
    MTLViewport,
    MTLWinding
};
use objc2_quartz_core::CAMetalDrawable;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        panic!("syntax: ./psx-redux <path_to_game>");
    }

    let file = File::open(&args[1]).unwrap();

    let game_data = unsafe  { Mmap::map(&file).unwrap() };

    let bios = fs::read("SCPH1001.bin").unwrap();

    let mut cpu = CPU::new();
    cpu.bus.load_bios(bios);
    cpu.bus.cdrom.load_game_arm64(game_data);

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

                    unsafe { frontend.renderer.metal_layer.setDrawableSize(CGSize::new(cpu.bus.gpu.display_width as f64, cpu.bus.gpu.display_height as f64)); }

                    let color_attachment = unsafe { rpd.colorAttachments().objectAtIndexedSubscript(0) };

                    unsafe {
                        color_attachment.setTexture(Some(&drawable.as_ref().unwrap().texture()));
                        color_attachment.setLoadAction(MTLLoadAction::Clear);
                        color_attachment.setStoreAction(MTLStoreAction::Store);

                        color_attachment.setClearColor(MTLClearColor { red: 0.0, green: 0.0, blue: 0.0, alpha: 1.0 });
                    }

                    encoder = command_buffer.as_ref().unwrap().renderCommandEncoderWithDescriptor(&rpd);
                }

                if let Some(encoder_ref) = &mut encoder {
                    encoder_ref.setCullMode(MTLCullMode::None);
                    encoder_ref.setFrontFacingWinding(MTLWinding::Clockwise);

                    let width = cpu.bus.gpu.display_width as f64;
                    let height = cpu.bus.gpu.display_height as f64;

                    let vp = MTLViewport {
                        originX: 0.0, originY: 0.0,
                        width, height,
                        znear: 0.0, zfar: 1.0,
                    };
                    encoder_ref.setViewport(vp);
                    frontend.renderer.render_polygons(&mut cpu.bus.gpu, encoder_ref);
                }
            }

            if let (Some(encoder), Some(command_buffer), Some(drawable)) = (encoder.take(), command_buffer.take(), drawable.take()) {
                encoder.endEncoding();
                command_buffer.presentDrawable(drawable.as_ref());
                command_buffer.commit();
            }
        }

        cpu.bus.gpu.frame_finished = false;
        cpu.bus.gpu.cap_fps();

        frontend.handle_events(&mut cpu);
    }
}