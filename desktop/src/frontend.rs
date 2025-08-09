use std::borrow::Borrow;
use std::fs;
use std::ops::Deref;
use std::os::raw::c_void;
use std::process::exit;

use objc2::rc::Retained;
use objc2_quartz_core::CAMetalLayer;
use rsx_redux::cpu::CPU;
use sdl2::keyboard::Keycode;
use sdl2::{controller::GameController, event::Event, video::Window, EventPump};
use sdl2::sys::{SDL_Metal_CreateView, SDL_Metal_GetLayer};
use objc2_foundation::NSString;

use objc2_metal::{
    MTLCommandBuffer, MTLCommandEncoder, MTLCommandQueue, MTLCreateSystemDefaultDevice, MTLDevice as _, MTLDrawable, MTLLibrary, MTLPackedFloat3, MTLPixelFormat, MTLPrimitiveType, MTLRenderCommandEncoder, MTLRenderPipelineDescriptor, MTLRenderPipelineState, MTLVertexDescriptor, MTLVertexFormat
};

use crate::renderer::Renderer;

pub struct Frontend {
    window: Window,
    event_pump: EventPump,
    _controller: Option<GameController>,
    pub renderer: Renderer
}

impl Frontend {
    pub fn new() -> Self {
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
            .window("RSX-redux", (320 * 2) as u32, (240 * 2) as u32)
            .position_centered()
            .build()
            .unwrap();

        let metal_view = unsafe { SDL_Metal_CreateView(window.raw()) };
        let metal_layer_ptr = unsafe { SDL_Metal_GetLayer(metal_view) };

        let metal_layer: Retained<CAMetalLayer> = unsafe { Retained::from_raw(metal_layer_ptr as *mut CAMetalLayer).expect("Couldn cast pointer to CAMetalLayer!") };

        let device = MTLCreateSystemDefaultDevice().unwrap();

        let source = NSString::from_str(&fs::read_to_string("shaders/Shaders.metal").unwrap());

        let library = device.newLibraryWithSource_options_error(source.deref(), None).unwrap();

        let vertex_str = NSString::from_str("vertex_main");
        let fragment_str = NSString::from_str("fragment_main");

        let vertex_main_function = library.newFunctionWithName(&vertex_str);
        let fragment_main_function = library.newFunctionWithName(&fragment_str);

        let pipeline_descriptor = MTLRenderPipelineDescriptor::new();

        pipeline_descriptor.setVertexFunction(vertex_main_function.as_deref());
        pipeline_descriptor.setFragmentFunction(fragment_main_function.as_deref());
        let color_attachment = unsafe { pipeline_descriptor.colorAttachments().objectAtIndexedSubscript(0) };

        // color_attachment.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
        unsafe { color_attachment.setPixelFormat(metal_layer.pixelFormat()) };

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

        let layout = unsafe { vertex_descriptor.layouts().objectAtIndexedSubscript(0) };

        unsafe { layout.setStride((8 * std::mem::size_of::<f32>()) as usize) };

        pipeline_descriptor.setVertexDescriptor(Some(&vertex_descriptor));

        let pipeline_state = device.newRenderPipelineStateWithDescriptor_error(&pipeline_descriptor).unwrap();

        unsafe { metal_layer.setDevice(Some(&device)) };

        let command_queue = device.newCommandQueue().unwrap();
        Self {
            window,
            event_pump: sdl_context.event_pump().unwrap(),
            _controller: controller,
            renderer: Renderer {
                metal_layer,
                metal_view,
                command_queue,
                device,
                pipeline_state
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
                        }
                    }
                }
                _ => ()
            }
        }
    }
}