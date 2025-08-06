use std::{env, fs};

use rsx_redux::cpu::CPU;


fn main() {
    let mut cpu = CPU::new();

    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        panic!("syntax: ./psx-redux <path_to_game>");
    }

    let bios = fs::read("SCPH1001.bin").unwrap();

    cpu.bus.load_bios(bios);

    loop {
        cpu.step();
    }
}