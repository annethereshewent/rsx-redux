use std::{ffi::c_void, fs, ops::Deref, ptr::NonNull};

use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_core_foundation::CGSize;
use objc2_foundation::NSString;
use objc2_metal::{
    MTLBlitCommandEncoder, MTLBuffer, MTLClearColor, MTLCommandBuffer, MTLCommandEncoder,
    MTLCommandQueue, MTLCompareFunction, MTLComputeCommandEncoder, MTLComputePipelineState,
    MTLCreateSystemDefaultDevice, MTLCullMode, MTLDepthStencilDescriptor, MTLDepthStencilState,
    MTLDevice, MTLLibrary, MTLLoadAction, MTLOrigin, MTLPixelFormat, MTLPrimitiveType, MTLRegion,
    MTLRenderCommandEncoder, MTLRenderPassDescriptor, MTLRenderPipelineDescriptor,
    MTLRenderPipelineState, MTLResourceOptions, MTLScissorRect, MTLSize, MTLStencilDescriptor,
    MTLStencilOperation, MTLStorageMode, MTLStoreAction, MTLTexture, MTLTextureDescriptor,
    MTLTextureUsage, MTLVertexDescriptor, MTLVertexFormat, MTLViewport, MTLWinding,
};
use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};
use rsx_redux::cpu::bus::gpu::{
    CPUTransferParams, GPU, GPUCommand, Polygon, TexturePageColors, VRAM_HEIGHT, VRAM_WIDTH,
};
use std::cmp;

pub const BYTE_LEN: usize = 4 * std::mem::size_of::<FbVertex>();

#[repr(C)]
struct FbParams {
    display_start_x: u32,
    display_start_y: u32,
    display_width: u32,
    display_height: u32,
    display_depth: u32,
}

