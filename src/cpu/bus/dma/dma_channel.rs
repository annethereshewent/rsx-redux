use serde::{Deserialize, Serialize};

use crate::cpu::bus::mdec::MdecDma;
use crate::cpu::bus::scheduler::{EventType, Scheduler};
use crate::cpu::bus::spu::SPU;
use crate::cpu::bus::{
    cdrom::CDRom, gpu::GPU, mdec::Mdec, registers::interrupt_register::InterruptRegister,
};

use super::{
    dma_channel_control_register::{DmaChannelControlRegister, SyncMode},
    dma_control_register::DmaControlRegister,
    dma_interrupt_register::DmaInterruptRegister,
};

pub const DMA_MDEC_IN: usize = 0;
pub const DMA_MDEC_OUT: usize = 1;
pub const DMA_GPU: usize = 2;
pub const DMA_CDROM: usize = 3;
pub const DMA_SPU: usize = 4;
pub const DMA_PIO: usize = 5;
pub const DMA_OTC: usize = 6;

pub const DMA_TICKS_REMAINING: usize = 100;
pub const DMA_HALT_TICKS: usize = 100;
pub const DMA_TICKS_PER_BLOCK: usize = 34;

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct DmaChannel {
    pub base_address: u32,
    pub block_size: u32,
    pub num_blocks: u32,
    pub blocks_remaining: u32,
    pub control: DmaChannelControlRegister,
    halted: bool
}

impl Default for DmaChannel {
    fn default() -> Self {
        Self::new()
    }
}

impl DmaChannel {
    pub fn new() -> Self {
        Self {
            base_address: 0,
            block_size: 0,
            num_blocks: 0,
            blocks_remaining: 0,
            control: DmaChannelControlRegister::from_bits_retain(0),
            halted: false,
        }
    }

    pub fn write(&mut self, register: usize, value: u32) {
        match register {
            0 => self.base_address = value & 0xffffff,
            4 => {
                self.block_size = value & 0xffff;
                self.num_blocks = value >> 16;
            }
            8 => self.control = DmaChannelControlRegister::from_bits_retain(value),
            _ => panic!("invalid register given: {register}"),
        }
    }

    pub fn read(&self, register: usize) -> u32 {
        match register {
            0 => self.base_address,
            4 => match self.control.sync_mode() {
                SyncMode::Manual => self.block_size,
                SyncMode::Request => self.block_size & 0xffff | (self.num_blocks & 0xffff) << 16,
                SyncMode::LinkedList => 0,
            },
            8 => self.control.bits(),
            _ => panic!("invalid register given: {register}"),
        }
    }

    pub fn get_num_words(&self) -> u32 {
        match self.control.sync_mode() {
            SyncMode::Manual => self.block_size,
            SyncMode::Request => self.block_size * self.num_blocks,
            SyncMode::LinkedList => 0, // calculate this after the end of the linked list transfer
        }
    }

    pub fn init_mdec_params(&mut self) {
        self.blocks_remaining = self.num_blocks;
    }

    fn handle_dma_request(&mut self, channel_id: usize, scheduler: &mut Scheduler, mut callback: impl FnMut(u32)) {
        self.halted = false;

        let mut current_address = self.base_address & 0x1fffff;
        let num_words = self.block_size;
        let mut ticks_remaining = DMA_TICKS_REMAINING;
        let mut words_transferred = 0;

        while ticks_remaining > 0 && self.num_blocks > 0 {
            for _ in 0..num_words {
                callback(current_address);

                if self.control.contains(DmaChannelControlRegister::DECREMENT) {
                    current_address -= 4;
                } else {
                    current_address += 4;
                }

                words_transferred += 1;
            }

            self.num_blocks -= 1;
            ticks_remaining = ticks_remaining.saturating_sub(DMA_TICKS_PER_BLOCK);
        }

        self.base_address = current_address;

        if self.num_blocks > 0 {
            self.halted = true;
            scheduler.schedule(EventType::UnhaltDma(channel_id), DMA_HALT_TICKS);
        } else {
            scheduler.schedule(EventType::DmaFinished(channel_id), words_transferred);
        }
    }

    pub fn start_mdec_in_transfer(&mut self, ram: &mut [u8], mdec: &mut Mdec, scheduler: &mut Scheduler) -> MdecDma {
        assert_eq!(self.control.sync_mode(), SyncMode::Request);
        assert!(
            self.control
                .contains(DmaChannelControlRegister::TRANSFER_DIR)
        );

        self.handle_dma_request(DMA_MDEC_IN, scheduler,  |current_address| {
            let word = unsafe { *(&ram[current_address as usize] as *const u8 as *const u32) };

            mdec.dma_write(word);
        });

        mdec.execute()
    }

