use serde::{Deserialize, Serialize};

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

const DMA_LINKED_LIST_MAX_TICKS: usize = 1000;
const DMA_LINKED_LIST_HEADER_READ_TICKS: usize = 8;
const DMA_LINKED_LIST_BLOCK_SETUP_TICKS: usize = 5;
const DMA_HALT_LINKED_LIST_TICKS: usize = 5;

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct DmaChannel {
    pub base_address: u32,
    pub block_size: u32,
    pub num_blocks: u32,
    pub control: DmaChannelControlRegister,
    halted: bool,
    request: bool,
    pub current_address: u32, // used for linked list transfers exclusively right now
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
            control: DmaChannelControlRegister::from_bits_retain(0),
            halted: false,
            request: false,
            current_address: 0,
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

    pub fn start_gpu_transfer(&mut self, ram: &mut [u8], gpu: &mut GPU) {
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
                _ => unreachable!(),
            }
        }
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

        let num_words = self.get_num_words();

        if self.control.contains(DmaChannelControlRegister::TRANSFER_DIR) {
            for _ in 0..num_words {
                let word = unsafe { *(&ram[current_address as usize] as *const u8 as *const u32) };

                spu.dma_write(word, interrupt_register);

                if self.control.contains(DmaChannelControlRegister::DECREMENT) {
                    current_address -= 4;
                } else {
                    current_address += 4;
                }
            }
        } else {
            for _ in 0..num_words {
                let word = spu.dma_read();

                unsafe { *(&mut ram[current_address as usize] as *mut u8 as *mut u32) = word };

                if self.control.contains(DmaChannelControlRegister::DECREMENT) {
                    current_address -= 4;
                } else {
                    current_address += 4;
                }
            }
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

    pub fn start_mdec_in_transfer(
        &mut self,
        ram: &mut [u8],
        mdec: &mut Mdec,
        scheduler: &mut Scheduler,
        interrupt_register: &mut InterruptRegister,
    ) {
        assert_eq!(
            self.channels[DMA_MDEC_IN].control.sync_mode(),
            SyncMode::Request,
            "expected Request sync mode, got {:?}",
            self.channels[DMA_MDEC_IN].control.sync_mode()
        );
        assert!(
            self.channels[DMA_MDEC_IN]
                .control
                .contains(DmaChannelControlRegister::TRANSFER_DIR)
        );

        self.channels[DMA_MDEC_IN].halted = false;

        let mut current_address = self.channels[DMA_MDEC_IN].base_address & 0x1fffff;
        let num_words = self.channels[DMA_MDEC_IN].block_size;
        let mut ticks_remaining = DMA_TICKS_REMAINING;

        while ticks_remaining > 0
            && self.channels[DMA_MDEC_IN].num_blocks > 0
            && self.channels[DMA_MDEC_IN].request
        {
            for _ in 0..num_words {
                let word = unsafe { *(&ram[current_address as usize] as *const u8 as *const u32) };

                mdec.dma_write(word);

                if self.channels[DMA_MDEC_IN]
                    .control
                    .contains(DmaChannelControlRegister::DECREMENT)
                {
                    current_address -= 4;
                } else {
                    current_address += 4;
                }
            }

            let mdec_dma = mdec.execute();

            self.set_request(
                mdec_dma.dma_in,
                DMA_MDEC_IN,
                ram,
                mdec,
                scheduler,
                interrupt_register,
            );
            self.set_request(
                mdec_dma.dma_out,
                DMA_MDEC_OUT,
                ram,
                mdec,
                scheduler,
                interrupt_register,
            );

            self.channels[DMA_MDEC_IN].num_blocks -= 1;
            ticks_remaining = ticks_remaining.saturating_sub(DMA_TICKS_PER_BLOCK);
        }

        self.channels[DMA_MDEC_IN].base_address = current_address;

        if self.channels[DMA_MDEC_IN].num_blocks > 0 {
            self.channels[DMA_MDEC_IN].halted = true;
            scheduler.schedule(EventType::UnhaltDma(DMA_MDEC_IN), DMA_HALT_TICKS);
        } else {
            self.finish_transfer(DMA_MDEC_IN, interrupt_register);
        }
    }

    pub fn start_mdec_out_transfer(
        &mut self,
        ram: &mut [u8],
        mdec: &mut Mdec,
        scheduler: &mut Scheduler,
        interrupt_register: &mut InterruptRegister,
    ) {
        assert!(
            self.channels[DMA_MDEC_OUT].control.sync_mode() == SyncMode::Request,
            "expected Request sync mode, got {:?}",
            self.channels[DMA_MDEC_OUT].control.sync_mode()
        );
        assert!(
            !self.channels[DMA_MDEC_OUT]
                .control
                .contains(DmaChannelControlRegister::TRANSFER_DIR)
        );

        self.channels[DMA_MDEC_OUT].halted = false;

        let mut current_address = self.channels[DMA_MDEC_OUT].base_address & 0x1fffff;
        let num_words = self.channels[DMA_MDEC_OUT].block_size;
        let mut ticks_remaining = DMA_TICKS_REMAINING;

        while ticks_remaining > 0
            && self.channels[DMA_MDEC_OUT].num_blocks > 0
            && self.channels[DMA_MDEC_OUT].request
        {
            for _ in 0..num_words {
                let word = mdec.read_out_fifo();

                unsafe { *(&mut ram[current_address as usize] as *mut u8 as *mut u32) = word };

                if self.channels[DMA_MDEC_OUT]
                    .control
                    .contains(DmaChannelControlRegister::DECREMENT)
                {
                    current_address -= 4;
                } else {
                    current_address += 4;
                }
            }

            if mdec.out_fifo_empty() {
                let mdec_dma = mdec.execute();

                self.set_request(
                    mdec_dma.dma_in,
                    DMA_MDEC_IN,
                    ram,
                    mdec,
                    scheduler,
                    interrupt_register,
                );
                self.set_request(
                    mdec_dma.dma_out,
                    DMA_MDEC_OUT,
                    ram,
                    mdec,
                    scheduler,
                    interrupt_register,
                );
            }

            self.channels[DMA_MDEC_OUT].num_blocks -= 1;
            ticks_remaining = ticks_remaining.saturating_sub(DMA_TICKS_PER_BLOCK);
        }

        self.channels[DMA_MDEC_OUT].base_address = current_address;

        if self.channels[DMA_MDEC_OUT].num_blocks > 0 {
            self.channels[DMA_MDEC_OUT].halted = true;
            scheduler.schedule(EventType::UnhaltDma(DMA_MDEC_OUT), DMA_HALT_TICKS);
        } else {
            self.finish_transfer(DMA_MDEC_OUT, interrupt_register);
        }
    }

    pub fn process_linked_list(
        &mut self,
        ram: &mut [u8],
        gpu: &mut GPU,
        interrupt_register: &mut InterruptRegister,
        scheduler: &mut Scheduler,
    ) {
        self.channels[DMA_GPU].halted = false;
        let mut remaining_ticks = DMA_LINKED_LIST_MAX_TICKS;
        while remaining_ticks > 0 {
            let packet = unsafe {
                *(&ram[self.channels[DMA_GPU].current_address as usize] as *const u8 as *const u32)
            };
            let mut word_count = packet >> 24;

            let word_ticks = word_count as usize;

            while word_count > 0 {
                self.channels[DMA_GPU].current_address += 4;

                let word = unsafe {
                    *(&ram[self.channels[DMA_GPU].current_address as usize] as *const u8
                        as *const u32)
                };

                word_count -= 1;

                gpu.process_gp0_commands(word);
            }

            self.channels[DMA_GPU].current_address = packet & 0xffffff;

            let tick_count = if word_ticks > 0 {
                word_ticks + DMA_LINKED_LIST_BLOCK_SETUP_TICKS + DMA_LINKED_LIST_HEADER_READ_TICKS
            } else {
                DMA_LINKED_LIST_HEADER_READ_TICKS
            };

            remaining_ticks = remaining_ticks.saturating_sub(tick_count);

            if self.channels[DMA_GPU].current_address == 0xffffff {
                // terminate the transfer and return
                self.finish_transfer(DMA_GPU, interrupt_register);
                return;
            }

            self.channels[DMA_GPU].current_address &= !(0x3);
        }

        self.channels[DMA_GPU].halted = true;
        scheduler.schedule(EventType::UnhaltDma(DMA_GPU), DMA_HALT_LINKED_LIST_TICKS);
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

    // initiates dma request mode, only supports mdec channels right now
    // since ff9 demands it
    pub fn set_request(
        &mut self,
        request: bool,
        channel: usize,
        ram: &mut [u8],
        mdec: &mut Mdec,
        scheduler: &mut Scheduler,
        interrupt_register: &mut InterruptRegister,
    ) {
        if request == self.channels[channel].request {
            return;
        }

        self.channels[channel].request = request;

        if self.channels[channel].request && self.can_transfer_dma(channel) {
            match channel {
                DMA_MDEC_IN => {
                    self.start_mdec_in_transfer(ram, mdec, scheduler, interrupt_register)
                }
                DMA_MDEC_OUT => {
                    self.start_mdec_out_transfer(ram, mdec, scheduler, interrupt_register)
                }
                _ => panic!("unsupported dma channel for set_request: {channel}"),
            };
        }
    }

    // only used for mdec in and mdec out request transfers
    fn can_transfer_dma(&self, channel: usize) -> bool {
        let dma_channel = &self.channels[channel];

        let shift = channel * 4 + 3;

        dma_channel
            .control
            .contains(DmaChannelControlRegister::START_TRANSFER)
            && !dma_channel.halted
            && dma_channel.request
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

        // Currently only mdec in and mdec out work with manual triggering of request transfers,
        // as ff9 relies on this behavior for fmvs to work.
        let request = if [DMA_MDEC_IN, DMA_MDEC_OUT].contains(&channel) {
            dma_channel.request && !dma_channel.halted
        } else if channel == DMA_GPU {
            !dma_channel.halted
        } else {
            true
        };

        dma_channel
            .control
            .contains(DmaChannelControlRegister::START_TRANSFER)
            && !previous_enable
            && request
            && (self.dma_control.bits() >> shift) & 0x1 == 1
    }
}
