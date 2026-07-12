use std::panic;

use renderer_webgl::renderer::Renderer;
use rsx_redux::cpu::CPU;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[allow(unused_macros)]
macro_rules! console_log {
  ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[wasm_bindgen]
pub struct PsxWebEmulator {
    cpu: CPU,
    memory_bytes: Vec<u8>,
    renderer: Renderer,
    canvas_id: String,
}

#[wasm_bindgen]
impl PsxWebEmulator {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas_id: &str) -> Self {
        panic::set_hook(Box::new(console_error_panic_hook::hook));

        Self {
            cpu: CPU::new(None, "".to_string()),
            memory_bytes: Vec::new(),
            renderer: Renderer::new(canvas_id),
            canvas_id: canvas_id.to_string(),
        }
    }

    pub fn load_bios(&mut self, bios_bytes: &[u8]) {
        self.cpu.bus.load_bios(bios_bytes.to_vec());
    }

    pub fn load_rom(&mut self, game_bytes: &[u8]) {
        self.cpu.bus.cdrom.game_bytes = Some(game_bytes.to_vec());
    }

    pub fn parse_cue(&mut self, cue_file_contents: &str) {
        self.cpu.bus.cdrom.parse_cue(cue_file_contents.to_string());
    }

    pub fn add_bin_file(&mut self, filename: &str, contents: &[u8]) {
        self.cpu.bus.cdrom.add_bin_file(filename, contents);
    }

    pub fn step_frame(&mut self) {
        self.renderer.clear_color();
        while !self.cpu.bus.gpu.frame_finished {
            self.cpu.step();
            self.renderer.process(&mut self.cpu.bus.gpu);
        }

        self.cpu.bus.gpu.frame_finished = false;

        self.renderer.present(&mut self.cpu.bus.gpu);

        #[cfg(feature = "software_gpu")]
        self.cpu.bus.gpu.update_framebuffer();
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

    pub fn set_memory_card(&mut self, memory_bytes: &[u8]) {
        self.memory_bytes = memory_bytes.to_vec();

        self.cpu
            .bus
            .peripherals
            .memory_card
            .set_memory_bytes(self.memory_bytes.clone());
    }

    pub fn load_state(&mut self, data: &[u8]) {
        if let Some(game_data) = self.cpu.bus.cdrom.game_bytes.clone() {
            self.cpu.load_save_state(data);

            self.cpu.bus.cdrom.load_game_web(game_data.clone());
        } else if self.cpu.bus.cdrom.bin_files.len() > 0 {
            let bin_files = self.cpu.bus.cdrom.bin_files.clone();
            let tracks = self.cpu.bus.cdrom.tracks.clone();

            self.cpu.load_save_state(data);

            self.cpu.bus.cdrom.tracks = tracks.to_vec();
            self.cpu.bus.cdrom.bin_files = bin_files;
        }

        self.cpu.reload_instructions();

        self.cpu.bus.scheduler.deserialize_scheduler();

        #[cfg(feature = "hardware_gpu_web")]
        {
            let rgba8_bytes = self.cpu.bus.gpu.vram_write_tex.to_vec();
            let rgba16_bytes = self.cpu.bus.gpu.vram_read_tex.to_vec();
            self.renderer.set_vram_textures(rgba8_bytes, rgba16_bytes);
        }

        self.cpu
            .bus
            .peripherals
            .memory_card
            .set_memory_bytes(self.memory_bytes.clone());
    }

    pub fn save_state(&mut self) -> Vec<u8> {
        self.cpu.bus.scheduler.serialize_scheduler();

        #[cfg(feature = "hardware_gpu_web")]
        {
            let (vram_write_tex, vram_read_tex) = self.renderer.get_vram_textures();

            self.cpu.bus.gpu.vram_write_tex = vram_write_tex.into_boxed_slice();
            self.cpu.bus.gpu.vram_read_tex = vram_read_tex.into_boxed_slice();
        }

        let (data, _) = self.cpu.create_save_state();

        data
    }

    pub fn get_dimensions(&self) -> Vec<u32> {
        let (width, height) = self.cpu.bus.gpu.get_dimensions();

        let vec = vec![width, height];

        vec
    }

    pub fn set_exe(&mut self, exe_bytes: Option<Vec<u8>>) {
        self.cpu.exe_bytes = exe_bytes;
    }

    pub fn get_rumble(&self) -> Vec<u8> {
        let (small_motor, large_motor) = self.cpu.bus.peripherals.controller.get_rumble();

        let vec = vec![small_motor as u8, large_motor];

        vec
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

    pub fn close_shell(&mut self) {
        self.cpu.bus.cdrom.close_shell();
    }

    pub fn open_shell(&mut self) {
        self.cpu
            .bus
            .cdrom
            .open_shell(&mut self.cpu.bus.interrupt_stat);
    }

    #[cfg(feature = "software_gpu")]
    pub fn get_framebuffer(&self) -> *const u8 {
        self.cpu.bus.gpu.picture.as_ptr()
    }

    #[cfg(feature = "software_gpu")]
    pub fn get_framebuffer_size(&self) -> usize {
        self.cpu.bus.gpu.picture.len()
    }

    pub fn reset(&mut self) {
        let game_bytes = self.cpu.bus.cdrom.game_bytes.clone();
        let exe_bytes = self.cpu.exe_bytes.clone();
        let bios = self.cpu.bus.get_bios();

        #[cfg(feature = "hardware_gpu_web")]
        {
            self.renderer = Renderer::new(&self.canvas_id);
        }

        self.cpu = CPU::new(exe_bytes, "".to_string());

        self.cpu.bus.load_bios(bios);

        if let Some(game_bytes) = game_bytes {
            self.cpu.bus.cdrom.load_game_web(game_bytes);
        }
    }

    pub fn set_port(&mut self, port: u8) {
        self.cpu.bus.peripherals.selected_controller = port;
    }

    pub fn get_memory_bytes(&mut self) -> Option<Vec<u8>> {
        if self.cpu.bus.peripherals.memory_card.is_memory_dirty() {
            self.cpu.bus.peripherals.memory_card.clear_dirty();
            return self.cpu.bus.peripherals.memory_card.get_memory_bytes();
        }

        None
    }
}
