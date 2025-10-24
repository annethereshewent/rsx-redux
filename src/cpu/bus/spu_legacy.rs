use std::{collections::VecDeque, sync::Arc};

use ringbuf::{SharedRb, storage::Heap, traits::Producer, wrap::caching::Caching};

use self::{
    reverb::Reverb,
    spu_control::{RamTransferMode, SpuControlRegister},
    voices::Voice,
};
use crate::cpu::bus::{
    registers::interrupt_register::InterruptRegister,
    scheduler::{EventType, Scheduler},
};

pub mod adsr;
pub mod reverb;
pub mod spu_control;
pub mod voices;

pub const FIFO_CAPACITY: usize = 32;
pub const SPU_RAM_SIZE: usize = 0x80000; // 512 kb

pub const NUM_SAMPLES: usize = 32768;

const SPU_CYCLES: usize = 768;

pub struct SoundRam {
    data: Box<[u8]>,
    pub irq_address: u32,
    pub irq: bool,
}

impl SoundRam {
    pub fn new() -> Self {
        Self {
            data: vec![0; SPU_RAM_SIZE].into_boxed_slice(),
            irq_address: 0,
            irq: false,
        }
    }

    pub fn read_16(&mut self, address: u32) -> u16 {
        if address == self.irq_address {
            self.irq = true;
        }

        (self.data[address as usize] as u16) | (self.data[(address + 1) as usize] as u16) << 8
    }

    pub fn write_16(&mut self, address: u32, val: u16) {
        self.data[address as usize] = val as u8;
        self.data[(address + 1) as usize] = ((val >> 8) & 0xff) as u8;

        if address == self.irq_address {
            self.irq = true;
        }
    }
}

struct DataTransfer {
    control: u16,
    transfer_address: u32,
    current_address: u32,
    fifo: Vec<u16>,
}

impl DataTransfer {
    pub fn new() -> Self {
        Self {
            control: 0,
            transfer_address: 0,
            current_address: 0,
            fifo: Vec::with_capacity(FIFO_CAPACITY),
        }
    }
}

pub struct SPU {
    pub audio_buffer: Vec<i16>,
    pub previous_value: i16,
    voices: [Voice; 24],
    volume_left: i16,
    volume_right: i16,
    reverb_volume_left: i16,
    reverb_volume_right: i16,
    external_volume_left: i16,
    external_volume_right: i16,
    current_volume_left: i16,
    current_volume_right: i16,
    cd_volume_left: i16,
    cd_volume_right: i16,
    key_on: u32,
    key_off: u32,
    modulate_on: u32,
    noise_on: u32,
    echo_on: u32,
    control: SpuControlRegister,
    irq_status: bool,
    data_transfer: DataTransfer,
    sound_ram: SoundRam,
    reverb: Reverb,
    endx: u32,
    noise_level: i16,
    noise_timer: isize,
    pub cd_left_buffer: VecDeque<i16>,
    pub cd_right_buffer: VecDeque<i16>,
    capture_index: u32,
    writing_to_capture_half: bool,
    producer: Caching<Arc<SharedRb<Heap<f32>>>, true, false>,
}

pub const CPU_TO_APU_CYCLES: i32 = 768;

impl SPU {
    pub fn new(
        producer: Caching<Arc<SharedRb<Heap<f32>>>, true, false>,
        scheduler: &mut Scheduler,
    ) -> Self {
        scheduler.schedule(EventType::TickSpu, SPU_CYCLES);
        Self {
            voices: [Voice::new(); 24],
            volume_left: 0,
            volume_right: 0,
            reverb_volume_left: 0,
            reverb_volume_right: 0,
            external_volume_left: 0,
            external_volume_right: 0,
            current_volume_left: 0,
            current_volume_right: 0,
            cd_volume_left: 0,
            cd_volume_right: 0,
            key_on: 0,
            key_off: 0,
            noise_on: 0,
            echo_on: 0,
            modulate_on: 0,
            control: SpuControlRegister::new(),
            irq_status: false,
            data_transfer: DataTransfer::new(),
            sound_ram: SoundRam::new(),
            reverb: Reverb::new(),
            endx: 0,
            audio_buffer: Vec::with_capacity(NUM_SAMPLES),
            previous_value: 0,
            noise_level: 1,
            noise_timer: 0,
            cd_left_buffer: VecDeque::new(),
            cd_right_buffer: VecDeque::new(),
            capture_index: 0,
            writing_to_capture_half: false,
            producer,
        }
    }

