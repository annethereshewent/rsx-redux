use std::{collections::VecDeque, sync::Arc};

use ringbuf::{storage::Heap, traits::Producer, wrap::caching::Caching, SharedRb};
use spu_control_register::{SoundRamTransferMode, SpuControlRegister};
use voice::Voice;

use crate::cpu::bus::{registers::interrupt_register::InterruptRegister, scheduler::{EventType, Scheduler}};

pub mod spu_control_register;
pub mod voice;

const SOUND_RAM_SIZE: usize = 0x8_0000;

pub const NUM_SAMPLES: usize = 8192 * 2;

const SPU_CYCLES: usize = 768;

pub struct SPU {
    pub main_volume_left: u16,
    pub main_volume_right: u16,
    pub reverb_volume_left: u16,
    pub reverb_volume_right: u16,
    pub spucnt: SpuControlRegister,
    pub sound_modulation: u32,
    pub keyoff: u32,
    pub keyon: u32,
    pub noise_enable: u32,
    pub echo_on: u32,
    pub cd_volume: (u16, u16),
    pub current_volume: (u16, u16),
    pub external_volume: (u16, u16),
    pub sound_ram_transfer_type: u16,
    pub current_ram_address: u32,
    pub sound_ram_address: u32,
    irq_address: u32,
    pub sample: i16,
    pub voices: [Voice; 24],
    pub m_base: u16,
    pub d_apf1: u16,
    pub d_apf2: u16,
    pub v_iir: i16,
    pub v_comb1: i16,
    pub v_comb2: i16,
    pub v_comb3: i16,
    pub v_comb4: i16,
    pub v_wall: i16,
    pub v_apf1: i16,
    pub v_apf2: i16,
    pub ml_same: u32,
    pub mr_same: u32,
    pub m_l_comb1: u32,
    pub m_r_comb1: u32,
    pub m_l_comb2: u32,
    pub m_r_comb2: u32,
    pub d_l_same: u32,
    pub d_r_same: u32,
    pub m_l_diff: u32,
    pub m_r_diff: u32,
    pub m_l_comb3: u32,
    pub m_r_comb3: u32,
    pub m_l_comb4: u32,
    pub m_r_comb4: u32,
    pub d_l_diff: u32,
    pub d_r_diff: u32,
    pub m_lapf1: u32,
    pub m_rapf1: u32,
    pub m_lapf2: u32,
    pub m_rapf2: u32,
    pub v_lin: i16,
    pub v_rin: i16,
    pub v_l_out: i16,
    pub v_r_out: i16,
    sample_fifo: VecDeque<u16>,
    sound_ram: Box<[u8]>,
    producer: Caching<Arc<SharedRb<Heap<i16>>>, true, false>,
    endx: u32
}

impl SPU {
    pub fn new(producer: Caching<Arc<SharedRb<Heap<i16>>>, true, false>, scheduler: &mut Scheduler) -> Self {
        scheduler.schedule(EventType::TickSpu, SPU_CYCLES);
        Self {
            main_volume_left: 0,
            main_volume_right: 0,
            reverb_volume_left: 0,
            reverb_volume_right: 0,
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
            sample: 0,
            voices: [Voice::new(); 24],
            keyon: 0,
            m_base: 0,
            d_apf1: 0,
            d_apf2: 0,
            v_iir: 0,
            v_comb1: 0,
            v_comb2: 0,
            v_comb3: 0,
            v_comb4: 0,
            v_wall: 0,
            v_apf1: 0,
            v_apf2: 0,
            ml_same: 0,
            mr_same: 0,
            m_l_comb1: 0,
            m_r_comb1: 0,
            m_l_comb2: 0,
            m_r_comb2: 0,
            d_l_same: 0,
            d_r_same: 0,
            m_l_diff: 0,
            m_r_diff: 0,
            m_l_comb3: 0,
            m_r_comb3: 0,
            m_l_comb4: 0,
            m_r_comb4: 0,
            d_l_diff: 0,
            d_r_diff: 0,
            m_lapf1: 0,
            m_rapf1: 0,
            m_lapf2: 0,
            m_rapf2: 0,
            v_lin: 0,
            v_rin: 0,
            v_l_out: 0,
            v_r_out: 0,
            sample_fifo: VecDeque::new(),
            sound_ram: vec![0; SOUND_RAM_SIZE].into_boxed_slice(),
            producer,
            endx: 0
        }
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
        let voice = (address - 0x1f801c00) / 16;
        let channel = (address - 0x1f801c00) & 0xf;

        self.voices[voice as usize].write(channel, value);
    }

