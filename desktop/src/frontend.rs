use std::os::raw::c_void;
use std::process::exit;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};
use rsx_redux::cpu::bus::gpu::Polygon;
use sdl2::{controller::GameController, event::Event, video::Window, EventPump};
use sdl2::sys::{SDL_Metal_CreateView, SDL_Metal_GetLayer};
use objc2_metal::{
    MTLCommandBuffer,
    MTLCommandEncoder,
    MTLCommandQueue,
    MTLCreateSystemDefaultDevice,
    MTLDevice as _,
    MTLDrawable,
    MTLLibrary,
    MTLPackedFloat3,
    MTLPrimitiveType,
    MTLRenderCommandEncoder,
    MTLRenderPipelineDescriptor,
    MTLRenderPipelineState
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

        unsafe { metal_layer.setDevice(Some(&device)) };

        let command_queue = device.newCommandQueue().unwrap();
        Self {
            window,
            event_pump: sdl_context.event_pump().unwrap(),
            _controller: controller,
            renderer: Renderer {
                metal_layer,
                metal_view,
                command_queue
            }
        }
    }

    pub fn handle_events(&mut self) {
        for event in self.event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => {
                    exit(0);
                }
                _ => ()
            }
        }
    }
}