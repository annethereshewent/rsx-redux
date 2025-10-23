use std::{env, fs::{self, File}};

use frontend::Frontend;
use memmap2::Mmap;
use objc2_core_foundation::CGSize;
use ringbuf::{traits::Split, HeapRb};
use rsx_redux::cpu::CPU;

#[cfg(feature="old_spu")]
use rsx_redux::cpu::bus::spu_legacy::NUM_SAMPLES;
#[cfg(feature="new_spu")]
use rsx_redux::cpu::bus::spu::NUM_SAMPLES;

pub mod frontend;
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

    let game_data = unsafe  { Mmap::map(&file).unwrap() };

    let bios = fs::read("SCPH1001.bin").unwrap();


    let ringbuffer = HeapRb::<f32>::new(NUM_SAMPLES);

    let (producer, consumer) = ringbuffer.split();

    let mut cpu = CPU::new(producer, exe_file);
    cpu.bus.load_bios(bios);
    cpu.bus.cdrom.load_game_desktop(game_data);

    let mut frontend = Frontend::new(&cpu.bus.gpu, consumer);

    unsafe { frontend.renderer.metal_layer.setDrawableSize(CGSize::new(cpu.bus.gpu.display_width as f64, cpu.bus.gpu.display_height as f64)); }

    loop {
        while !cpu.bus.gpu.frame_finished {
            cpu.step();
            frontend.renderer.process(&mut cpu.bus.gpu);
        }

        frontend.renderer.present(&mut cpu.bus.gpu);

        cpu.bus.gpu.frame_finished = false;
        cpu.bus.gpu.cap_fps();

        frontend.handle_events(&mut cpu);
    }
}