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

        Self {
            window,
            event_pump: sdl_context.event_pump().unwrap(),
            _controller: controller,
            renderer: Renderer::new(metal_view, metal_layer, gpu)
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
}