use std::{ffi::c_void, fs::File};

use memmap2::Mmap;
use objc2::rc::Retained;
use objc2_quartz_core::CAMetalLayer;
#[cfg(feature = "hardware_gpu")]
use renderer_metal::renderer::Renderer;
use rsx_redux::cpu::CPU;

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
}
