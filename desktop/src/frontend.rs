use dirs_next::data_dir;
use memmap2::{Mmap, MmapMut};
#[cfg(feature = "hardware_gpu")]
use objc2::rc::Retained;
#[cfg(feature = "hardware_gpu")]
use objc2_quartz_core::CAMetalLayer;
use rsx_redux::cpu::CPU;
use rsx_redux::cpu::bus::gpu::{GPU, SCREEN_HEIGHT, SCREEN_WIDTH};
use rsx_redux::cpu::bus::peripherals::memory_card::MEMORY_SIZE;
use sdl2::GameControllerSubsystem;
use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};
use sdl2::controller::{Axis, Button};
use sdl2::keyboard::Keycode;
#[cfg(feature = "software_gpu")]
use sdl2::pixels::PixelFormatEnum;
#[cfg(feature = "software_gpu")]
use sdl2::render::Canvas;
#[cfg(feature = "hardware_gpu")]
use sdl2::sys::{SDL_Metal_CreateView, SDL_Metal_GetLayer};
use sdl2::{EventPump, controller::GameController, event::Event, video::Window};
use std::collections::{HashMap, VecDeque};
use std::fs::{self, File, OpenOptions};
use std::ops::DerefMut;
use std::path::{Path, PathBuf};
use std::process::exit;

#[cfg(feature = "hardware_gpu")]
use crate::renderer::Renderer;

pub struct PsxAudioCallback {
    pub audio_buffer: VecDeque<i16>,
}

impl AudioCallback for PsxAudioCallback {
    type Channel = i16;

