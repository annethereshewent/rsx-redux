use std::{ffi::c_void, ops::Deref, ptr::NonNull};

use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_core_foundation::CGSize;
use objc2_metal::{
    MTLCommandEncoder,
    MTLCommandBuffer,
    MTLCommandQueue,
    MTLDevice,
    MTLOrigin,
    MTLPrimitiveType,
    MTLRegion,
    MTLRenderCommandEncoder,
    MTLRenderPipelineState,
    MTLResourceOptions,
    MTLSize,
    MTLTexture,
    MTLBlitCommandEncoder,
    MTLTextureDescriptor,
    MTLPixelFormat,
    MTLTextureUsage,
    MTLStorageMode,
    MTLScissorRect,
    MTLRenderPassDescriptor,
    MTLClearColor,
    MTLStoreAction,
    MTLLoadAction,
    MTLCullMode,
    MTLWinding,
    MTLViewport,
    MTLBuffer

};
use objc2_quartz_core::{CAMetalLayer, CAMetalDrawable};
use rsx_redux::cpu::bus::gpu::{
    CPUTransferParams,
    GPUCommand,
    TexturePageColors,
    GPU
};
use std::cmp;

use crate::frontend::{VRAM_HEIGHT, VRAM_WIDTH};

pub const BYTE_LEN: usize = 4 * std::mem::size_of::<FbVertex>();

#[repr(C)]
#[derive(Debug)]
struct FragmentUniform {
    has_texture: bool,
    texture_mask_x: u32,
    texture_mask_y: u32,
    texture_offset_x: u32,
    texture_offset_y: u32
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct MetalVertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
    page: [u32; 2],
    depth: u32,
    _padding: u32,
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
            _padding: 0,
            clut: [0; 2]
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct FbVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2]
}

pub struct Renderer {
    pub metal_view: *mut c_void,
    pub metal_layer: Retained<CAMetalLayer>,
    pub command_queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    pub device: Retained<ProtocolObject<dyn MTLDevice>>,
    pub pipeline_state: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    pub fb_pipeline_state: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    pub vram_read: Option<Retained<ProtocolObject<dyn MTLTexture>>>,
    pub vram_write: Option<Retained<ProtocolObject<dyn MTLTexture>>>,
    pub encoder: Option<Retained<ProtocolObject<dyn MTLRenderCommandEncoder>>>,
    pub command_buffer: Option<Retained<ProtocolObject<dyn MTLCommandBuffer>>>,
    pub vertices: [FbVertex; 4],
    pub buffer: Retained<ProtocolObject<dyn MTLBuffer>>
}

