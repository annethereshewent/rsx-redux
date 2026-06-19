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

        #[swift_bridge(swift_name = "saveState")]
        fn save_state(&mut self) -> Vec<u8>;

        #[swift_bridge(swift_name = "loadState")]
        fn load_state(&mut self, data: &[u8]);

        #[swift_bridge(swift_name = "getScreenshot")]
        fn get_screenshot(&self) -> Vec<u8>;

        #[swift_bridge(swift_name = "getDimensions")]
        fn get_dimensions(&self) -> (u32, u32);

        #[swift_bridge(swift_name = "startExe")]
        fn start_exe(&mut self, path: &str);

        #[swift_bridge(swift_name = "getRumble")]
        fn get_rumble(&self) -> (bool, u8);

        #[swift_bridge(swift_name = "getDigitalMode")]
        fn get_digital_mode(&self) -> bool;

        #[swift_bridge(swift_name = "setDigitalMode")]
        fn set_digital_mode(&mut self, mode: bool);

        #[swift_bridge(swift_name = "setLeftX")]
        fn set_left_x(&mut self, value: u8);

        #[swift_bridge(swift_name = "setLeftY")]
        fn set_left_y(&mut self, value: u8);

        #[swift_bridge(swift_name = "switchSelectedController")]
        fn switch_selected_controller(&mut self, controller_id: u8);

        #[swift_bridge(swift_name = "closeShell")]
        fn close_shell(&mut self, path: &str);

        #[swift_bridge(swift_name = "openShell")]
        fn open_shell(&mut self);
    }
}

pub struct PsxMacEmulator {
    cpu: CPU,
    #[cfg(feature = "hardware_gpu")]
    renderer: Renderer,
    memory_path: String,
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
            memory_path: "".to_string(),
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

    pub fn set_left_x(&mut self, value: u8) {
        self.cpu.bus.peripherals.controller.set_leftx(value);
    }

    pub fn set_left_y(&mut self, value: u8) {
        self.cpu.bus.peripherals.controller.set_lefty(value);
    }

    pub fn set_memory_card(&mut self, memory_path: &str) {
        self.memory_path = memory_path.to_string();

        let memory_card = Some(Self::get_memory_mmap(memory_path));
        self.cpu
            .bus
            .peripherals
            .memory_card
            .set_memory_file(memory_card);
    }

    fn get_memory_mmap(memory_path: &str) -> MmapMut {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(memory_path)
            .unwrap();

        file.set_len(MEMORY_SIZE as u64).unwrap();

        unsafe { MmapMut::map_mut(&file).unwrap() }
    }

    pub fn load_state(&mut self, data: &[u8]) {
        if let Ok(bytes) = zstd::decode_all(&*data) {
            self.cpu.load_save_state(&bytes);

            let game_file = File::open(&self.cpu.game_path).unwrap();

            let game_data = unsafe { Mmap::map(&game_file).unwrap() };

            self.cpu.bus.cdrom.load_game_desktop(game_data);

            self.cpu.reload_instructions();

            self.cpu.bus.scheduler.deserialize_scheduler();

            self.cpu
                .bus
                .peripherals
                .memory_card
                .set_memory_file(Some(Self::get_memory_mmap(&self.memory_path)));

            self.renderer.set_vram_textures(
                &self.cpu.bus.gpu.vram_read_tex,
                &self.cpu.bus.gpu.vram_write_tex,
            );
        }
    }

    pub fn save_state(&mut self) -> Vec<u8> {
        self.cpu.bus.scheduler.serialize_scheduler();
        let (vram_read, vram_write) = self.renderer.get_vram_textures();

        self.cpu.bus.gpu.vram_read_tex = vram_read.into_boxed_slice();
        self.cpu.bus.gpu.vram_write_tex = vram_write.into_boxed_slice();

        let (data, _) = self.cpu.create_save_state();

        let compressed = zstd::encode_all(&*data, 9).unwrap_or_default();

        compressed
    }

    pub fn get_screenshot(&self) -> Vec<u8> {
        let data = self
            .renderer
            .get_current_screenshot_bytes(&self.cpu.bus.gpu);

        data
    }

    pub fn get_dimensions(&self) -> (u32, u32) {
        self.cpu.bus.gpu.get_dimensions()
    }

    pub fn start_exe(&mut self, path: &str) {
        let exe_bytes = fs::read(path).unwrap();
        self.cpu.exe_bytes = Some(exe_bytes);
    }

    pub fn get_rumble(&self) -> (bool, u8) {
        self.cpu.bus.peripherals.controller.get_rumble()
    }

    pub fn get_digital_mode(&self) -> bool {
        self.cpu.bus.peripherals.controller.digital_mode
    }

    pub fn set_digital_mode(&mut self, mode: bool) {
        self.cpu.bus.peripherals.controller.digital_mode = mode;
    }

    pub fn switch_selected_controller(&mut self, controller_id: u8) {
        self.cpu.bus.peripherals.selected_controller = controller_id;
    }

    pub fn close_shell(&mut self, path: &str) {
        self.load_rom(path);
        self.cpu.bus.cdrom.close_shell();
    }

    pub fn open_shell(&mut self) {
        self.cpu.bus.cdrom.open_shell(&mut self.cpu.bus.interrupt_stat);
    }
}