    pub fn start_mdec_out_transfer(&mut self, ram: &mut [u8], mdec: &mut Mdec, scheduler: &mut Scheduler) -> MdecDma {
        assert!(self.control.sync_mode() == SyncMode::Request);
        assert!(
            !self
                .control
                .contains(DmaChannelControlRegister::TRANSFER_DIR)
        );

        self.handle_dma_request(DMA_MDEC_OUT, scheduler, |current_address| {
            let word = mdec.read_out_fifo();

            unsafe { *(&mut ram[current_address as usize] as *mut u8 as *mut u32) = word };
        });

        mdec.update_status()
    }

    pub fn start_gpu_transfer(&mut self, ram: &mut [u8], gpu: &mut GPU) -> u32 {
        if !self
            .control
            .contains(DmaChannelControlRegister::TRANSFER_DIR)
        {
            if self.control.sync_mode() == SyncMode::Request {
                let mut current_address = self.base_address & 0x1ffffc;

                for _ in 0..self.num_blocks {
                    for _ in 0..self.block_size {
                        let word = gpu.read_gpu();

                        unsafe {
                            *(&mut ram[current_address as usize] as *mut u8 as *mut u32) = word
                        };

                        if self.control.contains(DmaChannelControlRegister::DECREMENT) {
                            current_address -= 4;
                        } else {
                            current_address += 4;
                        }
                    }
                }
            } else {
                panic!(
                    "unsupported gpu transfer to cpu given: {:?}",
                    self.control.sync_mode()
                );
            }
        } else {
            // from ram
            match self.control.sync_mode() {
                SyncMode::LinkedList => {
                    let mut current_address = self.base_address & 0x1ffffc;

                    let mut total_word_count = 0;

                    loop {
                        let packet =
                            unsafe { *(&ram[current_address as usize] as *const u8 as *const u32) };
                        let mut word_count = packet >> 24;
                        total_word_count += word_count;

                        while word_count > 0 {
                            current_address += 4;

                            let word = unsafe {
                                *(&ram[current_address as usize] as *const u8 as *const u32)
                            };

                            word_count -= 1;

                            gpu.process_gp0_commands(word);
                        }

                        current_address = packet & 0xffffff;

                        if current_address == 0xffffff {
                            break;
                        }

                        current_address &= !(0x3);
                    }

                    return total_word_count;
                }
                SyncMode::Manual => {
                    let mut current_address = self.base_address & 0x1fffff;

                    let num_words = self.block_size;

                    for _ in 0..num_words {
                        let word =
                            unsafe { *(&ram[current_address as usize] as *const u8 as *const u32) };
                        gpu.process_gp0_commands(word);

                        if self.control.contains(DmaChannelControlRegister::DECREMENT) {
                            current_address -= 4;
                        } else {
                            current_address += 4;
                        }
                    }
                }
                SyncMode::Request => {
                    let block_size = if self.block_size == 0 {
                        0x10000
                    } else {
                        self.block_size
                    };

                    let mut current_address = self.base_address;

                    for _ in 0..self.num_blocks {
                        for _ in 0..block_size {
                            let word = unsafe {
                                *(&ram[(current_address & 0x1ffffc) as usize] as *const u8
                                    as *const u32)
                            };

                            gpu.process_gp0_commands(word);

                            if self.control.contains(DmaChannelControlRegister::DECREMENT) {
                                current_address -= 4;
                            } else {
                                current_address += 4;
                            }
                        }
                    }
                }
            }
        }

        0
    }

    pub fn start_cdrom_transfer(&mut self, ram: &mut [u8], cdrom: &mut CDRom) {
        assert!(self.control.sync_mode() == SyncMode::Manual);

        let mut current_address = self.base_address;

        if self
            .control
            .contains(DmaChannelControlRegister::TRANSFER_DIR)
        {
            panic!("only transfers from cdrom to RAM are supported");
        }

        for _ in 0..self.block_size {
            let value = cdrom.read_data_buffer();

            unsafe { *(&mut ram[current_address as usize] as *mut u8 as *mut u32) = value };

            if self.control.contains(DmaChannelControlRegister::DECREMENT) {
                current_address -= 4;
            } else {
                current_address += 4;
            }
        }
    }

    pub fn start_spu_transfer(
        &mut self,
        ram: &mut [u8],
        spu: &mut SPU,
        interrupt_register: &mut InterruptRegister,
    ) {
        let mut current_address = self.base_address;

        assert_eq!(self.control.sync_mode(), SyncMode::Request);

        if !self
            .control
            .contains(DmaChannelControlRegister::TRANSFER_DIR)
        {
            panic!("only transfers from ram to spu allowed");
        }

        let num_words = self.get_num_words();

        for _ in 0..num_words {
            let word = unsafe { *(&ram[current_address as usize] as *const u8 as *const u32) };

            spu.dma_write(word, interrupt_register);

            current_address += 4;
        }

        spu.update_dma_request();
    }