impl Renderer {
    pub fn render_polygons(
        &mut self,
        gpu: &mut GPU
    ) {
        for polygon in gpu.polygons.drain(..) {
            let mut vertices: Vec<MetalVertex> = vec![MetalVertex::new(); polygon.vertices.len()];
            let mut fragment_uniform = FragmentUniform {
                has_texture: false,
                texture_mask_x: gpu.texture_window_mask_x,
                texture_mask_y: gpu.texture_window_mask_y,
                texture_offset_x: gpu.texture_window_offset_x,
                texture_offset_y: gpu.texture_window_offset_y
            };

            if let Some(_) = polygon.texpage {
                fragment_uniform.has_texture = true;
            }

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

            if (max_x - min_x) >= 1024 || (max_y - min_y) >= 512 {
                continue;
            }


            for i in 0..polygon.vertices.len() {
                let vertex = &polygon.vertices[i];

                let u = vertex.u;
                let v = vertex.v;

                let metal_vert = &mut vertices[i];

                metal_vert.position[0] = (vertex.x as f32 / VRAM_WIDTH as f32) * 2.0 - 1.0;
                metal_vert.position[1] = 1.0 - (vertex.y as f32 / VRAM_HEIGHT as f32) * 2.0;

                metal_vert.color[0] = vertex.color.r as f32 / 255.0;
                metal_vert.color[1] = vertex.color.g as f32 / 255.0;
                metal_vert.color[2] = vertex.color.b as f32 / 255.0;
                metal_vert.color[3] = vertex.color.a as f32 / 255.0;

                let u_f32 = u as f32;
                let v_f32 = v as f32;

                metal_vert.uv[0] = u_f32;
                metal_vert.uv[1] = v_f32;
                if let Some(texpage) = polygon.texpage {
                    metal_vert.clut = [polygon.clut.0, polygon.clut.1];
                    metal_vert.depth = match texpage.texture_page_colors {
                        TexturePageColors::Bit4 => 0,
                        _ => todo!("8 bit and 15 bit color depth")
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

            if let Some(encoder) = &self.encoder {
                unsafe { encoder.setFragmentBytes_length_atIndex(NonNull::new(&mut fragment_uniform as *mut _ as *mut c_void).unwrap() , 1, 1) };
                unsafe { encoder.setVertexBuffer_offset_atIndex(Some(buffer.deref()), 0, 0) };

                let primitive_type = MTLPrimitiveType::Triangle;

                encoder.setRenderPipelineState(&self.pipeline_state);

                unsafe { encoder.setFragmentTexture_atIndex(self.vram_read.as_deref(), 0) };
                unsafe { encoder.drawPrimitives_vertexStart_vertexCount(primitive_type, 0, vertices.len()) };
            }
        }
    }

    pub fn process_commands(&mut self, gpu: &mut GPU) {
        for command in gpu.gpu_commands.drain(..) {
            match command {
                GPUCommand::CPUtoVram(params) => {
                    let mut rgba8_buffer: Vec<u8> = Vec::new();

                    let mut i = 0;
                    for _ in 0..params.height {
                        for _ in  0..params.width {
                            let halfword = params.halfwords[i];

                            let mut r = halfword & 0x1f;
                            let mut g = (halfword >> 5) & 0x1f;
                            let mut b = (halfword >> 10) & 0x1f;

                            let a = if halfword == 0 {
                                0
                            } else {
                                0xff
                            };

                            r = r << 3 | r >> 2;
                            g = g << 3 | g >> 2;
                            b = b << 3 | b >> 2;

                            rgba8_buffer.push(r as u8);
                            rgba8_buffer.push(g as u8);
                            rgba8_buffer.push(b as u8);
                            rgba8_buffer.push(a as u8);

                            i += 1;
                        }
                    }

                    let region = MTLRegion {
                        origin: MTLOrigin { x: params.start_x as usize, y: params.start_y as usize, z: 0 },
                        size: MTLSize { width: params.width as usize, height: params.height as usize, depth: 1 }
                    };

                    if let Some(texture) = &mut self.vram_write {
                        unsafe {
                            texture.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
                                region,
                                0,
                                NonNull::new(rgba8_buffer.as_ptr() as *mut c_void).unwrap(),
                                params.width as usize * 4
                            );
                        }
                    }

                    if let Some(texture) = &mut self.vram_read {
                        unsafe {
                            texture.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
                                region,
                                0,
                                NonNull::new(params.halfwords.as_ptr() as *mut c_void).unwrap(),
                                params.width as usize * 2
                            )
                        }
                    }
                }
                GPUCommand::VRAMtoCPU(params) => {
                    gpu.transfer_params = Some(params);
                }
                _  => todo!("VRAMFill")
            }
        }
    }

    pub fn handle_cpu_transfer(&mut self, params: &CPUTransferParams) -> Vec<u16> {
        let mut halfwords = Vec::new();

        let row_bytes = params.width * 2;

        if let Some(command_buffer) = self.command_queue.commandBuffer() {
            let desc = unsafe {
                MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
                    MTLPixelFormat::R16Uint, params.width as _, params.height as _, false
                )
            };

            desc.setUsage(MTLTextureUsage::ShaderRead);
            desc.setStorageMode(MTLStorageMode::Shared);

            let tmp: Retained<ProtocolObject<dyn MTLTexture>> =
                self.device.newTextureWithDescriptor(&desc).expect("tmp tex");

            if let Some(blit) = command_buffer.blitCommandEncoder(){
                unsafe {
                    blit.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin(
                        self.vram_read.as_ref().unwrap(), 0, 0,
                        MTLOrigin { x: params.start_x as _, y: params.start_y as _, z: 0 },
                        MTLSize   { width: params.width as _, height: params.height as _, depth: 1 },
                        &tmp, 0, 0,
                        MTLOrigin { x: 0, y: 0, z: 0 },
                    );

                    blit.endEncoding();
                    command_buffer.commit();
                    // command_buffer.waitUntilCompleted();

                    let mut bytes: Vec<u8> = vec![0xff; params.width as usize * params.height as usize * 2];

                    tmp.getBytes_bytesPerRow_fromRegion_mipmapLevel(
                        NonNull::new(bytes.as_mut_ptr().cast() as *mut c_void).unwrap(),
                        row_bytes as _,
                        MTLRegion {
                            origin: MTLOrigin { x: 0, y: 0, z: 0 },
                            size:   MTLSize   { width: params.width as _, height: params.height as usize, depth: 1 },
                        },
                        0,
                    );

                    for i in (0..bytes.len()).step_by(2) {
                        let halfword = bytes[i] as u16 | (bytes[i + 1] as u16) << 8;

                        halfwords.push(halfword);
                    }
                }
            }
        }

        halfwords
    }

    pub fn clip_drawing_area(gpu: &mut GPU) -> MTLScissorRect {
        let width = (gpu.x2 - gpu.x1 + 1) as usize;
        let height = (gpu.y2 - gpu.y1 + 1) as usize;

        MTLScissorRect { x: gpu.x1 as usize, y: gpu.y1 as usize, width, height }
    }

