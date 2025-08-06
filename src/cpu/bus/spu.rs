use spu_control_register::SpuControlRegister;
use voice::Voice;

pub mod spu_control_register;
pub mod voice;

pub struct SPU {
    pub main_volume_left: u16,
    pub main_volume_right: u16,
    pub reverb_volume_left: u16,
    pub reverb_volume_right: u16,
    pub spucnt: SpuControlRegister,
    pub spustat: u16,
    pub sound_modulation: u32,
    pub keyoff: u32,
    pub keyon: u32,
    pub noise_enable: u32,
    pub echo_on: u32,
    pub cd_volume: (u16, u16),
    pub external_volume: (u16, u16),
    pub sound_ram_transfer: u16,
    pub sound_ram_address: u16,
    pub sample: i16,
    pub voices: [Voice; 24]
}

impl SPU {
    pub fn new() -> Self {
        Self {
            main_volume_left: 0,
            main_volume_right: 0,
            reverb_volume_left: 0,
            reverb_volume_right: 0,
            spucnt: SpuControlRegister::from_bits_retain(0),
            spustat: 0,
            keyoff: 0,
            sound_modulation: 0,
            noise_enable: 0,
            echo_on: 0,
            cd_volume: (0, 0),
            external_volume: (0, 0),
            sound_ram_transfer: 0,
            sound_ram_address: 0,
            sample: 0,
            voices: [Voice::new(); 24],
            keyon: 0
        }
    }
    /*
    15-12 Unknown/Unused (seems to be usually zero)
    11    Writing to First/Second half of Capture Buffers (0=First, 1=Second)
    10    Data Transfer Busy Flag          (0=Ready, 1=Busy)
    9     Data Transfer DMA Read Request   (0=No, 1=Yes)
    8     Data Transfer DMA Write Request  (0=No, 1=Yes)
    7     Data Transfer DMA Read/Write Request ;seems to be same as SPUCNT.Bit5
    6     IRQ9 Flag                        (0=No, 1=Interrupt Request)
    5-0   Current SPU Mode   (same as SPUCNT.Bit5-0, but, applied a bit delayed)
    */
    pub fn read_stat(&self) -> u16 {
        self.spucnt.bits() & 0x1f
    }

    pub fn write_voices(&mut self, address: usize, value: u16) {
        let voice = (address - 0x1f801c00) / 16;
        let channel = (address - 0x1f801c00) & 0xf;

        self.voices[voice as usize].write(channel, value);
    }
}