    fn callback(&mut self, buf: &mut [Self::Channel]) {
        let mut left_sample: i16 = 0;
        let mut right_sample: i16 = 0;

        let len = self.audio_buffer.len();

        if self.audio_buffer.len() > 2 {
            left_sample = self.audio_buffer[len - 2];
            right_sample = self.audio_buffer[len - 1];
        }

        let mut is_left_sample = true;

        for b in buf.iter_mut() {
            *b = if let Some(sample) = self.audio_buffer.pop_front() {
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

impl PsxAudioCallback {
    pub fn push_samples(&mut self, samples: Vec<i16>) {
        for sample in samples.iter() {
            self.audio_buffer.push_back(*sample);
        }
    }
}

pub struct Frontend {
    #[cfg(feature = "hardware_gpu")]
    _window: Window,
    event_pump: EventPump,
    controller: Option<GameController>,
    game_controller_subsystem: GameControllerSubsystem,
    controller_id: Option<u32>,
    retry_attempts: usize,
    #[cfg(feature = "hardware_gpu")]
    pub renderer: Renderer,
    #[cfg(feature = "software_gpu")]
    canvas: Canvas<Window>,
    device: AudioDevice<PsxAudioCallback>,
    button_map: HashMap<Button, usize>,
    button_map2: HashMap<Axis, usize>,
    key_map: HashMap<Keycode, usize>,
}

impl Frontend {
    fn reconnect_controller(&mut self, controller_id: u32) -> Option<GameController> {
        if self.retry_attempts < 5 {
            match self.game_controller_subsystem.open(controller_id) {
                Ok(c) => Some(c),
                Err(_) => {
                    self.retry_attempts += 1;
                    None
                }
            }
        } else {
            None
        }
    }

    pub fn check_controller_status(&mut self) {
        if let Some(controller_id) = self.controller_id {
            self.controller = self.reconnect_controller(controller_id);

            if self.controller.is_some() || self.retry_attempts >= 5 {
                self.controller_id = None;
                self.retry_attempts = 0;
            }
        }
    }

    pub fn push_samples(&mut self, samples: Vec<i16>) {
        self.device.lock().deref_mut().push_samples(samples);
    }

    #[allow(unused_variables)]
    pub fn new(gpu: &GPU) -> Self {
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
            .window("RSX-redux", SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32)
            .position_centered()
            .build()
            .unwrap();

        #[cfg(feature = "software_gpu")]
        let mut canvas = window.into_canvas().present_vsync().build().unwrap();

        #[cfg(feature = "software_gpu")]
        canvas.set_scale(3.0, 3.0).unwrap();

        #[cfg(feature = "hardware_gpu")]
        let metal_view = unsafe { SDL_Metal_CreateView(window.raw()) };
        #[cfg(feature = "hardware_gpu")]
        let metal_layer_ptr = unsafe { SDL_Metal_GetLayer(metal_view) };

        #[cfg(feature = "hardware_gpu")]
        let metal_layer: Retained<CAMetalLayer> = unsafe {
            Retained::from_raw(metal_layer_ptr as *mut CAMetalLayer)
                .expect("Couldn't cast pointer to CAMetalLayer!")
        };

        let audio_subsystem = sdl_context.audio().unwrap();

        let spec = AudioSpecDesired {
            freq: Some(44100),
            channels: Some(2),
            samples: Some(512),
        };

        let device = audio_subsystem
            .open_playback(None, &spec, |_| PsxAudioCallback {
                audio_buffer: VecDeque::new(),
            })
            .unwrap();

        device.resume();

        let button_map = HashMap::from([
            (Button::Back, 0),
            (Button::LeftStick, 1),
            (Button::RightStick, 2),
            (Button::Start, 3),
            (Button::DPadUp, 4),
            (Button::DPadRight, 5),
            (Button::DPadDown, 6),
            (Button::DPadLeft, 7),
            (Button::LeftShoulder, 10),
            (Button::RightShoulder, 11),
            (Button::Y, 12),
            (Button::B, 13),
            (Button::A, 14),
            (Button::X, 15),
        ]);

        let button_map2 = HashMap::from([(Axis::TriggerLeft, 8), (Axis::TriggerRight, 9)]);

        let key_map = HashMap::from([
            (Keycode::Tab, 0),
            (Keycode::LShift, 1),
            (Keycode::RShift, 2),
            (Keycode::Return, 3),
            (Keycode::W, 4),
            (Keycode::D, 5),
            (Keycode::S, 6),
            (Keycode::A, 7),
            (Keycode::Num7, 8),
            (Keycode::Num9, 9),
            (Keycode::U, 10),
            (Keycode::O, 11),
            (Keycode::I, 12),
            (Keycode::L, 13),
            (Keycode::K, 14),
            (Keycode::J, 15),
        ]);
        Self {
            #[cfg(feature = "hardware_gpu")]
            _window: window,
            event_pump: sdl_context.event_pump().unwrap(),
            controller,
            game_controller_subsystem,
            #[cfg(feature = "hardware_gpu")]
            renderer: Renderer::new(metal_layer),
            #[cfg(feature = "software_gpu")]
            canvas,
            device,
            button_map,
            button_map2,
            controller_id: None,
            retry_attempts: 0,
            key_map,
        }
    }

    pub fn get_memory_card_path() -> Option<PathBuf> {
        if let Some(mut memory_path) = data_dir() {
            memory_path.push("RSX-redux");
            memory_path.push("memory_card.mcd");

            return Some(memory_path);
        }

        None
    }

    pub fn get_memory_file() -> Option<MmapMut> {
        if let Some(memory_path) = Self::get_memory_card_path() {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(memory_path)
                .unwrap();

            file.set_len(MEMORY_SIZE as u64).unwrap();

            let memory_data = unsafe { MmapMut::map_mut(&file).unwrap() };

            return Some(memory_data);
        }

        None
    }

    fn get_quick_state_path(cpu: &CPU) -> PathBuf {
        #[cfg(feature = "software_gpu")]
        let filename = "quick_save_sw.state";
        #[cfg(feature = "hardware_gpu")]
        let filename = "quick_save_hw.state";

        let game_path = Path::new(&cpu.game_path);

        let game_path_str = game_path
            .file_stem()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();

        let mut split: Vec<&str> = game_path_str.split('/').collect();

        let game_name = split.pop().unwrap();

        let mut dir = data_dir().unwrap();

        dir.push("RSX-redux");
        dir.push(game_name);

        fs::create_dir_all(&dir).expect("Couldn't create save state directory");

        dir.push(filename);

        dir
    }

    fn load_quick_state_inner(cpu: &mut CPU, after_load: impl FnOnce(&mut CPU)) {
        let quick_save_path = Self::get_quick_state_path(cpu);

        if let Ok(compressed) = fs::read(quick_save_path) {
            if let Ok(bytes) = zstd::decode_all(&*compressed) {
                let game_path = cpu.game_path.clone();
                cpu.load_save_state(&bytes);

                let game_file = File::open(&game_path).unwrap();

                let game_data = unsafe { Mmap::map(&game_file).unwrap() };

                cpu.bus.cdrom.load_game_desktop(game_data);
                cpu.reload_instructions();
                cpu.bus.scheduler.deserialize_scheduler();
                cpu.bus
                    .peripherals
                    .memory_card
                    .set_memory_file(Self::get_memory_file());
                after_load(cpu);
            }
        }
    }

    #[cfg(feature = "hardware_gpu")]
    fn load_quick_state(renderer: &mut Renderer, cpu: &mut CPU) {
        Self::load_quick_state_inner(cpu, |cpu| {
            renderer.set_vram_textures(&cpu.bus.gpu.vram_read_tex, &cpu.bus.gpu.vram_write_tex);
        });
    }

    #[cfg(feature = "software_gpu")]
    fn load_quick_state(cpu: &mut CPU) {
        Self::load_quick_state_inner(cpu, |_| {});
    }

    fn create_quick_state_inner(cpu: &mut CPU, before_save: impl Fn(&mut CPU)) {
        cpu.bus.scheduler.serialize_scheduler();
        before_save(cpu);
        let (data, _) = cpu.create_save_state();

        let compressed = zstd::encode_all(&*data, 9).unwrap_or_default();

        if !compressed.is_empty() {
            let quick_save_path = Self::get_quick_state_path(cpu);

            fs::write(quick_save_path, compressed).unwrap();
        }
    }

    #[cfg(feature = "software_gpu")]
    fn create_quick_state(cpu: &mut CPU) {
        Self::create_quick_state_inner(cpu, |_| {});
    }

    #[cfg(feature = "hardware_gpu")]
    fn create_quick_state(renderer: &mut Renderer, cpu: &mut CPU) {
        Self::create_quick_state_inner(cpu, |cpu| {
            let (vram_read, vram_write) = renderer.get_vram_textures();

            cpu.bus.gpu.vram_read_tex = vram_read.into_boxed_slice();
            cpu.bus.gpu.vram_write_tex = vram_write.into_boxed_slice();
        });
    }

    pub fn handle_events(&mut self, cpu: &mut CPU) {
        for event in self.event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => {
                    exit(0);
                }
                Event::KeyDown { keycode, .. } => {
                    if let Some(keycode) = keycode {
                        if let Some(index) = self.key_map.get(&keycode) {
                            cpu.bus.peripherals.controller.update_input(*index, true);
                        } else {
                            match keycode {
                                #[cfg(feature = "debug")]
                                Keycode::G => {
                                    cpu.debug_on = !cpu.debug_on;
                                    println!("setting debug on to {}", cpu.debug_on);
                                }
                                Keycode::F => {
                                    cpu.bus.gpu.debug_on = !cpu.bus.gpu.debug_on;
                                }
                                Keycode::F5 => {
                                    #[cfg(feature = "software_gpu")]
                                    Self::create_quick_state(cpu);
                                    #[cfg(feature = "hardware_gpu")]
                                    Self::create_quick_state(&mut self.renderer, cpu);
                                }
                                Keycode::F7 => {
                                    #[cfg(feature = "software_gpu")]
                                    Self::load_quick_state(cpu);
                                    #[cfg(feature = "hardware_gpu")]
                                    Self::load_quick_state(&mut self.renderer, cpu);
                                }
                                _ => (),
                            }
                        }
                    }
                }
                Event::KeyUp { keycode, .. } => {
                    if let Some(keycode) = keycode {
                        if let Some(index) = self.key_map.get(&keycode) {
                            cpu.bus.peripherals.controller.update_input(*index, false);
                        }
                    }
                }
                Event::ControllerButtonDown { button, .. } => {
                    if let Some(index) = self.button_map.get(&button) {
                        cpu.bus.peripherals.controller.update_input(*index, true);
                    }
                }
                Event::ControllerButtonUp { button, .. } => {
                    if let Some(index) = self.button_map.get(&button) {
                        cpu.bus.peripherals.controller.update_input(*index, false);
                    } else if button == Button::Touchpad {
                        println!(
                            "setting digital mode to {}",
                            !cpu.bus.peripherals.controller.digital_mode
                        );
                        cpu.bus.peripherals.controller.digital_mode =
                            !cpu.bus.peripherals.controller.digital_mode
                    }
                }
                Event::ControllerAxisMotion { axis, value, .. } => {
                    if let Some(index) = self.button_map2.get(&axis) {
                        cpu.bus
                            .peripherals
                            .controller
                            .update_input(*index, value >= 0x3fff);
                    } else {
                        let normalized_value = ((value >> 8) + 128) as u8;
                        let controller = &mut cpu.bus.peripherals.controller;

                        match axis {
                            Axis::LeftX => controller.set_leftx(normalized_value),
                            Axis::LeftY => controller.set_lefty(normalized_value),
                            Axis::RightX => controller.set_rightx(normalized_value),
                            Axis::RightY => controller.set_righty(normalized_value),
                            _ => (),
                        }
                    }
                }
                Event::JoyDeviceAdded { which, .. } => {
                    self.controller = match self.game_controller_subsystem.open(which) {
                        Ok(c) => Some(c),
                        Err(_) => {
                            self.controller_id = Some(which);
                            self.retry_attempts = 0;
                            None
                        }
                    }
                }
                _ => (),
            }
        }
    }

    #[cfg(feature = "software_gpu")]
    pub fn render(&mut self, gpu: &mut GPU) {
        gpu.update_framebuffer();

        let (width, height) = gpu.get_dimensions();

        let creator = self.canvas.texture_creator();
        let mut texture = creator
            .create_texture_target(PixelFormatEnum::RGB24, width, height)
            .unwrap();

        texture
            .update(None, &gpu.picture, width as usize * 3)
            .unwrap();

        self.canvas.copy(&texture, None, None).unwrap();

        self.canvas.present();
    }
}