#[repr(C)]
#[derive(Debug)]
struct FragmentUniform {
    has_texture: bool,
    semitransparent: bool,
    modulate: bool,
    texture_mask_x: u32,
    texture_mask_y: u32,
    texture_offset_x: u32,
    texture_offset_y: u32,
    depth: i32,
    transparent_mode: u32,
    pass: u32,
    page: [u32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct MetalVertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
    clut: [u32; 2],
    orig: [f32; 2],
}

impl MetalVertex {
    pub fn new() -> Self {
        Self {
            position: [0.0; 2],
            uv: [0.0; 2],
            color: [0.0; 4],
            clut: [0; 2],
            orig: [0.0; 2],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct FbVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
}

pub struct Renderer {
    pub metal_layer: Retained<CAMetalLayer>,
    command_queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    pipeline_state: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    fb_pipeline_state: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    compute_pipeline_state: Retained<ProtocolObject<dyn MTLComputePipelineState>>,
    vram_read: Option<Retained<ProtocolObject<dyn MTLTexture>>>,
    vram_write: Option<Retained<ProtocolObject<dyn MTLTexture>>>,
    encoder: Option<Retained<ProtocolObject<dyn MTLRenderCommandEncoder>>>,
    command_buffer: Option<Retained<ProtocolObject<dyn MTLCommandBuffer>>>,
    vertices: [FbVertex; 4],
    buffer: Retained<ProtocolObject<dyn MTLBuffer>>,
    no_mask: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
    check_only: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
    set_only: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
    both: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
    has_semitransparent_polys: bool,
    leftover_polygons: Vec<Polygon>,
}

impl Renderer {
    pub fn new(metal_layer: Retained<CAMetalLayer>, gpu: &GPU) -> Self {
        let device = MTLCreateSystemDefaultDevice().unwrap();

        let source = NSString::from_str(&fs::read_to_string("shaders/Shaders.metal").unwrap());
        let fb_source = NSString::from_str(&fs::read_to_string("shaders/ShadersFb.metal").unwrap());

        let library = device
            .newLibraryWithSource_options_error(source.deref(), None)
            .unwrap();
        let fb_library = device
            .newLibraryWithSource_options_error(fb_source.deref(), None)
            .unwrap();

        let vertex_str = NSString::from_str("vertex_main");
        let fragment_str = NSString::from_str("fragment_main");

        let vertex_fb_str = NSString::from_str("vertex_fb");
        let fragment_fb_str = NSString::from_str("fragment_fb");

        let vertex_main_function = library.newFunctionWithName(&vertex_str);
        let fragment_main_function = library.newFunctionWithName(&fragment_str);

        let vertex_fb_function = fb_library.newFunctionWithName(&vertex_fb_str);
        let fragment_fb_function = fb_library.newFunctionWithName(&fragment_fb_str);

        let compute_function = library
            .newFunctionWithName(&NSString::from_str("rgba8_to_rgb5551"))
            .expect("Missing compute shader");

        let pipeline_descriptor = MTLRenderPipelineDescriptor::new();

        let fb_pipeline_descriptor = MTLRenderPipelineDescriptor::new();

        pipeline_descriptor.setVertexFunction(vertex_main_function.as_deref());
        pipeline_descriptor.setFragmentFunction(fragment_main_function.as_deref());

        fb_pipeline_descriptor.setVertexFunction(vertex_fb_function.as_deref());
        fb_pipeline_descriptor.setFragmentFunction(fragment_fb_function.as_deref());

        let color_attachment = unsafe {
            pipeline_descriptor
                .colorAttachments()
                .objectAtIndexedSubscript(0)
        };
        let fb_color_attachment = unsafe {
            fb_pipeline_descriptor
                .colorAttachments()
                .objectAtIndexedSubscript(0)
        };

        color_attachment.setPixelFormat(MTLPixelFormat::RGBA8Unorm);
        color_attachment.setBlendingEnabled(false);
        color_attachment.setRgbBlendOperation(objc2_metal::MTLBlendOperation::Add);
        color_attachment.setAlphaBlendOperation(objc2_metal::MTLBlendOperation::Add);
        // straight (non‑premultiplied) alpha
        color_attachment.setSourceRGBBlendFactor(objc2_metal::MTLBlendFactor::SourceAlpha);
        color_attachment
            .setDestinationRGBBlendFactor(objc2_metal::MTLBlendFactor::OneMinusSourceAlpha);
        color_attachment.setSourceAlphaBlendFactor(objc2_metal::MTLBlendFactor::One);
        color_attachment
            .setDestinationAlphaBlendFactor(objc2_metal::MTLBlendFactor::OneMinusSourceAlpha);

        fb_color_attachment.setPixelFormat(metal_layer.pixelFormat());
        fb_color_attachment.setBlendingEnabled(false);

        let vertex_descriptor = MTLVertexDescriptor::new();

        let attributes = vertex_descriptor.attributes();

        let position = unsafe { attributes.objectAtIndexedSubscript(0) };

        position.setFormat(MTLVertexFormat::Float2);
        unsafe { position.setOffset(0) };
        unsafe { position.setBufferIndex(0) };

        let uv = unsafe { attributes.objectAtIndexedSubscript(1) };

        uv.setFormat(MTLVertexFormat::Float2);
        unsafe { uv.setOffset(8) };
        unsafe { uv.setBufferIndex(0) };

        let color = unsafe { attributes.objectAtIndexedSubscript(2) };

        color.setFormat(MTLVertexFormat::Float4);
        unsafe { color.setOffset(16) };
        unsafe { color.setBufferIndex(0) };

        let clut = unsafe { attributes.objectAtIndexedSubscript(3) };
        clut.setFormat(MTLVertexFormat::UInt2);
        unsafe {
            clut.setOffset(32);
            clut.setBufferIndex(0);
        }

        let orig = unsafe { attributes.objectAtIndexedSubscript(4) };

        unsafe {
            orig.setOffset(40);
            orig.setBufferIndex(0);
        }

        orig.setFormat(MTLVertexFormat::Float2);

        let layout = unsafe { vertex_descriptor.layouts().objectAtIndexedSubscript(0) };

        unsafe { layout.setStride((std::mem::size_of::<MetalVertex>()) as usize) };

        let fb_vertex_descriptor = MTLVertexDescriptor::new();

        let fb_attributes = fb_vertex_descriptor.attributes();

        let fb_position = unsafe { fb_attributes.objectAtIndexedSubscript(0) };

        fb_position.setFormat(MTLVertexFormat::Float2);
        unsafe { fb_position.setOffset(0) };
        unsafe { fb_position.setBufferIndex(0) };

        let fb_uv = unsafe { fb_attributes.objectAtIndexedSubscript(1) };

        fb_uv.setFormat(MTLVertexFormat::Float2);
        unsafe { fb_uv.setOffset(8) };
        unsafe { fb_uv.setBufferIndex(0) };

        assert_eq!(size_of::<MetalVertex>(), 48);

        let fb_layout = unsafe { fb_vertex_descriptor.layouts().objectAtIndexedSubscript(0) };

        unsafe { fb_layout.setStride((std::mem::size_of::<FbVertex>()) as usize) };

        fb_pipeline_descriptor.setVertexDescriptor(Some(&fb_vertex_descriptor));

        pipeline_descriptor.setVertexDescriptor(Some(&vertex_descriptor));

        let pipeline_state = device
            .newRenderPipelineStateWithDescriptor_error(&pipeline_descriptor)
            .unwrap();
        let fb_pipeline_state = device
            .newRenderPipelineStateWithDescriptor_error(&fb_pipeline_descriptor)
            .unwrap();

        let compute_pipeline_state = device
            .newComputePipelineStateWithFunction_error(&compute_function)
            .unwrap();

        metal_layer.setDevice(Some(&device));

        let command_queue = device.newCommandQueue().unwrap();

        let vertices = Renderer::get_vertices();

        let byte_len = vertices.len() * std::mem::size_of::<FbVertex>();

        let buffer = unsafe {
            device.newBufferWithBytes_length_options(
                NonNull::new(vertices.as_ptr() as *mut c_void).unwrap(),
                byte_len,
                MTLResourceOptions::empty(),
            )
        }
        .unwrap();

        let vram_read = Self::create_texture(&device, true);
        let vram_write = Self::create_texture(&device, false);

        let rpd = MTLRenderPassDescriptor::new();

        let color_attachment = unsafe { rpd.colorAttachments().objectAtIndexedSubscript(0) };

        color_attachment.setLoadAction(MTLLoadAction::Clear);
        color_attachment.setStoreAction(MTLStoreAction::Store);

        color_attachment.setClearColor(MTLClearColor {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        });
        color_attachment.setTexture(vram_write.as_deref());

        let no_mask = Self::make_stencil_state(
            &device,
            MTLCompareFunction::Always,
            MTLStencilOperation::Keep,
        );
        let check_only = Self::make_stencil_state(
            &device,
            MTLCompareFunction::Equal,
            MTLStencilOperation::Keep,
        );
        let set_only = Self::make_stencil_state(
            &device,
            MTLCompareFunction::Always,
            MTLStencilOperation::Replace,
        );
        let both = Self::make_stencil_state(
            &device,
            MTLCompareFunction::Equal,
            MTLStencilOperation::Replace,
        );

        Self {
            metal_layer,
            command_queue,
            vram_read,
            vram_write,
            device,
            pipeline_state,
            fb_pipeline_state,
            encoder: None,
            command_buffer: None,
            vertices,
            buffer,
            no_mask,
            check_only,
            set_only,
            both,
            compute_pipeline_state,
            has_semitransparent_polys: false,
            leftover_polygons: Vec::new(),
        }
    }

    fn make_stencil_state(
        device: &Retained<ProtocolObject<dyn MTLDevice>>,
        cmp: MTLCompareFunction,
        pass_op: MTLStencilOperation,
    ) -> Retained<ProtocolObject<dyn MTLDepthStencilState>> {
        let ds = MTLDepthStencilDescriptor::new();
        ds.setDepthCompareFunction(MTLCompareFunction::Always);
        ds.setDepthWriteEnabled(false);

        let front = MTLStencilDescriptor::new();
        front.setStencilCompareFunction(cmp);
        front.setReadMask(0xFF);
        front.setWriteMask(0xFF);
        front.setDepthStencilPassOperation(pass_op);
        front.setStencilFailureOperation(MTLStencilOperation::Keep);
        front.setDepthFailureOperation(MTLStencilOperation::Keep);
        ds.setFrontFaceStencil(Some(&front));
        ds.setBackFaceStencil(Some(&front));

        device.newDepthStencilStateWithDescriptor(&ds).unwrap()
    }

    fn setup_encoder(
        gpu: &GPU,
        encoder_ref: &mut Retained<ProtocolObject<dyn MTLRenderCommandEncoder>>,
    ) {
        encoder_ref.setCullMode(MTLCullMode::None);
        encoder_ref.setFrontFacingWinding(MTLWinding::Clockwise);

        let vp = MTLViewport {
            originX: 0.0,
            originY: 0.0,
            width: 1024.0,
            height: 512.0,
            znear: 0.0,
            zfar: 1.0,
        };

        encoder_ref.setViewport(vp);

        let drawing_area = Self::clip_drawing_area(gpu);
        encoder_ref.setScissorRect(drawing_area);
    }

    pub fn render_polygon(&mut self, gpu: &mut GPU, polygon: &Polygon) {
        let mut vertices: Vec<MetalVertex> = vec![MetalVertex::new(); polygon.vertices.len()];

        let depth = if let Some(texpage) = polygon.texpage {
            match texpage.texture_page_colors {
                TexturePageColors::Bit4 => 0,
                TexturePageColors::Bit8 => 1,
                TexturePageColors::Bit15 => 2,
            }
        } else {
            -1
        };

        let mut fragment_uniform = FragmentUniform {
            has_texture: polygon.textured,
            texture_mask_x: gpu.texture_window_mask_x,
            texture_mask_y: gpu.texture_window_mask_y,
            texture_offset_x: gpu.texture_window_offset_x,
            texture_offset_y: gpu.texture_window_offset_y,
            semitransparent: polygon.semitransparent,
            modulate: polygon.modulate,
            depth,
            transparent_mode: polygon.transparent_mode,
            pass: 1,
            page: [0; 2],
        };

        let cross_product = GPU::cross_product(&polygon.vertices);
        let v = &polygon.vertices;

        if cross_product == 0 {
            return;
        }

        let min_x = cmp::min(v[0].x, cmp::min(v[1].x, v[2].x));
        let min_y = cmp::min(v[0].y, cmp::min(v[1].y, v[2].y));

        let max_x = cmp::max(v[0].x, cmp::max(v[1].x, v[2].x));
        let max_y = cmp::max(v[0].y, cmp::max(v[1].y, v[2].y));

        if (max_x >= 1024 && min_x >= 1024) || (max_x < 0 && min_x < 0) {
            return;
        }

        if (max_y >= 512 && min_y >= 512) || (max_y < 0 && min_y < 0) {
            return;
        }

        if (max_x - min_x) >= 1024 || (max_y - min_y) >= 512 {
            return;
        }

        for i in 0..polygon.vertices.len() {
            let vertex = &polygon.vertices[i];

            let u = vertex.u;
            let v = vertex.v;

            let metal_vert = &mut vertices[i];

            metal_vert.orig[0] = vertex.x as f32;
            metal_vert.orig[1] = vertex.y as f32;

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
                fragment_uniform.page = [texpage.x_base as u32 * 64, texpage.y_base1 as u32 * 16];
            }
        }

        let byte_len = vertices.len() * std::mem::size_of::<MetalVertex>();

        let buffer = unsafe {
            self.device.newBufferWithBytes_length_options(
                NonNull::new(vertices.as_ptr() as *mut c_void).unwrap(),
                byte_len,
                MTLResourceOptions::empty(),
            )
        }
        .unwrap();

        if let Some(encoder) = &self.encoder {
            unsafe {
                encoder.setFragmentBytes_length_atIndex(
                    NonNull::new(&mut fragment_uniform as *mut _ as *mut c_void).unwrap(),
                    size_of::<FragmentUniform>(),
                    1,
                )
            };
            unsafe { encoder.setVertexBuffer_offset_atIndex(Some(buffer.deref()), 0, 0) };

            let primitive_type = MTLPrimitiveType::Triangle;

            if polygon.semitransparent && !self.has_semitransparent_polys {
                self.has_semitransparent_polys = true;
                return;
            }

            encoder.setRenderPipelineState(&self.pipeline_state);

            let stencil_state = match (gpu.force_mask_bit, gpu.preserve_masked_pixels) {
                (false, false) => &self.no_mask,
                (true, false) => &self.set_only,
                (false, true) => &self.check_only,
                (true, true) => &self.both,
            };

            unsafe {
                encoder.setDepthStencilState(Some(stencil_state));
                encoder.setStencilReferenceValue(if gpu.force_mask_bit { 1 } else { 0 });
                encoder.setFragmentTexture_atIndex(self.vram_read.as_deref(), 0);
                encoder.drawPrimitives_vertexStart_vertexCount(primitive_type, 0, vertices.len());
            }
        }
        self.has_semitransparent_polys = false;
    }

    pub fn render_polygons(&mut self, gpu: &mut GPU) {
        let polygons: Vec<Polygon> = if !self.has_semitransparent_polys {
            gpu.polygons.drain(..).collect()
        } else {
            self.leftover_polygons.drain(..).collect()
        };
        for (index, polygon) in polygons.iter().enumerate() {
            self.render_polygon(gpu, polygon);
            if self.has_semitransparent_polys {
                self.vram_writeback(gpu);

                self.leftover_polygons = polygons[index..].to_vec();

                self.render_polygons(gpu);

                break;
            }
        }
    }

    fn create_encoder(&mut self) {
        let rpd = MTLRenderPassDescriptor::new();
        self.command_buffer = self.command_queue.commandBuffer();

        let color_attachment = unsafe { rpd.colorAttachments().objectAtIndexedSubscript(0) };

        color_attachment.setLoadAction(MTLLoadAction::Load);
        color_attachment.setStoreAction(MTLStoreAction::Store);

        color_attachment.setClearColor(MTLClearColor {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        });
        color_attachment.setTexture(self.vram_write.as_deref());

        self.encoder = self
            .command_buffer
            .as_ref()
            .unwrap()
            .renderCommandEncoderWithDescriptor(&rpd);
    }

    pub fn process_commands(&mut self, gpu: &mut GPU) {
        for command in gpu.gpu_commands.drain(..) {
            match command {
                GPUCommand::CPUtoVram(params) => {
                    let mut rgba8_buffer: Vec<u8> = Vec::new();

                    let mut i = 0;
                    for _ in 0..params.height {
                        for _ in 0..params.width {
                            let halfword = params.halfwords[i];

                            let mut r = halfword & 0x1f;
                            let mut g = (halfword >> 5) & 0x1f;
                            let mut b = (halfword >> 10) & 0x1f;
                            let a = ((halfword >> 15) & 1) * 0xff;

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
                        origin: MTLOrigin {
                            x: params.start_x as usize,
                            y: params.start_y as usize,
                            z: 0,
                        },
                        size: MTLSize {
                            width: params.width as usize,
                            height: params.height as usize,
                            depth: 1,
                        },
                    };

                    if let Some(texture) = &self.vram_write {
                        unsafe {
                            texture.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
                                region,
                                0,
                                NonNull::new(rgba8_buffer.as_ptr() as *mut c_void).unwrap(),
                                4 * params.width as usize,
                            )
                        }
                    }

                    if let Some(texture) = &self.vram_read {
                        unsafe {
                            texture.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
                                region,
                                0,
                                NonNull::new(params.halfwords.as_ptr() as *mut c_void).unwrap(),
                                2 * params.width as usize,
                            )
                        }
                    }
                }
                GPUCommand::VRAMtoCPU(params) => {
                    gpu.transfer_params = Some(params);
                }
                GPUCommand::VramToVram(params) => {
                    let texture_descriptor = unsafe {
                        MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
                            MTLPixelFormat::RGBA8Unorm,
                            params.width as usize,
                            params.height as usize,
                            false,
                        )
                    };

                    texture_descriptor.setStorageMode(MTLStorageMode::Shared);
                    texture_descriptor
                        .setUsage(MTLTextureUsage::ShaderRead | MTLTextureUsage::ShaderWrite);

                    let read_texture = self
                        .device
                        .newTextureWithDescriptor(&texture_descriptor)
                        .unwrap();
                    let write_texture = self
                        .device
                        .newTextureWithDescriptor(&texture_descriptor)
                        .unwrap();

                    let command_buffer = self.command_queue.commandBuffer().unwrap();
                    let blit_encoder = command_buffer.blitCommandEncoder().unwrap();

                    let origin = MTLOrigin {
                        x: params.source_start_x as usize,
                        y: params.source_start_y as usize,
                        z: 0,
                    };
                    let start_origin = MTLOrigin { x: 0, y: 0, z: 0 };
                    let size = MTLSize {
                        width: params.width as usize,
                        height: params.height as usize,
                        depth: 1,
                    };

                    unsafe {
                        blit_encoder.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin(
                            self.vram_write.as_ref().unwrap(),
                            0,
                            0,
                            origin,
                            size,
                            &write_texture,
                            0,
                            0,
                            start_origin
                        );

                        blit_encoder.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin(
                            &write_texture,
                            0,
                            0,
                            start_origin,
                            size,
                            self.vram_write.as_ref().unwrap(),
                            0,
                            0,
                            origin
                        );

                        blit_encoder.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin(
                            self.vram_read.as_ref().unwrap(),
                            0,
                            0,
                            origin,
                            size,
                            &read_texture,
                            0,
                            0,
                            start_origin
                        );

                        blit_encoder.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin(
                            &read_texture,
                            0,
                            0,
                            start_origin,
                            size,
                            self.vram_read.as_ref().unwrap(),
                            0,
                            0,
                            origin
                        );
                    }

                    blit_encoder.endEncoding();
                    command_buffer.commit();
                }
                GPUCommand::FillVRAM(params) => {
                    let mut halfwords: Vec<u16> = Vec::new();
                    let mut rgba8_bytes: Vec<u8> = Vec::new();

                    let mut r = (params.pixel & 0x1f) as u8;
                    let mut g = ((params.pixel >> 5) & 0x1f) as u8;
                    let mut b = ((params.pixel >> 10) & 0x1f) as u8;

                    r = r << 3 | r >> 2;
                    g = g << 3 | g >> 2;
                    b = b << 3 | b >> 2;

                    let a = (((params.pixel >> 15) & 1) * 255) as u8;

                    for _ in 0..params.height {
                        for _ in 0..params.width {
                            rgba8_bytes.push(r);
                            rgba8_bytes.push(g);
                            rgba8_bytes.push(b);
                            rgba8_bytes.push(a);

                            halfwords.push(params.pixel);
                        }
                    }

                    let region = MTLRegion {
                        origin: MTLOrigin {
                            x: params.start_x as usize,
                            y: params.start_y as usize,
                            z: 0,
                        },
                        size: MTLSize {
                            width: params.width as usize,
                            height: params.height as usize,
                            depth: 1,
                        },
                    };

                    if let Some(texture) = &self.vram_read {
                        unsafe {
                            texture.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
                                region,
                                0,
                                NonNull::new(halfwords.as_ptr() as *mut c_void).unwrap(),
                                2 * params.width as usize,
                            );
                        }
                    }

                    if let Some(texture) = &self.vram_write {
                        unsafe {
                            texture.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
                                region,
                                0,
                                NonNull::new(rgba8_bytes.as_ptr() as *mut c_void).unwrap(),
                                4 * params.width as usize,
                            );
                        }
                    }
                }
            }
        }
    }

    pub fn handle_cpu_transfer(&mut self, params: &CPUTransferParams) -> Vec<u16> {
        let mut halfwords = Vec::new();

        let row_bytes = params.width * 2;

        if let Some(texture) = &self.vram_read {
            let mut bytes: Vec<u8> = vec![0xff; params.width as usize * params.height as usize * 2];
            unsafe {
                texture.getBytes_bytesPerRow_fromRegion_mipmapLevel(
                    NonNull::new(bytes.as_mut_ptr() as *mut c_void).unwrap(),
                    row_bytes as usize,
                    MTLRegion {
                        origin: MTLOrigin {
                            x: params.start_x as usize,
                            y: params.start_y as usize,
                            z: 0,
                        },
                        size: MTLSize {
                            width: params.width as usize,
                            height: params.height as usize,
                            depth: 1,
                        },
                    },
                    0,
                );
            }

            for i in (0..bytes.len()).step_by(2) {
                let halfword = bytes[i] as u16 | (bytes[i + 1] as u16) << 8;

                halfwords.push(halfword);
            }
        }

        halfwords
    }

    fn clip_drawing_area(gpu: &GPU) -> MTLScissorRect {
        let (x, y, width, height) = Self::get_drawing_area(gpu);

        MTLScissorRect {
            x,
            y,
            width,
            height,
        }
    }

    fn get_drawing_area(gpu: &GPU) -> (usize, usize, usize, usize) {
        let width = (gpu.x2 - gpu.x1 + 1) as usize;
        let height = (gpu.y2 - gpu.y1 + 1) as usize;

        (gpu.x1 as usize, gpu.y1 as usize, width, height)
    }

    pub fn process(&mut self, gpu: &mut GPU) {
        if gpu.commands_ready {
            gpu.commands_ready = false;
            if !gpu.gpu_commands.is_empty() {
                self.process_commands(gpu);
            }

            if gpu.polygons.len() > 0 {
                if self.encoder.is_none() {
                    self.create_encoder();
                }

                if let Some(encoder_ref) = &mut self.encoder {
                    Self::setup_encoder(gpu, encoder_ref);

                    self.render_polygons(gpu);
                }
            }
            if let Some(params) = &gpu.transfer_params.take() {
                self.vram_writeback(gpu);

                let halfwords = self.handle_cpu_transfer(params);

                for halfword in halfwords {
                    gpu.gpuread_fifo.push_back(halfword);
                }
            }
        }
    }

    fn vram_writeback(&mut self, gpu: &mut GPU) {
        if let (Some(encoder), Some(command_buffer)) =
            (&mut self.encoder.take(), &mut self.command_buffer.take())
        {
            encoder.endEncoding();

            let compute_encoder = command_buffer.computeCommandEncoder().unwrap();
            compute_encoder.setComputePipelineState(&self.compute_pipeline_state);

            let (x, y, width, height) = Self::get_drawing_area(gpu);

            unsafe {
                compute_encoder.setTexture_atIndex(self.vram_write.as_deref(), 0);
                compute_encoder.setTexture_atIndex(self.vram_read.as_deref(), 1);

                let mut origin: [u32; 2] = [x as u32, y as u32];

                compute_encoder.setBytes_length_atIndex(
                    NonNull::new(&mut origin as *mut _ as *mut c_void).unwrap(),
                    std::mem::size_of::<[u32; 2]>(),
                    0,
                );

                let threadgroup_size = MTLSize {
                    width: 8,
                    height: 8,
                    depth: 1,
                };

                let threadgroups = MTLSize {
                    width: (width + 7) / 8,
                    height: (height + 7) / 8,
                    depth: 1,
                };

                compute_encoder
                    .dispatchThreadgroups_threadsPerThreadgroup(threadgroups, threadgroup_size);
                compute_encoder.endEncoding();
            }

            command_buffer.commit();

            self.create_encoder();

            if let Some(encoder_ref) = &mut self.encoder {
                Self::setup_encoder(gpu, encoder_ref);
            }
        }
    }

    pub fn present(&mut self, gpu: &mut GPU) {
        let drawable = self.metal_layer.nextDrawable();

        if let (Some(encoder), Some(command_buffer)) =
            (&mut self.encoder.take(), &mut self.command_buffer.take())
        {
            encoder.endEncoding();
            command_buffer.commit();
        }

        if let Some(drawable) = &drawable {
            let rpd = MTLRenderPassDescriptor::new();

            self.command_buffer = self.command_queue.commandBuffer();

            let color_attachment = unsafe { rpd.colorAttachments().objectAtIndexedSubscript(0) };

            color_attachment.setLoadAction(MTLLoadAction::Load);
            color_attachment.setStoreAction(MTLStoreAction::Store);

            color_attachment.setClearColor(MTLClearColor {
                red: 1.0,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0,
            });
            color_attachment.setTexture(Some(&drawable.texture()));

            if let Some(command_buffer) = &self.command_buffer {
                if let Some(draw_encoder) = command_buffer.renderCommandEncoderWithDescriptor(&rpd)
                {
                    draw_encoder.setCullMode(MTLCullMode::None);
                    draw_encoder.setFrontFacingWinding(MTLWinding::Clockwise);

                    self.metal_layer.setDrawableSize(CGSize::new(640.0, 480.0));

                    self.vertices = Self::get_vertices();

                    self.buffer = unsafe {
                        self.device.newBufferWithBytes_length_options(
                            NonNull::new(self.vertices.as_ptr() as *mut c_void).unwrap(),
                            BYTE_LEN,
                            MTLResourceOptions::empty(),
                        )
                    }
                    .unwrap();

                    let vp = MTLViewport {
                        originX: 0.0,
                        originY: 0.0,
                        width: 640.0,
                        height: 480.0,
                        znear: 0.0,
                        zfar: 1.0,
                    };

                    draw_encoder.setViewport(vp);

                    draw_encoder.setRenderPipelineState(&self.fb_pipeline_state);

                    unsafe {
                        draw_encoder.setVertexBuffer_offset_atIndex(Some(&self.buffer), 0, 0);
                        draw_encoder.setFragmentTexture_atIndex(self.vram_write.as_deref(), 0);
                        draw_encoder.setFragmentTexture_atIndex(self.vram_read.as_deref(), 1);

                        let display_depth = gpu.display_depth as u32;
                        let (width, height) = gpu.get_dimensions();

                        let fb_params = FbParams {
                            display_depth,
                            display_height: height,
                            display_width: width,
                            display_start_x: gpu.display_start_x,
                            display_start_y: gpu.display_start_y,
                        };
                        draw_encoder.setFragmentBytes_length_atIndex(
                            NonNull::new(&fb_params as *const FbParams as *mut c_void).unwrap(),
                            std::mem::size_of::<FbParams>(),
                            0,
                        );

                        draw_encoder.drawPrimitives_vertexStart_vertexCount(
                            MTLPrimitiveType::TriangleStrip,
                            0,
                            4,
                        );
                    }

                    draw_encoder.endEncoding();
                    command_buffer.presentDrawable(drawable.as_ref());
                    command_buffer.commit();
                }
            }
        }
    }

    pub fn get_vertices() -> [FbVertex; 4] {
        [
            FbVertex {
                position: [-1.0, 1.0],
                uv: [0.0, 0.0],
            },
            FbVertex {
                position: [1.0, 1.0],
                uv: [1.0, 0.0],
            },
            FbVertex {
                position: [-1.0, -1.0],
                uv: [0.0, 1.0],
            },
            FbVertex {
                position: [1.0, -1.0],
                uv: [1.0, 1.0],
            },
        ]
    }

    fn create_texture(
        device: &Retained<ProtocolObject<dyn MTLDevice>>,
        is_read: bool,
    ) -> Option<Retained<ProtocolObject<dyn MTLTexture>>> {
        let pixel_format = if is_read {
            MTLPixelFormat::R16Uint
        } else {
            MTLPixelFormat::RGBA8Unorm
        };
        let descriptor = unsafe {
            MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
                pixel_format,
                VRAM_WIDTH,
                VRAM_HEIGHT,
                false,
            )
        };

        descriptor.setStorageMode(MTLStorageMode::Shared);

        if is_read {
            descriptor.setUsage(MTLTextureUsage::ShaderRead | MTLTextureUsage::ShaderWrite)
        } else {
            descriptor.setUsage(MTLTextureUsage::ShaderRead | MTLTextureUsage::RenderTarget);
        }

        let mtl_texture = device.newTextureWithDescriptor(&descriptor);

        mtl_texture
    }
}