    fn update_echo(&mut self) {
        for i in 0..self.voices.len() {
            if (self.echo_on >> i) & 0b1 == 1 {
                self.voices[i].update_echo();
            }
        }
    }

    fn update_key_off(&mut self) {
        for i in 0..self.voices.len() {
            if (self.key_off >> i) & 0b1 == 1 {
                self.voices[i].update_key_off();
            }
        }

        self.key_off = 0;
    }

    fn update_key_on(&mut self) {
        for i in 0..self.voices.len() {
            if (self.key_on >> i) & 0b1 == 1 {
                self.voices[i].update_key_on();
            }
        }

        self.key_on = 0;
    }

    fn update_noise(&mut self) {
        for i in 0..self.voices.len() {
            self.voices[i].update_noise((self.noise_on >> i) & 0b1 == 1);
        }
    }

    fn update_endx(&mut self) {
        self.endx = 0;

        for i in 0..self.voices.len() {
            if self.voices[i].endx {
                self.endx |= 1 << i;
            }
        }
    }

    // per https://psx-spx.consoledev.net/soundprocessingunitspu/#spu-noise-generator
    fn tick_noise(&mut self) {
        let noise_step = self.control.noise_frequency_step();

        self.noise_timer -= noise_step as isize;

        let mut parity_bit = (self.noise_level >> 15) & 0b1;

        for i in 12..9 {
            parity_bit ^= (self.noise_level >> i) & 0b1;
        }

        parity_bit ^= 1;

        if self.noise_timer < 0 {
            self.noise_level = self.noise_level * 2 + parity_bit;
            self.noise_timer += 0x2_0000 >> self.control.noise_frequency_shift();

            if self.noise_timer < 0 {
                self.noise_timer += 0x2_0000 >> self.control.noise_frequency_shift();
            }
        }
    }

    fn update_voices(&mut self) {
        self.update_endx();
        self.update_key_off();
        self.update_key_on();
    }