    pub fn process(&mut self, gpu: &mut GPU) {
        if gpu.commands_ready {
            gpu.commands_ready = false;
            if !gpu.gpu_commands.is_empty() {
                self.process_commands(gpu);
            }

            if gpu.polygons.len() > 0 {
                if self.encoder.is_none() {
                    let rpd = unsafe { MTLRenderPassDescriptor::new() };
                    self.command_buffer = self.command_queue.commandBuffer();

                    let color_attachment = unsafe { rpd.colorAttachments().objectAtIndexedSubscript(0) };

                    color_attachment.setLoadAction(MTLLoadAction::Load);
                    color_attachment.setStoreAction(MTLStoreAction::Store);

                    color_attachment.setClearColor(MTLClearColor { red: 0.0, green: 0.0, blue: 0.0, alpha: 1.0 });
                    color_attachment.setTexture(self.vram_write.as_deref());

                    self.encoder = self.command_buffer.as_ref().unwrap().renderCommandEncoderWithDescriptor(&rpd);
                }

                if let Some(encoder_ref) = &mut self.encoder {
                    encoder_ref.setCullMode(MTLCullMode::None);
                    encoder_ref.setFrontFacingWinding(MTLWinding::Clockwise);

                    let vp = MTLViewport {
                        originX: 0.0, originY: 0.0,
                        width: 1024.0, height: 512.0,
                        znear: 0.0, zfar: 1.0,
                    };

                    encoder_ref.setViewport(vp);

                    let drawing_area = Self::clip_drawing_area(gpu);
                    encoder_ref.setScissorRect(drawing_area);

                    self.render_polygons(gpu);
                }
            }
            if let Some(params) = &gpu.transfer_params {
                let halfwords = self.handle_cpu_transfer(params);

                for halfword in halfwords {
                    gpu.gpuread_fifo.push_back(halfword);
                }
            }
        }
    }

    pub fn present(&mut self, gpu: &mut GPU) {
        let drawable = unsafe { self.metal_layer.nextDrawable() };

        if let (Some(encoder), Some(command_buffer)) = (&mut self.encoder.take(), &mut self.command_buffer.take()) {
            encoder.endEncoding();
            command_buffer.commit();
        }

        if let Some(drawable) = &drawable {
            let rpd = unsafe { MTLRenderPassDescriptor::new() };

            self.command_buffer = self.command_queue.commandBuffer();

            let color_attachment = unsafe { rpd.colorAttachments().objectAtIndexedSubscript(0) };

            color_attachment.setLoadAction(MTLLoadAction::Load);
            color_attachment.setStoreAction(MTLStoreAction::Store);

            color_attachment.setClearColor(MTLClearColor { red: 1.0, green: 0.0, blue: 0.0, alpha: 1.0 });
            unsafe {
                color_attachment.setTexture(Some(&drawable.texture()));
            }

            if let Some(command_buffer) = &self.command_buffer {
                if let Some(draw_encoder) = command_buffer.renderCommandEncoderWithDescriptor(&rpd) {
                    draw_encoder.setCullMode(MTLCullMode::None);
                    draw_encoder.setFrontFacingWinding(MTLWinding::Clockwise);

                    if gpu.resolution_changed {
                        let width = gpu.display_width as f64;
                        let height = gpu.display_height as f64;

                        gpu.resolution_changed = false;

                        unsafe { self.metal_layer.setDrawableSize(CGSize::new(gpu.display_width as f64, gpu.display_height as f64)); }
                        self.vertices = Self::get_vertices(gpu.display_width, gpu.display_height);

                        self.buffer = unsafe {
                            self.device.newBufferWithBytes_length_options(
                                NonNull::new(
                                    self.vertices.as_ptr() as *mut c_void).unwrap(),
                                    BYTE_LEN,
                                    MTLResourceOptions::empty())

                        }.unwrap();

                        let vp = MTLViewport {
                            originX: 0.0, originY: 0.0,
                            width, height,
                            znear: 0.0, zfar: 1.0,
                        };

                        draw_encoder.setViewport(vp);

                    }

                    draw_encoder.setRenderPipelineState(&self.fb_pipeline_state);

                    unsafe {
                        draw_encoder.setVertexBuffer_offset_atIndex(Some(&self.buffer), 0, 0);
                        draw_encoder.setFragmentTexture_atIndex(self.vram_write.as_deref(), 0);
                        draw_encoder.drawPrimitives_vertexStart_vertexCount(MTLPrimitiveType::TriangleStrip, 0, 4);
                    }

                    draw_encoder.endEncoding();
                    command_buffer.presentDrawable(drawable.as_ref());
                    command_buffer.commit();
                }
            }
        }
    }

    pub fn get_vertices(display_width: u32, display_height: u32) -> [FbVertex; 4] {
        [
            FbVertex {
                position: [-1.0, 1.0],
                uv: [0.0, 0.0]
            },
            FbVertex {
                position: [1.0, 1.0],
                uv: [display_width as f32 / VRAM_WIDTH as f32, 0.0]
            },
            FbVertex {
                position: [-1.0, -1.0],
                uv: [0.0, display_height as f32 / VRAM_HEIGHT as f32]
            },
            FbVertex {
                position: [1.0, -1.0],
                uv: [display_width as f32 / VRAM_WIDTH as f32, display_height as f32 / VRAM_HEIGHT as f32]
            }
        ]
    }

}