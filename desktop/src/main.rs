use std::{
    env,
    fs::{self, File},
    path::Path,
};

use frontend::Frontend;
use memmap2::Mmap;
#[cfg(feature = "hardware_gpu_metal")]
use objc2_core_foundation::CGSize;
use rsx_redux::cpu::CPU;

pub mod frontend;

// TODO: fix using unsafe for type coersion (ie reading a u16 from a byte array) to use std::ptr::read_unaligned

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        panic!("syntax: ./psx-redux <path_to_game/exe>");
    }

    let file_path = Path::new(&args[1]);
    let bios = fs::read("SCPH1001.bin").unwrap();

    let file_extension = file_path
        .extension()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default();

    let mut cpu = if file_extension == "exe" {
        let exe_bytes = fs::read(&args[1]).unwrap();
        CPU::new(Some(exe_bytes), "".to_string())
    } else {
        let file = File::open(file_path).unwrap();
        let game_data = unsafe { Mmap::map(&file).unwrap() };

        let mut cpu = CPU::new(None, args[1].to_string());
        cpu.bus.cdrom.load_game_desktop(game_data);

        cpu
    };

    cpu.bus.load_bios(bios);
    cpu.bus
        .peripherals
        .memory_card
        .set_memory_file(Frontend::get_memory_file());

    let mut frontend = Frontend::new(&cpu.bus.gpu);

    #[cfg(feature = "hardware_gpu_metal")]
    frontend.renderer.metal_layer.setDrawableSize(CGSize::new(
        cpu.bus.gpu.display_width as f64,
        cpu.bus.gpu.display_height as f64,
    ));

    loop {
        while !cpu.bus.gpu.frame_finished {
            cpu.step();
            #[cfg(feature = "hardware_gpu_metal")]
            frontend.renderer.process(&mut cpu.bus.gpu);
        }

        cpu.bus.gpu.frame_finished = false;

        #[cfg(feature = "software_gpu")]
        cpu.bus.gpu.cap_fps();

        #[cfg(feature = "hardware_gpu_metal")]
        frontend.renderer.present(&mut cpu.bus.gpu);
        #[cfg(feature = "software_gpu")]
        frontend.render(&mut cpu.bus.gpu);

        #[cfg(feature = "hardware_gpu_metal")]
        cpu.bus.gpu.cap_fps();

        frontend.handle_events(&mut cpu);
        frontend.check_controller_status();
        frontend.push_samples(cpu.bus.spu.audio_buffer.drain(..).collect());
    }
}
