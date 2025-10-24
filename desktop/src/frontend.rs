use std::process::exit;
use std::sync::Arc;

use objc2::rc::Retained;
use objc2_quartz_core::CAMetalLayer;
use ringbuf::SharedRb;
use ringbuf::storage::Heap;
use ringbuf::traits::{Consumer, Observer};
use ringbuf::wrap::caching::Caching;
use rsx_redux::cpu::CPU;
use rsx_redux::cpu::bus::gpu::GPU;
use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};
use sdl2::keyboard::Keycode;
use sdl2::sys::{SDL_Metal_CreateView, SDL_Metal_GetLayer};
use sdl2::{EventPump, controller::GameController, event::Event, video::Window};

pub const VRAM_WIDTH: usize = 1024;
pub const VRAM_HEIGHT: usize = 512;

use crate::renderer::Renderer;

pub struct PsxAudioCallback {
    pub consumer: Caching<Arc<SharedRb<Heap<f32>>>, false, true>,
}

impl AudioCallback for PsxAudioCallback {
    type Channel = f32;

    fn callback(&mut self, buf: &mut [Self::Channel]) {
        let mut left_sample: f32 = 0.0;
        let mut right_sample: f32 = 0.0;

        if self.consumer.vacant_len() > 2 {
            left_sample = *self.consumer.try_peek().unwrap_or(&0.0);
            right_sample = *self.consumer.try_peek().unwrap_or(&0.0);
        }

        let mut is_left_sample = true;

        for b in buf.iter_mut() {
            *b = if let Some(sample) = self.consumer.try_pop() {
                sample
            } else {
                if is_left_sample {
                    left_sample
                } else {
                    right_sample
                }
            };

            is_left_sample = !is_left_sample;
        }
    }
}

pub struct Frontend {
    _window: Window,
    event_pump: EventPump,
    _controller: Option<GameController>,
    pub renderer: Renderer,
    _device: AudioDevice<PsxAudioCallback>,
}

impl Frontend {
    pub fn new(gpu: &GPU, consumer: Caching<Arc<SharedRb<Heap<f32>>>, false, true>) -> Self {
        let sdl_context = sdl2::init().unwrap();
        let video_subsystem = sdl_context.video().unwrap();

        let game_controller_subsystem = sdl_context.game_controller().unwrap();

        let available = game_controller_subsystem
            .num_joysticks()
            .map_err(|e| format!("can't enumerate joysticks: {}", e))
            .unwrap();

        let controller = (0..available).find_map(|id| match game_controller_subsystem.open(id) {
            Ok(c) => Some(c),
            Err(_) => None,
        });

        let window = video_subsystem
            .window("RSX-redux", 640 as u32, 480 as u32)
            .position_centered()
            .build()
            .unwrap();

        let metal_view = unsafe { SDL_Metal_CreateView(window.raw()) };
        let metal_layer_ptr = unsafe { SDL_Metal_GetLayer(metal_view) };

        let metal_layer: Retained<CAMetalLayer> = unsafe {
            Retained::from_raw(metal_layer_ptr as *mut CAMetalLayer)
                .expect("Couldn cast pointer to CAMetalLayer!")
        };

        let audio_subsystem = sdl_context.audio().unwrap();

        let spec = AudioSpecDesired {
            freq: Some(44100),
            channels: Some(2),
            samples: Some(4096),
        };

        let device = audio_subsystem
            .open_playback(None, &spec, |_| PsxAudioCallback { consumer })
            .unwrap();

        device.resume();

        Self {
            _window: window,
            event_pump: sdl_context.event_pump().unwrap(),
            _controller: controller,
            renderer: Renderer::new(metal_layer, gpu),
            _device: device,
        }
    }

    pub fn handle_events(&mut self, cpu: &mut CPU) {
        for event in self.event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => {
                    exit(0);
                }
                Event::KeyDown { keycode, .. } => {
                    if let Some(keycode) = keycode {
                        if keycode == Keycode::G {
                            cpu.debug_on = !cpu.debug_on
                        } else if keycode == Keycode::F {
                            cpu.bus.gpu.debug_on = !cpu.bus.gpu.debug_on;
                        }
                    }
                }
                _ => (),
            }
        }
    }
}