    pub fn start_pio_transfer(&mut self) {
        todo!("pio transfer");
    }

    pub fn start_otc_transfer(&mut self, ram: &mut [u8]) {
        assert!(self.control.sync_mode() == SyncMode::Manual);

        let mut current_address = self.base_address & 0x1ffffc;

        for i in 0..self.block_size {
            let value = if i == self.block_size - 1 {
                0xffffff
            } else {
                current_address - 4
            };

            unsafe { *(&mut ram[current_address as usize] as *mut u8 as *mut u32) = value };

            if self.control.contains(DmaChannelControlRegister::DECREMENT) {
                current_address -= 4;
            } else {
                current_address += 4;
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Dma {
    pub channels: [DmaChannel; 7],
    pub dma_control: DmaControlRegister,
    pub dicr: DmaInterruptRegister,
}

impl Default for Dma {
    fn default() -> Self {
        Self::new()
    }
}

impl Dma {
    pub fn new() -> Self {
        Self {
            channels: [DmaChannel::new(); 7],
            dma_control: DmaControlRegister::from_bits_retain(0x7654321),
            dicr: DmaInterruptRegister::from_bits_retain(0),
        }
    }

    pub fn read_registers(&self, address: usize) -> u32 {
        let channel = (address - 0x1f801080) / 0x10;
        let register = address & 0xf;

        if channel < 7 {
            self.channels[channel].read(register)
        } else {
            match address {
                0x1f8010f0 => self.dma_control.bits(),
                0x1f8010f4 => self.dicr.bits(),
                _ => panic!("invalid dma address given: 0x{address:x}"),
            }
        }
    }

    pub fn process_mdec_dma(&mut self, mdec_dma: MdecDma, ram: &mut [u8], mdec: &mut Mdec, scheduler: &mut Scheduler) {
        if mdec_dma.dma_in && self.can_transfer_dma(DMA_MDEC_IN) {
            self.channels[DMA_MDEC_IN].start_mdec_in_transfer(ram, mdec, scheduler);
        }

        if mdec_dma.dma_out && self.can_transfer_dma(DMA_MDEC_OUT) {
            self.channels[DMA_MDEC_OUT].start_mdec_out_transfer(ram, mdec, scheduler);
        }
    }

    fn can_transfer_dma(&self, channel: usize) -> bool {
        let dma_channel = &self.channels[channel];

        let shift = channel * 4 + 3;

        dma_channel
            .control
            .contains(DmaChannelControlRegister::START_TRANSFER)
            && !dma_channel.halted
            && (self.dma_control.bits() >> shift) & 0x1 == 1
    }

    pub fn finish_transfer(&mut self, channel: usize, interrupt_stat: &mut InterruptRegister) {
        let dma_channel = &mut self.channels[channel];

        dma_channel
            .control
            .remove(DmaChannelControlRegister::START_TRANSFER);

        dma_channel
            .control
            .remove(DmaChannelControlRegister::FORCE_TRANSFER);

        self.dicr.set_channel_irq_if_enabled(channel);

        if self.dicr.master_interrupt_flag() {
            interrupt_stat.insert(InterruptRegister::DMA);
        }
    }

    pub fn write_registers(&mut self, address: usize, value: u32) -> bool {
        let channel = (address - 0x1f801080) / 0x10;
        let register = address & 0xf;

        let previous_enable = if channel < 7 {
            let previous_enable = self.channels[channel]
                .control
                .contains(DmaChannelControlRegister::START_TRANSFER);
            self.channels[channel].write(register, value);
            previous_enable
        } else {
            match address {
                0x1f8010f0 => self.dma_control = DmaControlRegister::from_bits_retain(value),
                0x1f8010f4 => {
                    let mut bits = self.dicr.bits();

                    bits &= 0xff00_0000;
                    bits &= !(value & 0x7f00_0000);
                    bits |= value & 0xff_803f;

                    self.dicr = DmaInterruptRegister::from_bits_retain(bits)
                }
                _ => panic!("invalid dma address given: 0x{address:x}"),
            }

            return false;
        };

        let shift = channel * 4 + 3;

        let dma_channel = &self.channels[channel];

        dma_channel
            .control
            .contains(DmaChannelControlRegister::START_TRANSFER)
            && !previous_enable
            && (self.dma_control.bits() >> shift) & 0x1 == 1
    }
}
