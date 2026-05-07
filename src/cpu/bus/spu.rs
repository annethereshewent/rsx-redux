use std::{array::from_fn, collections::VecDeque, sync::Arc};

use serde::{Deserialize, Serialize};
use spu_control_register::{SoundRamTransferMode, SpuControlRegister};
use voice::Voice;

use crate::cpu::bus::{
    registers::interrupt_register::InterruptRegister,
    scheduler::{EventType, Scheduler},
    spu::{reverb::Reverb, spu_stat_register::SpuStatRegister, voice::AdsrPhase},
};

pub mod reverb;
pub mod spu_control_register;
pub mod spu_stat_register;
pub mod voice;

const SOUND_RAM_SIZE: usize = 0x8_0000;
const FIFO_SIZE: usize = 32;
pub const NUM_SAMPLES: usize = 8192 * 2;

const SPU_CYCLES: usize = 768;
const CAPTURE_SIZE: usize = 0x400;

#[derive(Serialize, Deserialize)]
pub struct SoundRam {
    ram: Box<[u8]>,
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
        unsafe { *(&mut self.ram[address] as *mut u8 as *mut u16) = SPU::to_i16(value) as u16 };
    }
}

#[derive(Serialize, Deserialize)]
enum CaptureIndexes {
    _CdLeft = 0,
    _CdRight = 1,
    Voice1 = 2,
    Voice3 = 3,
}

#[derive(Serialize, Deserialize)]
pub struct SPU {
    reverb: Reverb,
    main_volume_left: u16,
    main_volume_right: u16,
    spucnt: SpuControlRegister,
    spustat: SpuStatRegister,
    sound_modulation: u32,
    keyoff: u32,
    keyon: u32,
    noise_enable: u32,
    echo_on: u32,
    cd_volume: (u16, u16),
    reverb_volume: (u16, u16),
    current_volume: (u16, u16),
    external_volume: (u16, u16),
    sound_ram_transfer_type: u16,
    current_ram_address: u32,
    sound_ram_address: u32,
    irq_address: u32,
    voices: [Voice; 24],
    sample_fifo: VecDeque<u16>,
    sound_ram: SoundRam,
    pub audio_buffer: Vec<i16>,
    endx: u32,
    pub cd_left_samples: VecDeque<i16>,
    pub cd_right_samples: VecDeque<i16>,
    capture_buffer_pos: u16,
}

impl SPU {
    pub fn new(scheduler: &mut Scheduler) -> Self {
        scheduler.schedule(EventType::TickSpu, SPU_CYCLES);
        Self {
            main_volume_left: 0,
            main_volume_right: 0,
            spucnt: SpuControlRegister::from_bits_retain(0),
            spustat: SpuStatRegister::from_bits_retain(0),
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
            voices: from_fn(|index| Voice::new(index)),
            keyon: 0,
            sample_fifo: VecDeque::new(),
            sound_ram: SoundRam::new(),
            endx: 0,
            reverb: Reverb::new(),
            audio_buffer: Vec::with_capacity(NUM_SAMPLES),
            cd_left_samples: VecDeque::new(),
            cd_right_samples: VecDeque::new(),
            reverb_volume: (0, 0),
            capture_buffer_pos: 0,
        }
    }

