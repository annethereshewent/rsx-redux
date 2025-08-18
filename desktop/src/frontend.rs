use std::ffi::c_void;
use std::fs;
use std::ops::Deref;
use std::process::exit;
use std::ptr::NonNull;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_quartz_core::CAMetalLayer;
use rsx_redux::cpu::bus::gpu::GPU;
use rsx_redux::cpu::CPU;
use sdl2::keyboard::Keycode;
use sdl2::{controller::GameController, event::Event, video::Window, EventPump};
use sdl2::sys::{SDL_Metal_CreateView, SDL_Metal_GetLayer};
use objc2_foundation::NSString;

use objc2_metal::{
    MTLCreateSystemDefaultDevice,
    MTLDevice,
    MTLLibrary,
    MTLPixelFormat,
    MTLRenderPipelineDescriptor,
    MTLTexture,
    MTLTextureDescriptor,
    MTLTextureUsage,
    MTLVertexDescriptor,
    MTLVertexFormat,
    MTLStorageMode,
    MTLResourceOptions
};

pub const VRAM_WIDTH: usize = 1024;
pub const VRAM_HEIGHT: usize = 512;

use crate::renderer::{FbVertex, MetalVertex, Renderer};

pub struct Frontend {
    window: Window,
    event_pump: EventPump,
    _controller: Option<GameController>,
    pub renderer: Renderer
}

impl Frontend {
    pub fn new(gpu: &GPU) -> Self {
        let sdl_context = sdl2::init().unwrap();
        let video_subsystem = sdl_context.video().unwrap();

        let game_controller_subsystem = sdl_context.game_controller().unwrap();

        let available = game_controller_subsystem
            .num_joysticks()
            .map_err(|e| format!("can't enumerate joysticks: {}", e)).unwrap();

        let controller = (0..available)
            .find_map(|id| {
            match game_controller_subsystem.open(id) {
                Ok(c) => {
                    Some(c)
                }
                Err(_) => {
                    None
                }
            }
        });

        let window = video_subsystem
            .window("RSX-redux", 640 as u32, 480 as u32)
            .position_centered()
            .build()
            .unwrap();

        let metal_view = unsafe { SDL_Metal_CreateView(window.raw()) };
        let metal_layer_ptr = unsafe { SDL_Metal_GetLayer(metal_view) };

        let metal_layer: Retained<CAMetalLayer> = unsafe { Retained::from_raw(metal_layer_ptr as *mut CAMetalLayer).expect("Couldn cast pointer to CAMetalLayer!") };

        let device = MTLCreateSystemDefaultDevice().unwrap();

        let source = NSString::from_str(&fs::read_to_string("shaders/Shaders.metal").unwrap());
        let fb_source = NSString::from_str(&fs::read_to_string("shaders/ShadersFb.metal").unwrap());

        let library = device.newLibraryWithSource_options_error(source.deref(), None).unwrap();
        let fb_library = device.newLibraryWithSource_options_error(fb_source.deref(), None).unwrap();

        let vertex_str = NSString::from_str("vertex_main");
        let fragment_str = NSString::from_str("fragment_main");

        let vertex_fb_str = NSString::from_str("vertex_fb");
        let fragment_fb_str = NSString::from_str("fragment_fb");

        let vertex_main_function = library.newFunctionWithName(&vertex_str);
        let fragment_main_function = library.newFunctionWithName(&fragment_str);

        let vertex_fb_function = fb_library.newFunctionWithName(&vertex_fb_str);
        let fragment_fb_function = fb_library.newFunctionWithName(&fragment_fb_str);

        let pipeline_descriptor = MTLRenderPipelineDescriptor::new();

        let fb_pipeline_descriptor = MTLRenderPipelineDescriptor::new();

        pipeline_descriptor.setVertexFunction(vertex_main_function.as_deref());
        pipeline_descriptor.setFragmentFunction(fragment_main_function.as_deref());

        fb_pipeline_descriptor.setVertexFunction(vertex_fb_function.as_deref());
        fb_pipeline_descriptor.setFragmentFunction(fragment_fb_function.as_deref());

        let color_attachment = unsafe { pipeline_descriptor.colorAttachments().objectAtIndexedSubscript(0) };
        let fb_color_attachment = unsafe { fb_pipeline_descriptor.colorAttachments().objectAtIndexedSubscript(0) };

        // color_attachment.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
        unsafe {
            color_attachment.setPixelFormat(metal_layer.pixelFormat());
            color_attachment.setBlendingEnabled(true);
            color_attachment.setRgbBlendOperation(objc2_metal::MTLBlendOperation::Add);
            color_attachment.setAlphaBlendOperation(objc2_metal::MTLBlendOperation::Add);
            // straight (non‑premultiplied) alpha
            color_attachment.setSourceRGBBlendFactor(objc2_metal::MTLBlendFactor::SourceAlpha);
            color_attachment.setDestinationRGBBlendFactor(objc2_metal::MTLBlendFactor::OneMinusSourceAlpha);
            color_attachment.setSourceAlphaBlendFactor(objc2_metal::MTLBlendFactor::One);
            color_attachment.setDestinationAlphaBlendFactor(objc2_metal::MTLBlendFactor::OneMinusSourceAlpha);

            fb_color_attachment.setPixelFormat(metal_layer.pixelFormat());
            fb_color_attachment.setBlendingEnabled(false);
        }



        let vertex_descriptor = unsafe { MTLVertexDescriptor::new() };

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

        let page = unsafe { attributes.objectAtIndexedSubscript(3) };

        page.setFormat(MTLVertexFormat::UInt2);
        unsafe {
            page.setOffset(32);
            page.setBufferIndex(0);
        }

        let depth = unsafe { attributes.objectAtIndexedSubscript(4) };

        depth.setFormat(MTLVertexFormat::UInt);
        unsafe {
            depth.setOffset(40);
            depth.setBufferIndex(0);
        }

        let clut = unsafe { attributes.objectAtIndexedSubscript(5) };
        clut.setFormat(MTLVertexFormat::UInt2);
        unsafe {
            clut.setOffset(48);
            clut.setBufferIndex(0);
        }

        let layout = unsafe { vertex_descriptor.layouts().objectAtIndexedSubscript(0) };

        unsafe { layout.setStride((std::mem::size_of::<MetalVertex>()) as usize) };


        let fb_vertex_descriptor = unsafe { MTLVertexDescriptor::new() };

        let fb_attributes = fb_vertex_descriptor.attributes();

        let fb_position = unsafe { fb_attributes.objectAtIndexedSubscript(0) };

        fb_position.setFormat(MTLVertexFormat::Float2);
        unsafe { fb_position.setOffset(0) };
        unsafe { fb_position.setBufferIndex(0) };

        let fb_uv = unsafe { fb_attributes.objectAtIndexedSubscript(1) };

        fb_uv.setFormat(MTLVertexFormat::Float2);
        unsafe { fb_uv.setOffset(8) };
        unsafe { fb_uv.setBufferIndex(0) };

        assert_eq!(size_of::<MetalVertex>(), 56);

        let fb_layout = unsafe { fb_vertex_descriptor.layouts().objectAtIndexedSubscript(0) };

        unsafe { fb_layout.setStride((std::mem::size_of::<FbVertex>()) as usize) };

        fb_pipeline_descriptor.setVertexDescriptor(Some(&fb_vertex_descriptor));

        pipeline_descriptor.setVertexDescriptor(Some(&vertex_descriptor));

        let pipeline_state = device.newRenderPipelineStateWithDescriptor_error(&pipeline_descriptor).unwrap();
        let fb_pipeline_state = device.newRenderPipelineStateWithDescriptor_error(&fb_pipeline_descriptor).unwrap();

        unsafe { metal_layer.setDevice(Some(&device)) };

        let command_queue = device.newCommandQueue().unwrap();


        let vertices = Renderer::get_vertices(gpu.display_width, gpu.display_height);

        let byte_len = vertices.len() * std::mem::size_of::<FbVertex>();

        let buffer = unsafe {
            device.newBufferWithBytes_length_options(
                NonNull::new(
                    vertices.as_ptr() as *mut c_void).unwrap(),
                    byte_len,
                    MTLResourceOptions::empty())

        }.unwrap();

        Self {
            window,
            event_pump: sdl_context.event_pump().unwrap(),
            _controller: controller,
            renderer: Renderer {
                metal_layer,
                metal_view,
                command_queue,
                vram_read: Self::create_texture(&device, true),
                vram_write: Self::create_texture(&device, false),
                device,
                pipeline_state,
                fb_pipeline_state,
                encoder: None,
                command_buffer: None,
                vertices,
                buffer
            }
        }
    }

