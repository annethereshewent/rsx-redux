use std::{collections::VecDeque, sync::Arc};

use ringbuf::{storage::Heap, traits::Producer, wrap::caching::Caching, SharedRb};
use spu_control_register::{SoundRamTransferMode, SpuControlRegister};
use voice::Voice;

use crate::cpu::bus::{registers::interrupt_register::InterruptRegister, scheduler::{EventType, Scheduler}, spu::reverb::Reverb};

pub mod spu_control_register;
pub mod voice;
pub mod reverb;

const SOUND_RAM_SIZE: usize = 0x8_0000;

pub const NUM_SAMPLES: usize = 8192 * 2;

const SPU_CYCLES: usize = 768;

const CAPTURE_SIZE: usize = 0x400;

pub struct SoundRam {
    ram: Box<[u8]>
}

impl SoundRam {
    pub fn new() -> Self {
        Self {
            ram: vec![0; SOUND_RAM_SIZE].into_boxed_slice(),
        }
    }

    pub fn read16(&self, address: usize) -> u16 {
        unsafe { *(&self.ram[address] as *const u8 as *const u16) }
    }

    pub fn read8(&self, address: usize) -> u8 {
        self.ram[address]
    }

    pub fn readf32(&self, address: usize) -> f32 {
        SPU::to_f32(self.read16(address) as i16)
    }

    pub fn write16(&mut self, address: usize, value: u16) {
        unsafe { *(&mut self.ram[address] as *mut u8 as *mut u16) = value };
    }

    pub fn writef32(&mut self, address: usize, value: f32) {
        unsafe { *(&mut self.ram[address] as *mut u8 as *mut u16 ) = SPU::to_i16(value) as u16 };
    }
}

enum CaptureIndexes {
    _CdLeft = 0,
    _CdRight = 1,
    Voice1 = 2,
    Voice3 = 3
}

pub struct SPU {
    reverb: Reverb,
    main_volume_left: u16,
    main_volume_right: u16,
    spucnt: SpuControlRegister,
    sound_modulation: u32,
    keyoff: u32,
    keyon: u32,
    noise_enable: u32,
    echo_on: u32,
    cd_volume: (u16, u16),
    current_volume: (u16, u16),
    external_volume: (u16, u16),
    sound_ram_transfer_type: u16,
    current_ram_address: u32,
    sound_ram_address: u32,
    irq_address: u32,
    voices: [Voice; 24],
    sample_fifo: VecDeque<u16>,
    sound_ram: SoundRam,
    producer: Caching<Arc<SharedRb<Heap<f32>>>, true, false>,
    endx: u32
}

impl SPU {
    pub fn new(producer: Caching<Arc<SharedRb<Heap<f32>>>, true, false>, scheduler: &mut Scheduler) -> Self {
        scheduler.schedule(EventType::TickSpu, SPU_CYCLES);
        Self {
            main_volume_left: 0,
            main_volume_right: 0,
            spucnt: SpuControlRegister::from_bits_retain(0),
            keyoff: 0,
            sound_modulation: 0,
            noise_enable: 0,
            echo_on: 0,
            cd_volume: (0, 0),
            external_volume: (0, 0),
            current_volume: (0, 0),
            sound_ram_transfer_type: 0,
            sound_ram_address: 0,
            irq_address: 0,
            current_ram_address: 0,
            voices: [Voice::new(); 24],
            keyon: 0,
            sample_fifo: VecDeque::new(),
            sound_ram: SoundRam::new(),
            producer,
            endx: 0,
            reverb: Reverb::new()
        }
    }

    fn apply_volume(sample: f32, volume: i16) -> f32 {
        sample * SPU::to_f32(volume)
    }

    pub fn clamp(value: i32, min: i32, max: i32) -> i16 {
        if value < min {
            return min as i16;
        }
        if value > max {
            return max as i16;
        }

        value as i16
    }

