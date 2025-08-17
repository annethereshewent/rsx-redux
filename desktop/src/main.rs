use std::{env, ffi::c_void, fs::{self, File}, ptr::NonNull};

use frontend::Frontend;
use memmap2::Mmap;
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_core_foundation::CGSize;
use renderer::FbVertex;
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
    MTLWinding,
    MTLPrimitiveType,
    MTLDevice,
    MTLResourceOptions,
    MTLSamplerDescriptor,
    MTLSamplerMinMagFilter,
    MTLSamplerAddressMode
};
use objc2_quartz_core::CAMetalDrawable;

pub const VERTICES: [FbVertex; 4] = [
    FbVertex {
        position: [-1.0, 1.0],
        uv: [0.0, 0.0]
    },
    FbVertex {
        position: [1.0, 1.0],
        uv: [1.0, 0.0]
    },
    FbVertex {
        position: [-1.0, -1.0],
        uv: [0.0, 1.0]
    },
    FbVertex {
        position: [1.0, -1.0],
        uv: [1.0, 1.0]
    }
];


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
    // let mut drawable = unsafe { frontend.renderer.metal_layer.nextDrawable() };
    let mut command_buffer: Option<Retained<ProtocolObject<dyn MTLCommandBuffer>>> = None;

    loop {
        while !cpu.bus.gpu.frame_finished {
            cpu.step();
            if cpu.bus.gpu.commands_ready {
                cpu.bus.gpu.commands_ready = false;
                if !cpu.bus.gpu.gpu_commands.is_empty() {
                    frontend.renderer.process_commands(&mut cpu.bus.gpu);
                }

                if cpu.bus.gpu.polygons.len() > 0 {
                    if encoder.is_none() {
                        let rpd = unsafe { MTLRenderPassDescriptor::new() };
                        command_buffer = frontend.renderer.command_queue.commandBuffer();

                        let color_attachment = unsafe { rpd.colorAttachments().objectAtIndexedSubscript(0) };

                        color_attachment.setLoadAction(MTLLoadAction::Clear);
                        color_attachment.setStoreAction(MTLStoreAction::Store);

                        color_attachment.setClearColor(MTLClearColor { red: 0.0, green: 0.0, blue: 0.0, alpha: 1.0 });
                        color_attachment.setTexture(frontend.renderer.vram_write.as_deref());

                        encoder = command_buffer.as_ref().unwrap().renderCommandEncoderWithDescriptor(&rpd);
                    }

                    if let (Some(encoder_ref), Some(command_buffer)) = (&mut encoder.take(), &mut command_buffer.take()) {
                        encoder_ref.setCullMode(MTLCullMode::None);
                        encoder_ref.setFrontFacingWinding(MTLWinding::Clockwise);

                        let vp = MTLViewport {
                            originX: 0.0, originY: 0.0,
                            width: 1024.0, height: 512.0,
                            znear: 0.0, zfar: 1.0,
                        };

                        encoder_ref.setViewport(vp);

                        // let drawing_area = frontend.renderer.clip_drawing_area(&mut cpu.bus.gpu);
                        // encoder_ref.setScissorRect(drawing_area);

                        frontend.renderer.render_polygons(&mut cpu.bus.gpu, encoder_ref);

                        encoder_ref.endEncoding();
                        command_buffer.commit();
                    }
                }
                if let Some(params) = &cpu.bus.gpu.transfer_params {
                    let halfwords = frontend.renderer.handle_cpu_transfer(params);

                    for halfword in halfwords {
                        cpu.bus.gpu.gpuread_fifo.push_back(halfword);
                    }
                }
            }
        }

        let drawable = unsafe { frontend.renderer.metal_layer.nextDrawable() };

        if let Some(drawable) = &drawable {
            let rpd = unsafe { MTLRenderPassDescriptor::new() };

            command_buffer = frontend.renderer.command_queue.commandBuffer();

            unsafe { frontend.renderer.metal_layer.setDrawableSize(CGSize::new(cpu.bus.gpu.display_width as f64, cpu.bus.gpu.display_height as f64)); }

            let color_attachment = unsafe { rpd.colorAttachments().objectAtIndexedSubscript(0) };

            color_attachment.setLoadAction(MTLLoadAction::Load);
            color_attachment.setStoreAction(MTLStoreAction::Store);

            color_attachment.setClearColor(MTLClearColor { red: 1.0, green: 0.0, blue: 0.0, alpha: 1.0 });
            unsafe {
                color_attachment.setTexture(Some(&drawable.texture()));
            }

            if let Some(command_buffer) = &command_buffer {
                if let Some(draw_encoder) = command_buffer.renderCommandEncoderWithDescriptor(&rpd) {
                    draw_encoder.setCullMode(MTLCullMode::None);
                    draw_encoder.setFrontFacingWinding(MTLWinding::Clockwise);

                    let width = cpu.bus.gpu.display_width as f64;
                    let height = cpu.bus.gpu.display_height as f64;

                    let vp = MTLViewport {
                        originX: 0.0, originY: 0.0,
                        width, height,
                        znear: 0.0, zfar: 1.0,
                    };

                    draw_encoder.setViewport(vp);

                    draw_encoder.setRenderPipelineState(&frontend.renderer.fb_pipeline_state);

                    let byte_len = VERTICES.len() * std::mem::size_of::<FbVertex>();

                    let buffer = unsafe {
                        frontend.renderer.device.newBufferWithBytes_length_options(
                            NonNull::new(
                                VERTICES.as_ptr() as *mut c_void).unwrap(),
                                byte_len,
                                MTLResourceOptions::empty())

                    }.unwrap();

                    unsafe {
                        draw_encoder.setVertexBuffer_offset_atIndex(Some(&buffer), 0, 0);
                        draw_encoder.setFragmentTexture_atIndex(frontend.renderer.vram_write.as_deref(), 0);
                        draw_encoder.drawPrimitives_vertexStart_vertexCount(MTLPrimitiveType::TriangleStrip, 0, 4);
                    }

                    draw_encoder.endEncoding();
                    command_buffer.presentDrawable(drawable.as_ref());
                    command_buffer.commit();
                }
            }
        }

        cpu.bus.gpu.frame_finished = false;
        cpu.bus.gpu.cap_fps();

        frontend.handle_events(&mut cpu);
    }
}