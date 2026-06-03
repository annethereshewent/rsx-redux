use std::{
    ffi::c_void,
    fs::{self, File, OpenOptions},
};

use memmap2::{Mmap, MmapMut};
use objc2::rc::Retained;
use objc2_quartz_core::CAMetalLayer;
#[cfg(feature = "hardware_gpu")]
use renderer_metal::renderer::Renderer;
use rsx_redux::cpu::{bus::peripherals::memory_card::MEMORY_SIZE, CPU};

#[swift_bridge::bridge]
mod ffi {
    extern "Rust" {
        type PsxMacEmulator;

        #[swift_bridge(init)]
        fn new(metal_layer: *mut c_void) -> PsxMacEmulator;

        #[swift_bridge(swift_name = "loadRom")]
        fn load_rom(&mut self, game_path: &str);

        #[swift_bridge(swift_name = "stepFrame")]
        fn step_frame(&mut self);

        #[swift_bridge(swift_name = "drainSamples")]
        fn drain_samples(&mut self) -> Vec<i16>;

        #[swift_bridge(swift_name = "loadBios")]
        fn load_bios(&mut self, bios_path: &str);

        #[swift_bridge(swift_name = "updateInput")]
        fn update_input(&mut self, button: usize, pressed: bool);

        #[swift_bridge(swift_name = "toggleDigitalMode")]
        fn toggle_digital_mode(&mut self);

        #[swift_bridge(swift_name = "setLeftThumbstick")]
        fn set_left_thumbstick(&mut self, normalized_x: u8, normalized_y: u8);

        #[swift_bridge(swift_name = "setRightThumbstick")]
        fn set_right_thumbstick(&mut self, normalized_x: u8, normalized_y: u8);

        #[swift_bridge(swift_name = "setMemoryCard")]
        fn set_memory_card(&mut self, memory_path: &str);
    }
}

pub struct PsxMacEmulator {
    cpu: CPU,
    #[cfg(feature = "hardware_gpu")]
    renderer: Renderer,
}

impl PsxMacEmulator {
    pub fn new(metal_layer_ptr: *mut c_void) -> Self {
        #[cfg(feature = "hardware_gpu")]
        let metal_layer: Retained<CAMetalLayer> = unsafe {
            Retained::from_raw(metal_layer_ptr as *mut CAMetalLayer)
                .expect("Couldn't cast pointer to CAMetalLayer!")
        };
        Self {
            cpu: CPU::new(None, "".to_string()),
            #[cfg(feature = "hardware_gpu")]
            renderer: Renderer::new(metal_layer),
        }
    }

    pub fn load_bios(&mut self, bios_path: &str) {
        let bios_bytes = fs::read(bios_path).unwrap();
        self.cpu.bus.load_bios(bios_bytes);
    }

    pub fn load_rom(&mut self, game_path: &str) {
        let file = File::open(game_path).unwrap();
        self.cpu.bus.cdrom.game_data = unsafe { Some(Mmap::map(&file).unwrap()) };
        self.cpu.game_path = game_path.to_string();
    }

    pub fn step_frame(&mut self) {
        while !self.cpu.bus.gpu.frame_finished {
            self.cpu.step();
            #[cfg(feature = "hardware_gpu")]
            self.renderer.process(&mut self.cpu.bus.gpu);
        }

        self.cpu.bus.gpu.frame_finished = false;

        #[cfg(feature = "software_gpu")]
        self.cpu.bus.gpu.cap_fps();

        #[cfg(feature = "hardware_gpu")]
        self.renderer.present(&mut self.cpu.bus.gpu);
        #[cfg(feature = "software_gpu")]
        self.render(&mut cpu.bus.gpu);

        #[cfg(feature = "hardware_gpu")]
        self.cpu.bus.gpu.cap_fps();
    }

    pub fn drain_samples(&mut self) -> Vec<i16> {
        self.cpu.bus.spu.audio_buffer.drain(..).collect()
    }

    pub fn update_input(&mut self, button: usize, pressed: bool) {
        self.cpu
            .bus
            .peripherals
            .controller
            .update_input(button, pressed);
    }

    pub fn toggle_digital_mode(&mut self) {
        let digital_mode = &mut self.cpu.bus.peripherals.controller.digital_mode;
        *digital_mode = !*digital_mode;
    }

    pub fn set_left_thumbstick(&mut self, normalized_x: u8, normalized_y: u8) {
        self.cpu.bus.peripherals.controller.set_leftx(normalized_x);
        self.cpu.bus.peripherals.controller.set_lefty(normalized_y);
    }

    pub fn set_right_thumbstick(&mut self, normalized_x: u8, normalized_y: u8) {
        self.cpu.bus.peripherals.controller.set_rightx(normalized_x);
        self.cpu.bus.peripherals.controller.set_righty(normalized_y);
    }

    pub fn set_memory_card(&mut self, memory_path: &str) {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(memory_path)
            .unwrap();

        file.set_len(MEMORY_SIZE as u64).unwrap();

        let memory_card = unsafe { Some(MmapMut::map_mut(&file).unwrap()) };
        self.cpu
            .bus
            .peripherals
            .memory_card
            .set_memory_file(memory_card);
    }
}