    pub fn handle_events(&mut self, cpu: &mut CPU) {
        for event in self.event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => {
                    exit(0);
                }
                Event::KeyDown { keycode, ..} => {
                    if let Some(keycode) = keycode {
                        if keycode == Keycode::G {
                            cpu.debug_on = !cpu.debug_on
                        } else if keycode == Keycode::F {
                            cpu.bus.gpu.debug_on = !cpu.bus.gpu.debug_on;
                        }
                    }
                }
                _ => ()
            }
        }
    }

    pub fn create_texture(device: &Retained<ProtocolObject<dyn MTLDevice>>, is_read: bool) -> Option<Retained<ProtocolObject<dyn MTLTexture>>> {
        let pixel_format = if is_read { MTLPixelFormat::R16Uint } else { MTLPixelFormat::BGRA8Unorm };
        let descriptor = unsafe {
            MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
                pixel_format,
                VRAM_WIDTH,
                VRAM_HEIGHT,
                false
            )
        };

        descriptor.setStorageMode(MTLStorageMode::Shared);

        if is_read {
            descriptor.setUsage(MTLTextureUsage::ShaderRead)
        } else {
            descriptor.setUsage(MTLTextureUsage::ShaderRead | MTLTextureUsage::RenderTarget);
        }

        let mtl_texture = device.newTextureWithDescriptor(&descriptor);

        mtl_texture
    }
}