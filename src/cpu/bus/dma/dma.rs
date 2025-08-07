use crate::cpu::bus::{gpu::GPU, registers::interrupt_register::InterruptRegister, scheduler::{EventType, Scheduler}};

use super::{dma_channel_control_register::{DmaChannelControlRegister, SyncMode}, dma_control_register::DmaControlRegister, dma_interrupt_register::DmaInterruptRegister};


#[derive(Copy, Clone)]
pub struct DmaChannel {
    pub base_address: u32,
    pub num_words: u32,
    pub block_size: u32,
    pub num_blocks: u32,
    pub control: DmaChannelControlRegister
}

const MDEC_IN: usize = 0;
const MDEC_OUT: usize = 1;
const GPU: usize = 2;
const CDROM: usize = 3;
const SPU: usize = 4;
const PIO: usize = 5;
const OTC: usize = 6;

impl DmaChannel {
    pub fn new() -> Self {
        Self {
            base_address: 0,
            num_words: 0,
            block_size: 0,
            num_blocks: 0,
            control: DmaChannelControlRegister::from_bits_retain(0)
        }
    }

    pub fn write(&mut self, register: usize, value: u32) {
        match register {
            0 => self.base_address = value & 0xffffff,
            4 => match self.control.sync_mode() {
                SyncMode::Burst => self.num_words = value & 0xffff,
                SyncMode::Slice => {
                    self.block_size = value & 0xffff;
                    self.num_blocks = value >> 16;
                }
                SyncMode::LinkedList => ()
            }
            8 => self.control = DmaChannelControlRegister::from_bits_retain(value),
            _ => panic!("invalid register given: {register}")
        }
    }

    pub fn read(&self, register: usize) -> u32 {
        match register {
            0 => self.base_address,
            4 => match self.control.sync_mode() {
                SyncMode::Burst => self.num_words,
                SyncMode::Slice => self.block_size & 0xffff | (self.num_blocks & 0xffff) << 16,
                SyncMode::LinkedList => 0
            }
            8 => self.control.bits(),
            _ => panic!("invalid register given: {register}")
        }
    }
}

pub struct Dma {
    channels: [DmaChannel; 7],
    pub dma_control: DmaControlRegister,
    pub dicr: DmaInterruptRegister
}

