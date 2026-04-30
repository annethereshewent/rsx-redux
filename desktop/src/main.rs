use std::{
    env,
    fs::{self, File},
};

use frontend::Frontend;
use memmap2::Mmap;
#[cfg(feature = "hardware_gpu")]
use objc2_core_foundation::CGSize;
use rsx_redux::cpu::CPU;

pub mod frontend;
#[cfg(feature = "hardware_gpu")]
pub mod renderer;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        panic!("syntax: ./psx-redux <path_to_game>");
    }

    let file = File::open(&args[1]).unwrap();

    let mut exe_file: Option<String> = None;

    if args.len() >= 3 {
        exe_file = Some(args[2].to_string());
    }

    let game_data = unsafe { Mmap::map(&file).unwrap() };

    let bios = fs::read("SCPH1001.bin").unwrap();

    let mut cpu = CPU::new(exe_file);
    cpu.bus.load_bios(bios);
    cpu.bus.cdrom.load_game_desktop(game_data);

    let mut frontend = Frontend::new(&cpu.bus.gpu);

    #[cfg(feature = "hardware_gpu")]
    frontend.renderer.metal_layer.setDrawableSize(CGSize::new(
        cpu.bus.gpu.display_width as f64,
        cpu.bus.gpu.display_height as f64,
    ));

    loop {
        while !cpu.bus.gpu.frame_finished {
            cpu.step();
            #[cfg(feature = "hardware_gpu")]
            frontend.renderer.process(&mut cpu.bus.gpu);
        }
        cpu.bus.gpu.frame_finished = false;

        #[cfg(feature = "software_gpu")]
        cpu.bus.gpu.cap_fps();

        #[cfg(feature = "hardware_gpu")]
        frontend.renderer.present(&mut cpu.bus.gpu);
        #[cfg(feature = "software_gpu")]
        frontend.render(&mut cpu.bus.gpu);

        #[cfg(feature = "hardware_gpu")]
        cpu.bus.gpu.cap_fps();

        frontend.handle_events(&mut cpu);
        frontend.check_controller_status();
        frontend.push_samples(cpu.bus.spu.audio_buffer.drain(..).collect());
    }
}