    fn apply_volume(sample: i32, volume: i32) -> i32 {
        (sample * volume) >> 15
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

    fn execute_transfer(&mut self, interrupt_register: &mut InterruptRegister) {
        if self.spucnt.sound_ram_transfer_mode() == SoundRamTransferMode::DMARead {
            while self.sample_fifo.len() < FIFO_SIZE {
                let value = self.sound_ram.read16(self.current_ram_address as usize);

                self.current_ram_address += 2;

                self.sample_fifo.push_back(value);

                if self.is_irq_triggerable() && self.current_ram_address == self.irq_address {
                    interrupt_register.insert(InterruptRegister::SPU);
                    self.spustat.insert(SpuStatRegister::IRQ9_FLAG);
                }

                self.update_dma_request();
            }

            self.spustat.remove(SpuStatRegister::DMA_TRANSFER_BUSY);
        } else {
            while let Some(value) = self.sample_fifo.pop_back() {
                self.sound_ram
                    .write16(self.current_ram_address as usize, value);

                self.current_ram_address += 2;

                if self.is_irq_triggerable() && self.irq_address == self.current_ram_address {
                    interrupt_register.insert(InterruptRegister::SPU);
                    self.spustat.insert(SpuStatRegister::IRQ9_FLAG);
                }

                self.update_dma_request();
            }

            self.spustat.remove(SpuStatRegister::DMA_TRANSFER_BUSY);
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
        self.spustat.bits()
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

    pub fn dma_write(&mut self, value: u32) {
        self.sample_fifo.push_back(value as u16);
        self.sample_fifo.push_back((value >> 16) as u16);
    }

    fn manual_transfer_write(&mut self, value: u16, interrupt_register: &mut InterruptRegister) {
        let sound_transfer_mode = self.spucnt.sound_ram_transfer_mode();
        if !self.sample_fifo.is_empty()
            && sound_transfer_mode != SoundRamTransferMode::DMARead
            && sound_transfer_mode != SoundRamTransferMode::Stop
        {
            self.execute_transfer(interrupt_register);
        }

        self.sound_ram
            .write16(self.current_ram_address as usize, value);

        self.current_ram_address += 2;

        if self.is_irq_triggerable() && self.current_ram_address == self.irq_address {
            interrupt_register.insert(InterruptRegister::SPU);
            self.spustat.insert(SpuStatRegister::IRQ9_FLAG);
        }
    }

    pub fn read16(&self, address: usize) -> u16 {
        match address {
            0x1f801c00..=0x1f801d7f => self.read_voices(address),
            0x1f80_1d84 => self.reverb_volume.0,
            0x1f80_1d86 => self.reverb_volume.1,
            0x1f801d88 => self.keyon as u16,
            0x1f801d8a => (self.keyon >> 16) as u16,
            0x1f801d8c => self.keyoff as u16,
            0x1f801d8e => (self.keyoff >> 16) as u16,
            0x1f80_1d90 => self.sound_modulation as u16,
            0x1f80_1d92 => (self.sound_modulation >> 16) as u16,
            0x1f80_1d94 => self.noise_enable as u16,
            0x1f80_1d96 => (self.noise_enable >> 16) as u16,
            0x1f80_1d98 => self.echo_on as u16,
            0x1f80_1d9a => (self.echo_on >> 16) as u16,
            0x1f801da6 => (self.sound_ram_address / 8) as u16,
            0x1f801daa => self.spucnt.bits(),
            0x1f801dac => self.sound_ram_transfer_type,
            0x1f801dae => self.read_stat(),
            0x1f801db8 => self.current_volume.0,
            0x1f801dba => self.current_volume.1,
            _ => todo!("SPU address 0x{:x}", address),
        }
    }

    pub fn update_dma_request(&mut self) {
        match self.spucnt.sound_ram_transfer_mode() {
            SoundRamTransferMode::DMARead => {
                self.spustat.set(
                    SpuStatRegister::DMA_READ_REQUEST,
                    self.sample_fifo.len() == FIFO_SIZE,
                );
                self.spustat.remove(SpuStatRegister::DMA_WRITE_REQUEST);
                self.spustat.set(
                    SpuStatRegister::DMA_REQUEST_BIT,
                    self.sample_fifo.len() == FIFO_SIZE,
                );
            }
            SoundRamTransferMode::DMAWrite => {
                self.spustat.remove(SpuStatRegister::DMA_READ_REQUEST);
                self.spustat.set(
                    SpuStatRegister::DMA_WRITE_REQUEST,
                    self.sample_fifo.is_empty(),
                );
                self.spustat.set(
                    SpuStatRegister::DMA_REQUEST_BIT,
                    self.sample_fifo.is_empty(),
                );
            }
            SoundRamTransferMode::ManualWrite | SoundRamTransferMode::Stop => {
                self.spustat.remove(SpuStatRegister::DMA_READ_REQUEST);
                self.spustat.remove(SpuStatRegister::DMA_WRITE_REQUEST);
                self.spustat.remove(SpuStatRegister::DMA_REQUEST_BIT);
            }
        }
    }

    pub fn write16(
        &mut self,
        address: usize,
        value: u16,
        interrupt_register: &mut InterruptRegister,
    ) {
        match address {
            0x1f801c00..=0x1f801d7f => self.write_voices(address, value),
            0x1f801d80 => self.main_volume_left = value,
            0x1f801d82 => self.main_volume_right = value,
            0x1f801d84 => self.reverb_volume.0 = value,
            0x1f801d86 => self.reverb_volume.1 = value,
            0x1f801d88 => self.keyon = (self.keyon & 0xffff0000) | value as u32,
            0x1f801d8a => self.keyon = (self.keyon & 0xffff) | (value as u32) << 16,
            0x1f801d8c => self.keyoff = (self.keyoff & 0xffff0000) | value as u32,
            0x1f801d8e => self.keyoff = (self.keyoff & 0xffff) | (value as u32) << 16,
            0x1f801d90 => {
                self.sound_modulation = (self.sound_modulation & 0xffff000) | value as u32
            }
            0x1f801d92 => {
                self.sound_modulation = (self.sound_modulation & 0xffff) | (value as u32) << 16
            }
            0x1f801d94 => self.noise_enable = (self.noise_enable & 0xffff000) | value as u32,
            0x1f801d96 => self.noise_enable = (self.noise_enable & 0xffff) | (value as u32) << 16,
            0x1f801d98 => self.echo_on = (self.echo_on & 0xffff000) | value as u32,
            0x1f801d9a => self.echo_on = (self.echo_on & 0xffff) | (value as u32) << 16,
            0x1f801da4 => {
                self.irq_address = value as u32 * 8;

                if self.is_irq_triggerable() && self.irq_address == self.current_ram_address {
                    interrupt_register.insert(InterruptRegister::SPU);
                    self.spustat.insert(SpuStatRegister::IRQ9_FLAG);
                }
            }
            0x1f801da6 => {
                self.sound_ram_address = value as u32 * 8;
                self.current_ram_address = self.sound_ram_address;

                if self.irq_address == self.current_ram_address && self.is_irq_triggerable() {
                    self.spustat.insert(SpuStatRegister::IRQ9_FLAG);
                    interrupt_register.insert(InterruptRegister::SPU);
                }
            }
            0x1f801da8 => self.manual_transfer_write(value, interrupt_register),
            0x1f801daa => {
                let previous_enable = self.spucnt.contains(SpuControlRegister::SPU_ENABLE);
                self.spucnt = SpuControlRegister::from_bits_retain(value);

                if self.spucnt.sound_ram_transfer_mode() == SoundRamTransferMode::ManualWrite {
                    while !self.sample_fifo.is_empty() {
                        if self.current_ram_address == self.irq_address && self.is_irq_triggerable()
                        {
                            self.spustat.insert(SpuStatRegister::IRQ9_FLAG);
                            interrupt_register.insert(InterruptRegister::SPU);
                        }
                        self.sound_ram.write16(
                            self.current_ram_address as usize,
                            self.sample_fifo.pop_front().unwrap(),
                        );
                        self.current_ram_address = (self.current_ram_address + 2) & 0x7_ffff;
                    }
                }

                if previous_enable && !self.spucnt.contains(SpuControlRegister::SPU_ENABLE) {
                    for voice in &mut self.voices {
                        voice.force_off();
                    }
                }

                let mode = self.spucnt.bits() & 0x3f;

                self.spustat =
                    SpuStatRegister::from_bits_retain((self.spustat.bits() & !0x3f) | mode);

                if !self.spucnt.contains(SpuControlRegister::IRQ9_ENABLE) {
                    self.spustat.remove(SpuStatRegister::IRQ9_FLAG);
                } else if self.is_irq_triggerable() && self.irq_address == self.current_ram_address
                {
                    self.spustat.insert(SpuStatRegister::IRQ9_FLAG);
                    interrupt_register.insert(InterruptRegister::SPU);
                }

                self.update_dma_request();
            }
            0x1f801dac => self.sound_ram_transfer_type = value,
            0x1f801db0 => self.cd_volume.0 = value,
            0x1f801db2 => self.cd_volume.1 = value,
            0x1f801db4 => self.external_volume.0 = value,
            0x1f801db6 => self.external_volume.1 = value,
            0x1f801dc0..=0x1f801dfe | 0x1f801da2 => self.reverb.write16(address, value),
            _ => panic!(
                "invalid address given to control spu control registers: 0x{:x}",
                address
            ),
        }
    }

    fn is_irq_triggerable(&self) -> bool {
        self.spucnt.contains(SpuControlRegister::IRQ9_ENABLE)
            && !self.spustat.contains(SpuStatRegister::IRQ9_FLAG)
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
        let mut output_left = 0;
        let mut output_right = 0;
        let mut reverb_in_left = 0;
        let mut reverb_in_right = 0;
        let mut cd_audio_left = 0;
        let mut cd_audio_right = 0;

        for i in 0..self.voices.len() {
            let previous_out = if i > 0 {
                self.voices[i - 1].last_volume
            } else {
                0
            };

            let voice = &mut self.voices[i];

            let (left, right) = voice.generate_samples(
                &self.sound_ram,
                &mut self.endx,
                self.irq_address,
                self.spucnt.contains(SpuControlRegister::IRQ9_ENABLE),
                self.spustat.contains(SpuStatRegister::IRQ9_FLAG),
                interrupt_register,
                &mut self.spustat,
                (self.sound_modulation >> i) & 1 == 1 && i > 0,
                previous_out,
                (self.noise_enable >> i) & 1 == 1,
            );

            output_left += left;
            output_right += right;

            if (self.echo_on >> i) & 1 == 1 {
                reverb_in_left += left;
                reverb_in_right += right;
            }
        }

        if self.spucnt.contains(SpuControlRegister::MUTE_SPU) {
            output_left = 0;
            output_right = 0;
            reverb_in_left = 0;
            reverb_in_right = 0;
        }

        if self.spucnt.contains(SpuControlRegister::CD_AUDIO_ENABLE) {
            if let Some(cd_left_sample) = self.cd_left_samples.pop_front() {
                cd_audio_left = cd_left_sample;
                let cd_volume_left =
                    Self::apply_volume(cd_left_sample as i32, self.cd_volume.0 as i32);

                if self.spucnt.contains(SpuControlRegister::CD_AUDIO_REVERB) {
                    reverb_in_left += cd_volume_left;
                }

                output_left += cd_volume_left;
            }
            if let Some(cd_right_sample) = self.cd_right_samples.pop_front() {
                cd_audio_right = cd_right_sample;
                let cd_volume_right =
                    Self::apply_volume(cd_right_sample as i32, self.cd_volume.1 as i32);

                if self.spucnt.contains(SpuControlRegister::CD_AUDIO_REVERB) {
                    reverb_in_right += cd_volume_right;
                }

                output_right += cd_volume_right;
            }
        }

        if self
            .spucnt
            .contains(SpuControlRegister::REVERB_MASTER_ENABLE)
        {
            output_left += self.reverb.left_out;
            output_right += self.reverb.right_out;

            if self.reverb.calculate_left {
                self.reverb
                    .calculate_left_reverb(&mut self.sound_ram, reverb_in_left);
            } else {
                self.reverb
                    .calculate_right_reverb(&mut self.sound_ram, reverb_in_right);
            }

            self.reverb.calculate_left = !self.reverb.calculate_left;
        }

        output_left = Self::apply_volume(output_left, self.main_volume_left as i16 as i32);
        output_right = Self::apply_volume(output_right, self.main_volume_right as i16 as i32);

        self.write_to_capture(0, cd_audio_left as u16, interrupt_register);
        self.write_to_capture(1, cd_audio_right as u16, interrupt_register);
        self.write_to_capture(
            2,
            self.voices[1].last_volume.clamp(-0x8000, 0x7fff) as u16,
            interrupt_register,
        );
        self.write_to_capture(
            3,
            self.voices[3].last_volume.clamp(-0x8000, 0x7fff) as u16,
            interrupt_register,
        );
        self.increment_capture_buffer_address();

        self.push_sample(output_left as i16);
        self.push_sample(output_right as i16);

        scheduler.schedule(EventType::TickSpu, SPU_CYCLES);
    }

    fn increment_capture_buffer_address(&mut self) {
        self.capture_buffer_pos += 2;
        self.capture_buffer_pos %= CAPTURE_SIZE as u16;

        self.spustat.set(
            SpuStatRegister::CAPTURE_BUFFER_ID,
            self.capture_buffer_pos >= (CAPTURE_SIZE as u16 / 2),
        );
    }

    fn push_sample(&mut self, sample: i16) {
        if self.audio_buffer.len() < NUM_SAMPLES {
            self.audio_buffer.push(sample);
        }
    }

    fn write_to_capture(
        &mut self,
        capture_index: usize,
        volume: u16,
        interrupt_register: &mut InterruptRegister,
    ) {
        let ram_address = (capture_index * CAPTURE_SIZE) | self.capture_buffer_pos as usize;
        self.sound_ram.write16(ram_address as usize, volume);

        if self.is_irq_triggerable() && self.irq_address == ram_address as u32 {
            interrupt_register.insert(InterruptRegister::SPU);
            self.spustat.insert(SpuStatRegister::IRQ9_FLAG);
        }
    }
}