impl Dma {
    pub fn new() -> Self {
        Self {
            channels: [DmaChannel::new(); 7],
            dma_control: DmaControlRegister::from_bits_retain(0x7654321),
            dicr: DmaInterruptRegister::from_bits_retain(0)
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
                _ => panic!("invalid dma address given: 0x{:x}", address)
            }
        }
    }

    fn start_mdec_in_transfer(&mut self) {
        let _dma_channel = &mut self.channels[MDEC_IN];

        todo!("mdec in transfer");
    }

    fn start_mdec_out_transfer(&mut self) {
        todo!("mdec out transfer");
    }

    fn start_gpu_transfer(&mut self, ram: &mut [u8], gpu: &mut GPU) -> u32 {
        let dma_channel = &mut self.channels[GPU];

        if !dma_channel.control.contains(DmaChannelControlRegister::TRANSFER_DIR) {
            // to ram
            todo!("transfer to ram");
        } else {
            // from ram
            if dma_channel.control.sync_mode() != SyncMode::LinkedList {
                let mut current_address = dma_channel.base_address & 0x1fffff;

                for _ in 0..dma_channel.num_words {
                    let word = unsafe { *(&ram[current_address as usize] as *const u8 as *const u32) };
                    gpu.command_fifo.push_back(word);

                    if dma_channel.control.contains(DmaChannelControlRegister::DECREMENT) {
                        current_address -= 4;
                    } else {
                        current_address += 4;
                    }
                }
            } else {
                let mut current_address = dma_channel.base_address & 0x1fffff;
                let mut packet = unsafe { *(&ram[current_address as usize] as *const u8 as *const u32) };

                let mut next_packet_address = current_address;
                current_address += 4;

                let mut total_word_count = 0;

                loop {
                    let mut word_count = packet >> 24;

                    total_word_count += word_count;

                    packet = unsafe { *(&ram[next_packet_address as usize] as *const u8 as *const u32 ) };

                    while word_count > 0 {
                        let word = unsafe { *(&ram[current_address as usize] as *const u8 as *const u32 ) };
                        word_count -= 1;
                        current_address += 4;

                        gpu.command_fifo.push_back(word);
                    }

                    if dma_channel.control.contains(DmaChannelControlRegister::DECREMENT) {
                        current_address -= 4;
                    } else {
                        current_address += 4;
                    }

                    next_packet_address = packet & 0xffffff;

                    if next_packet_address == 0xffffff {
                        break;
                    }

                    current_address = next_packet_address + 4;
                }

                return total_word_count;
            }
        }

        0
    }

    fn start_cdrom_transfer(&mut self) {
        todo!("cdrom transfer");
    }

    fn start_spu_transfer(&mut self) {
        todo!("spu transfer");
    }

    fn start_pio_transfer(&mut self) {
        todo!("pio transfer");
    }

    fn start_otc_transfer(&mut self, ram: &mut [u8]) {
        let channel = &mut self.channels[OTC];

        let mut current_address = channel.base_address & 0x1ffffc;

        for i in 0..channel.num_words {
            let value = if i == channel.num_words -1 {
                0xffffff
            } else {
                current_address - 4
            };

            unsafe { *(&mut ram[current_address as usize] as *mut u8 as *mut u32) = value };

            if channel.control.contains(DmaChannelControlRegister::DECREMENT) {
                current_address -= 4;
            } else {
                current_address += 4;
            }
        }
    }

    pub fn finish_transfer(&mut self, channel: usize, interrupt_stat: &mut InterruptRegister) {
        let dma_channel = &mut self.channels[channel];

        dma_channel.control.remove(DmaChannelControlRegister::START_TRANSFER);

        let shift = 24 + channel;

        let mut interrupt_bits = self.dicr.bits();

        interrupt_bits |= 1 << shift;

        self.dicr = DmaInterruptRegister::from_bits_retain(interrupt_bits);

        if self.dicr.master_interrupt_flag() {
            interrupt_stat.insert(InterruptRegister::DMA);
        }
    }

    pub fn write_registers(&mut self, address: usize, value: u32, scheduler: &mut Scheduler, ram: &mut [u8], gpu: &mut GPU) {
        let channel = (address - 0x1f801080) / 0x10;
        let register = address & 0xf;

        let previous_enable = if channel < 7 {
            let previous_enable = self.channels[channel].control.contains(DmaChannelControlRegister::START_TRANSFER);
            self.channels[channel].write(register, value);
            previous_enable
        } else {
            match address {
                0x1f8010f0 => self.dma_control = DmaControlRegister::from_bits_retain(value),
                0x1f8010f4 => self.dicr = DmaInterruptRegister::from_bits_retain(value),
                _ => panic!("invalid dma address given: 0x{:x}", address)
            }

            return;
        };

        let shift = channel * 4 + 3;

        let dma_channel = &mut self.channels[channel];

        if dma_channel.control.contains(DmaChannelControlRegister::START_TRANSFER) && !previous_enable && (self.dma_control.bits() >> shift) & 0x1 == 1 {
            let clocks = match channel {
                0 | 1 | 2 | 6 => 1,
                3 => 24, // CDROM BIOS access, or...
                4 => 4,
                5 => 20,
                _ => panic!("Unknown DMA channel")
            };

            let mut num_words = match dma_channel.control.sync_mode() {
                SyncMode::Burst => dma_channel.num_words,
                SyncMode::Slice => dma_channel.block_size * dma_channel.num_blocks,
                SyncMode::LinkedList => 0 // calculate this after the end of the linked list transfer
            };

            match channel {
                0 => self.start_mdec_in_transfer(),
                1 => self.start_mdec_out_transfer(),
                2 =>  {
                    if dma_channel.control.sync_mode() == SyncMode::LinkedList {
                        num_words = self.start_gpu_transfer(ram, gpu);
                    } else {
                        self.start_gpu_transfer(ram, gpu);
                    }
                }
                3 => self.start_cdrom_transfer(),
                4 => self.start_spu_transfer(),
                5 => self.start_pio_transfer(),
                6 => self.start_otc_transfer(ram),
                _ => todo!("dma transfer for channel {channel}")
            }

            scheduler.schedule(EventType::DmaFinished(channel), (num_words * clocks) as usize);
        }
    }
}