    pub fn clampf32(value: f32) -> f32 {
        if value < -1.0 {
            return -1.0;
        }
        if value > 1.0 {
            return 1.0;
        }

        value
    }

    fn to_f32(sample: i16) -> f32 {
        if sample < 0 {
            -sample as f32 / i16::MIN as f32
        } else {
            sample as f32 / i16::MAX as f32
        }
    }

    fn to_i16(sample: f32) -> i16 {
        if sample < 0.0 {
            (-sample * i16::MIN as f32) as i16
        } else {
            (sample * i16::MAX as f32) as i16
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
        self.spucnt.bits() & 0x3f
    }

    pub fn write_voices(&mut self, address: usize, value: u16) {
        let voice = ((address >> 4) & 0x1f) as usize;
        let channel = address & 0xf;

        self.voices[voice as usize].write(channel, value);
    }

    pub fn read_voices(&self, address: usize) -> u16 {
        let voice = ((address >> 4) & 0x1f) as usize;
        let channel = (address - 0x1f801c00) & 0xf;

        self.voices[voice as usize].read(channel)
    }

    pub fn read16(&self, address: usize) -> u16 {
        match address {
            0x1f801c00..=0x1f801d7f => self.read_voices(address),
            0x1f801d88 => self.keyon as u16,
            0x1f801d8a => (self.keyon >> 16) as u16,
            0x1f801d8c => self.keyoff as u16,
            0x1f801d8e => (self.keyoff >> 16) as u16,
            0x1f801da6 => (self.sound_ram_address / 8) as u16,
            0x1f801daa => self.spucnt.bits(),
            0x1f801dac => self.sound_ram_transfer_type,
            0x1f801dae => self.read_stat(),
            0x1f801db8 => self.current_volume.0,
            0x1f801dba => self.current_volume.1,
            _ => todo!("SPU address 0x{:x}", address)
        }
    }

    pub fn write16(&mut self, address: usize, value: u16, interrupt_register: &mut InterruptRegister) {
        match address {
            0x1f801c00..=0x1f801d7f => self.write_voices(address, value),
            0x1f801d80 => self.main_volume_left = value,
            0x1f801d82 => self.main_volume_right = value,
            0x1f801d88 => self.keyon = (self.keyon & 0xffff0000) | value as u32,
            0x1f801d8a => self.keyon = (self.keyon & 0xffff) | (value as u32) << 16,
            0x1f801d8c => self.keyoff = (self.keyoff & 0xffff0000) | value as u32,
            0x1f801d8e => self.keyoff = (self.keyoff & 0xffff) | (value as u32) << 16,
            0x1f801d90 => self.sound_modulation = (self.sound_modulation & 0xffff000) | value as u32,
            0x1f801d92 => self.sound_modulation = (self.sound_modulation & 0xffff) | (value as u32) << 16,
            0x1f801d94 => self.noise_enable = (self.noise_enable & 0xffff000) | value as u32,
            0x1f801d96 => self.noise_enable = (self.noise_enable & 0xffff) | (value as u32) << 16,
            0x1f801d98 => self.echo_on = (self.echo_on & 0xffff000) | value as u32,
            0x1f801d9a => self.echo_on = (self.echo_on & 0xffff) | (value as u32) << 16,
            0x1f801da4 => self.irq_address = value as u32 * 8,
            0x1f801da6 => {
                self.sound_ram_address = value as u32 * 8;
                self.current_ram_address = self.sound_ram_address;

                if self.irq_address == self.current_ram_address && self.spucnt.contains(SpuControlRegister::IRQ9_ENABLE) {
                    interrupt_register.insert(InterruptRegister::SPU);
                }
            }
            0x1f801da8 => self.sample_fifo.push_back(value),
            0x1f801daa => {

                self.spucnt = SpuControlRegister::from_bits_retain(value);

                if self.spucnt.sound_ram_transfer_mode() == SoundRamTransferMode::ManualWrite {
                    while !self.sample_fifo.is_empty() {
                        if self.current_ram_address == self.irq_address && self.spucnt.contains(SpuControlRegister::IRQ9_ENABLE) {
                            interrupt_register.insert(InterruptRegister::SPU);
                        }
                        self.sound_ram.write16(
                            self.current_ram_address as usize,
                            self.sample_fifo.pop_front().unwrap()
                        );
                        self.current_ram_address = (self.current_ram_address + 2) & 0x7_ffff;
                    }
                }
            }
            0x1f801dac => self.sound_ram_transfer_type = value,
            0x1f801db0 => self.cd_volume.0 = value,
            0x1f801db2 => self.cd_volume.1 = value,
            0x1f801db4 => self.external_volume.0 = value,
            0x1f801db6 => self.external_volume.1 = value,
            0x1f801dc0..=0x1f801dfe | 0x1f801d84..=0x1f801d86 | 0x1f801da2 => self.reverb.write16(address, value),
            _ => panic!("invalid address given to control spu control registers: 0x{:x}", address)
        }
    }

    fn update_keystatus(&mut self) {
        if self.keyoff != 0 || self.keyon != 0 {
            for i in 0..self.voices.len() {
                if (self.keyoff >> i) & 1 == 1 {
                    self.voices[i].update_keyoff();
                }

                if (self.keyon >> i) & 1 == 1 {
                    self.endx &= !(1 << i);
                    self.voices[i].update_keyon();
                }
            }

            self.keyoff = 0;
            self.keyon = 0;
        }
    }

    pub fn tick(&mut self, interrupt_register: &mut InterruptRegister, scheduler: &mut Scheduler) {
        let mut left_total: f32 = 0.0;
        let mut right_total: f32 = 0.0;

        let mut reverb_left: f32 = 0.0;
        let mut reverb_right: f32 = 0.0;

        for i in 0..self.voices.len() {
            let previous_out = if i > 0 {
                self.voices[i - 1].last_volume
            } else {
                0
            };
            let voice = &mut self.voices[i];
            let (left, right, endx) = voice.generate_samples(
                &self.sound_ram,
                self.irq_address,
                self.spucnt.contains(SpuControlRegister::IRQ9_ENABLE),
                interrupt_register,
                self.sound_modulation >> i == 1 && i > 0,
                previous_out,
                (self.noise_enable >> i) == 1
            );

            if endx {
                self.endx |= 1 << i;
            }

            left_total += left;
            right_total += right;

            if (self.echo_on >> i) & 1 == 1 {
                reverb_left += left;
                reverb_right += right;
            }
        }

        if self.spucnt.contains(SpuControlRegister::REVERB_MASTER_ENABLE) {
            left_total += self.reverb.reverb_out_left;
            right_total += self.reverb.reverb_out_right;

            if self.reverb.is_left {
                self.reverb.calculate_left(
                    reverb_left,
                    &mut self.sound_ram
                );

            } else {
                self.reverb.calculate_right(
                    reverb_right,
                    &mut self.sound_ram
                );
            }

            self.reverb.is_left = !self.reverb.is_left;
        }

        self.write_to_capture(CaptureIndexes::Voice1 as usize, self.voices[1].last_volume as u16);
        self.write_to_capture(CaptureIndexes::Voice3 as usize, self.voices[3].last_volume as u16);

        self.producer.try_push(SPU::clampf32(left_total)).unwrap_or(());
        self.producer.try_push(SPU::clampf32(right_total)).unwrap_or(());

        self.update_keystatus();

        scheduler.schedule(EventType::TickSpu, SPU_CYCLES);
    }

    fn write_to_capture(&mut self, capture_index: usize, volume: u16) {
        // unsafe { *(&mut self.sound_ram[CAPTURE_SIZE * capture_index] as *mut u8 as *mut u16) = volume };
        self.sound_ram.write16(CAPTURE_SIZE * capture_index, volume)
    }
}