use std::panic;

use renderer_webgl::renderer::Renderer;
use rsx_redux::cpu::CPU;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
  ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[wasm_bindgen]
pub struct PsxWebEmulator {
    cpu: CPU,
    memory_bytes: Vec<u8>,
    renderer: Renderer,
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
        }
    }

    pub fn load_bios(&mut self, bios_bytes: &[u8]) {
        self.cpu.bus.load_bios(bios_bytes.to_vec());
    }

    pub fn load_rom(&mut self, game_bytes: &[u8]) {
        self.cpu.bus.cdrom.game_bytes = Some(game_bytes.to_vec());
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
        let game_data = self.cpu.bus.cdrom.game_bytes.clone().unwrap();

        self.cpu.load_save_state(data);

        self.cpu.bus.cdrom.load_game_web(game_data);

        self.cpu.reload_instructions();

        self.cpu.bus.scheduler.deserialize_scheduler();

        self.cpu
            .bus
            .peripherals
            .memory_card
            .set_memory_bytes(self.memory_bytes.clone());

    }

    pub fn save_state(&mut self) -> Vec<u8> {
        self.cpu.bus.scheduler.serialize_scheduler();

        let (data, _) = self.cpu.create_save_state();

        data
    }

    pub fn get_dimensions(&self) -> Vec<u32> {
        let (width, height) = self.cpu.bus.gpu.get_dimensions();

        let vec = vec![width, height];

        vec
    }

    pub fn start_exe(&mut self, path: &str) {
        self.cpu.exe_file = Some(path.to_string());
        self.cpu.load_exe(path);
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

    #[cfg(feature = "software_gpu")]
    pub fn get_framebuffer(&self) -> *const u8 {
        self.cpu.bus.gpu.picture.as_ptr()
    }

    #[cfg(feature = "software_gpu")]
    pub fn get_framebuffer_size(&self) -> usize {
        self.cpu.bus.gpu.picture.len()
    }

    pub fn reset(&mut self) {
        let game_bytes = self.cpu.bus.cdrom.game_bytes.clone().unwrap();
        let bios = self.cpu.bus.get_bios();

        self.cpu = CPU::new(None, "".to_string());

        self.cpu.bus.load_bios(bios);
        self.cpu.bus.cdrom.load_game_web(game_bytes);
    }

    pub fn get_memory_bytes(&mut self) -> Option<Vec<u8>> {
        if self.cpu.bus.peripherals.memory_card.is_memory_dirty() {
            self.cpu.bus.peripherals.memory_card.clear_dirty();
            return self.cpu.bus.peripherals.memory_card.get_memory_bytes();
        }

        None
    }
}