    pub fn tick(&mut self, interrupt_register: &mut InterruptRegister, scheduler: &mut Scheduler) {
        let mut output_left = 0.0;
        let mut output_right = 0.0;

        let mut modulator: i16 = 0;

        let mut left_reverb = 0.0;
        let mut right_reverb = 0.0;

        let mut cd_left = 0.0;
        let mut cd_right = 0.0;

        self.update_voices();
        self.tick_noise();

        if !self.cd_left_buffer.is_empty() {
            cd_left = SPU::to_f32(self.cd_left_buffer.pop_front().unwrap());
        }

        if !self.cd_right_buffer.is_empty() {
            cd_right = SPU::to_f32(self.cd_right_buffer.pop_front().unwrap());
        }

        for i in 0..self.voices.len() {
            let voice = &mut self.voices[i];

            if voice.is_disabled() {
                continue;
            }

            let (sample_left, sample_right) = voice.get_samples(self.noise_level);

            if i == 1 {
                self.sound_ram
                    .write_16(self.capture_index + 0x800, SPU::to_i16(sample_left) as u16);
            }
            if i == 3 {
                self.sound_ram
                    .write_16(self.capture_index + 0xc00, SPU::to_i16(sample_left) as u16);
            }

            output_left += sample_left;
            output_right += sample_right;

            if voice.reverb {
                left_reverb += sample_left;
                right_reverb += sample_right;
            }

            let should_modulate = (self.modulate_on >> i) & 0b1 == 1;

            voice.tick(i > 0 && should_modulate, modulator, &mut self.sound_ram);

            modulator = voice.modulator;
        }

        output_left *= SPU::to_f32(self.volume_left);
        output_right *= SPU::to_f32(self.volume_right);

        if self.control.cd_audio_enable() {
            output_left += cd_left * SPU::to_f32(self.cd_volume_left);
            output_right += cd_right * SPU::to_f32(self.cd_volume_right);
        }

        if self.control.cd_audio_reverb() {
            left_reverb += cd_left * SPU::to_f32(self.cd_volume_left);
            right_reverb += cd_right * SPU::to_f32(self.cd_volume_right);
        }

        if self.control.reverb_master_enable() {
            output_left += self.reverb.left_out * SPU::to_f32(self.reverb_volume_left);
            output_right += self.reverb.right_out * SPU::to_f32(self.reverb_volume_right);

            self.reverb
                .calculate_reverb([left_reverb, right_reverb], &mut self.sound_ram);
        }

        self.sound_ram
            .write_16(self.capture_index, SPU::to_i16(cd_left) as u16);
        self.sound_ram
            .write_16(self.capture_index + 0x400, SPU::to_i16(cd_right) as u16);

        self.capture_index = (self.capture_index + 2) & 0x3ff;
        self.writing_to_capture_half = self.capture_index >= 0x200;

        if self.control.irq9_enable() && self.sound_ram.irq {
            self.sound_ram.irq = false;
            interrupt_register.insert(InterruptRegister::SPU);
        }

        self.producer
            .try_push(SPU::clampf32(output_left))
            .unwrap_or(());
        self.producer
            .try_push(SPU::clampf32(output_right))
            .unwrap_or(());

        scheduler.schedule(EventType::TickSpu, SPU_CYCLES);
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

    fn to_i16(sample: f32) -> i16 {
        if sample >= 0.0 {
            (sample * f32::from(i16::max_value())) as i16
        } else {
            (-sample * f32::from(i16::min_value())) as i16
        }
    }

    pub fn to_f32(value: i16) -> f32 {
        if value >= 0 {
            f32::from(value) / f32::from(i16::max_value())
        } else {
            -f32::from(value) / f32::from(i16::min_value())
        }
    }

    fn push_fifo(&mut self, val: u16) {
        if self.data_transfer.fifo.len() < FIFO_CAPACITY {
            self.data_transfer.fifo.push(val);
        }
    }

    pub fn dma_write(&mut self, value: u32) {
        self.sound_ram
            .write_16(self.data_transfer.current_address, value as u16);
        self.sound_ram
            .write_16(self.data_transfer.current_address + 2, (value >> 16) as u16);

        self.data_transfer.current_address = (self.data_transfer.current_address + 4) & 0x7_ffff;
    }

    pub fn read32(&self, address: usize) -> u32 {
        (self.read16(address) as u32) | (self.read16(address) as u32) << 16
    }

    pub fn read16(&self, address: usize) -> u16 {
        match address {
            0x1f80_1c00..=0x1f80_1d7f => {
                let voice = ((address >> 4) & 0x1f) as usize;
                let offset = address & 0xf;

                self.voices[voice].read16(offset)
            }
            0x1f80_1d80 => self.volume_left as u16,
            0x1f80_1d82 => self.volume_right as u16,
            0x1f80_1d84 => self.reverb_volume_left as u16,
            0x1f80_1d86 => self.reverb_volume_right as u16,
            0x1f80_1d88 => self.key_on as u16,
            0x1f80_1d8a => (self.key_on >> 16) as u16,
            0x1f80_1d8c => self.key_off as u16,
            0x1f80_1d8e => (self.key_off >> 16) as u16,
            0x1f80_1d90 => self.modulate_on as u16,
            0x1f80_1d92 => (self.modulate_on >> 16) as u16,
            0x1f80_1d94 => self.noise_on as u16,
            0x1f80_1d96 => (self.noise_on >> 16) as u16,
            0x1f80_1d98 => self.echo_on as u16,
            0x1f80_1d9a => (self.echo_on >> 16) as u16,
            0x1f80_1d9c => self.endx as u16,
            0x1f80_1d9e => (self.endx >> 16) as u16,
            0x1f80_1da2 => (self.reverb.mbase / 8) as u16,
            0x1f80_1da6 => (self.data_transfer.transfer_address / 8) as u16,
            0x1f80_1daa => self.control.read(),
            0x1f80_1dac => self.data_transfer.control,
            0x1f80_1dae => {
                let control = self.control.read();

                let mut value = (control & 0x20) << 2;
                value |= (self.irq_status as u16) << 6;
                value |= control & 0x3f;
                value |= (self.writing_to_capture_half as u16) << 11;

                value
            }
            0x1f80_1db0 => self.cd_volume_left as u16,
            0x1f80_1db2 => self.cd_volume_right as u16,
            0x1f80_1db4 => self.external_volume_left as u16,
            0x1f80_1db6 => self.external_volume_right as u16,
            0x1f80_1db8 => self.current_volume_left as u16,
            0x1f80_1dba => self.current_volume_right as u16,
            0x1f801e00..=0x1f801fff => 0xffff,
            _ => panic!("reading from unsupported SPU address: {:X}", address),
        }
    }

    pub fn write16(&mut self, address: usize, val: u16) {
        match address {
            0x1f80_1c00..=0x1f80_1d7f => {
                let voice = ((address >> 4) & 0x1f) as usize;
                let offset = address & 0xf;

                self.voices[voice].write16(offset, val);
            }
            0x1f80_1d80 => self.volume_left = val as i16,
            0x1f80_1d82 => self.volume_right = val as i16,
            0x1f80_1d84 => self.reverb_volume_left = val as i16,
            0x1f80_1d86 => self.reverb_volume_right = val as i16,
            0x1f80_1d88 => {
                self.key_on &= 0xffff0000;
                self.key_on |= val as u32;
            }
            0x1f80_1d8a => {
                self.key_on &= 0xffff;
                self.key_on |= (val as u32) << 16;
            }
            0x1f80_1d8c => {
                self.key_off &= 0xffff0000;
                self.key_off |= val as u32;
            }
            0x1f80_1d8e => {
                self.key_off &= 0xffff;
                self.key_off |= (val as u32) << 16;
            }
            0x1f80_1d90 => {
                self.modulate_on &= 0xffff0000;
                self.modulate_on |= val as u32;
            }
            0x1f80_1d92 => {
                self.modulate_on &= 0xffff;
                self.modulate_on |= (val as u32) << 16;
            }
            0x1f80_1d94 => {
                self.noise_on &= 0xffff0000;
                self.noise_on |= val as u32;

                self.update_noise();
            }
            0x1f80_1d96 => {
                self.noise_on &= 0xffff;
                self.noise_on |= (val as u32) << 16;

                self.update_noise();
            }
            0x1f80_1d98 => {
                self.echo_on &= 0xffff0000;
                self.echo_on |= val as u32;

                self.update_echo();
            }
            0x1f80_1d9a => {
                self.echo_on &= 0xffff;
                self.echo_on |= (val as u32) << 16;

                self.update_echo();
            }
            0x1f80_1d9c..=0x1f80_1d9e => (),
            0x1f80_1da2 => self.reverb.write_mbase(val),
            0x1f80_1da4 => self.sound_ram.irq_address = (val as u32) * 8,
            0x1f80_1da6 => {
                self.data_transfer.transfer_address = (val as u32) * 8;
                self.data_transfer.current_address = (val as u32) * 8;
            }
            0x1f80_1da8 => self.push_fifo(val),
            0x1f80_1daa => {
                self.control.write(val);

                if !self.control.irq9_enable() {
                    self.irq_status = false;
                }

                if self.control.transfer_mode() == RamTransferMode::ManualWrite {
                    // do the manual transfer from fifo to sound ram
                    while !self.data_transfer.fifo.is_empty() {
                        let sample = self.data_transfer.fifo.remove(0);

                        let address = self.data_transfer.current_address;

                        self.sound_ram.write_16(address, sample);

                        self.data_transfer.current_address =
                            (self.data_transfer.current_address + 2) & 0x7_ffff;
                    }
                }
            }
            0x1f80_1dac => self.data_transfer.control = val,
            0x1f80_1db0 => self.cd_volume_left = val as i16,
            0x1f80_1db2 => self.cd_volume_right = val as i16,
            0x1f80_1db4 => self.external_volume_left = val as i16,
            0x1f80_1db6 => self.external_volume_right = val as i16,
            0x1f80_1db8 => self.current_volume_left = val as i16,
            0x1f80_1dba => self.current_volume_right = val as i16,
            0x1f80_1dc0..=0x1f80_1dff => self.reverb.write16(address, val),
            _ => panic!("writing to unsupported SPU address: {:X}", address),
        }
    }
}