    pub fn read_voices(&self, address: usize) -> u16 {
        let voice = (address - 0x1f801c00) / 16;
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
    /*
    1f801DA2h spu   mBASE   base    Reverb Work Area Start Address in Sound RAM
    1f801DC0h rev00 dAPF1   disp    Reverb APF Offset 1
    1f801DC2h rev01 dAPF2   disp    Reverb APF Offset 2
    1f801DC4h rev02 vIIR    volume  Reverb Reflection Volume 1
    1f801DC6h rev03 vCOMB1  volume  Reverb Comb Volume 1
    1f801DC8h rev04 vCOMB2  volume  Reverb Comb Volume 2
    1f801DCAh rev05 vCOMB3  volume  Reverb Comb Volume 3
    1f801DCCh rev06 vCOMB4  volume  Reverb Comb Volume 4
    1f801DCEh rev07 vWALL   volume  Reverb Reflection Volume 2
    1f801DD0h rev08 vAPF1   volume  Reverb APF Volume 1
    1f801DD2h rev09 vAPF2   volume  Reverb APF Volume 2
    1f801DD4h rev0A mLSAME  src/dst Reverb Same Side Reflection Address 1 Left
    1f801DD6h rev0B mRSAME  src/dst Reverb Same Side Reflection Address 1 Right
    1f801DD8h rev0C mLCOMB1 src     Reverb Comb Address 1 Left
    1f801DDAh rev0D mRCOMB1 src     Reverb Comb Address 1 Right
    1f801DDCh rev0E mLCOMB2 src     Reverb Comb Address 2 Left
    1f801DDEh rev0F mRCOMB2 src     Reverb Comb Address 2 Right
    1f801DE0h rev10 dLSAME  src     Reverb Same Side Reflection Address 2 Left
    1f801DE2h rev11 dRSAME  src     Reverb Same Side Reflection Address 2 Right
    1f801DE4h rev12 mLDIFF  src/dst Reverb Different Side Reflect Address 1 Left
    1f801DE6h rev13 mRDIFF  src/dst Reverb Different Side Reflect Address 1 Right
    1f801DE8h rev14 mLCOMB3 src     Reverb Comb Address 3 Left
    1f801DEAh rev15 mRCOMB3 src     Reverb Comb Address 3 Right
    1f801DECh rev16 mLCOMB4 src     Reverb Comb Address 4 Left
    1f801DEEh rev17 mRCOMB4 src     Reverb Comb Address 4 Right
    1f801DF0h rev18 dLDIFF  src     Reverb Different Side Reflect Address 2 Left
    1f801DF2h rev19 dRDIFF  src     Reverb Different Side Reflect Address 2 Right
    1f801DF4h rev1A mLAPF1  src/dst Reverb APF Address 1 Left
    1f801DF6h rev1B mRAPF1  src/dst Reverb APF Address 1 Right
    1f801DF8h rev1C mLAPF2  src/dst Reverb APF Address 2 Left
    1f801DFAh rev1D mRAPF2  src/dst Reverb APF Address 2 Right
    1f801DFCh rev1E vLIN    volume  Reverb Input Volume Left
    1f801DFEh rev1F vRIN    volume  Reverb Input Volume Right
    */
    pub fn write16(&mut self, address: usize, value: u16, interrupt_register: &mut InterruptRegister) {
        match address {
            0x1f801c00..=0x1f801d7f => self.write_voices(address, value),
            0x1f801d80 => self.main_volume_left = value,
            0x1f801d82 => self.main_volume_right = value,
            0x1f801d84 => self.v_l_out = value as i16,
            0x1f801d86 => self.v_r_out = value as i16,
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
            0x1f801da2 => self.m_base = value,
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
                        unsafe { *(&mut self.sound_ram[self.current_ram_address as usize] as *mut u8 as *mut u16 ) = self.sample_fifo.pop_front().unwrap() };
                        self.current_ram_address = (self.current_ram_address + 2) & 0x7_ffff;
                    }
                }
            }
            0x1f801dac => self.sound_ram_transfer_type = value,
            0x1f801db0 => self.cd_volume.0 = value,
            0x1f801db2 => self.cd_volume.1 = value,
            0x1f801db4 => self.external_volume.0 = value,
            0x1f801db6 => self.external_volume.1 = value,
            0x1f801dc0 => self.d_apf1 = value,
            0x1f801dc2 => self.d_apf2 = value,
            0x1f801dc4 => self.v_iir = value as i16,
            0x1f801dc6 => self.v_comb1 = value as i16,
            0x1f801dc8 => self.v_comb2 = value as i16,
            0x1f801dca => self.v_comb3 = value as i16,
            0x1f801dcc => self.v_comb4 = value as i16,
            0x1f801dce => self.v_wall = value as i16,
            0x1f801dd0 => self.v_apf1 = value as i16,
            0x1f801dd2 => self.v_apf2 = value as i16,
            0x1f801dd4 => self.ml_same = value as u32 * 8,
            0x1f801dd6 => self.mr_same = value as u32 * 8,
            0x1f801dd8 => self.m_l_comb1 = value as u32 * 8,
            0x1f801dda => self.m_r_comb1 = value as u32 * 8,
            0x1f801ddc => self.m_l_comb2 = value as u32 * 8,
            0x1f801dde => self.m_r_comb2 = value as u32 * 8,
            0x1f801de0 => self.d_l_same = value as u32 * 8,
            0x1f801de2 => self.d_r_same = value as u32 * 8,
            0x1f801de4 => self.m_l_diff = value as u32 * 8,
            0x1f801de6 => self.m_r_diff = value as u32 * 8,
            0x1f801de8 => self.m_l_comb3 = value as u32 * 8,
            0x1f801dea => self.m_r_comb3 = value as u32 * 8,
            0x1f801dec => self.m_l_comb4 = value as u32 * 8,
            0x1f801dee => self.m_r_comb4 = value as u32 * 8,
            0x1f801df0 => self.d_l_diff = value as u32 * 8,
            0x1f801df2 => self.d_r_diff = value as u32 * 8,
            0x1f801df4 => self.m_lapf1 = value as u32 * 8,
            0x1f801df6 => self.m_rapf1 = value as u32 * 8,
            0x1f801df8 => self.m_lapf2 = value as u32 * 8,
            0x1f801dfa => self.m_rapf2 = value as u32 * 8,
            0x1f801dfc => self.v_lin = value as i16,
            0x1f801dfe => self.v_rin = value as i16,
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
        let mut left_total: i32 = 0;
        let mut right_total: i32 = 0;

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
        }

        self.producer.try_push(Self::clamp(left_total, -0x8000, 0x7fff)).unwrap_or(());
        self.producer.try_push(Self::clamp(right_total, -0x8000, 0x7fff)).unwrap_or(());

        self.update_keystatus();

        scheduler.schedule(EventType::TickSpu, SPU_CYCLES);
    }
}