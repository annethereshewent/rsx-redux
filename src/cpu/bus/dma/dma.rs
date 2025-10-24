use crate::cpu::bus::{
    cdrom::CDRom,
    gpu::GPU,
    mdec::Mdec,
    registers::interrupt_register::InterruptRegister,
    scheduler::{EventType, Scheduler},
    spu_legacy::SPU,
};

use super::{
    dma_channel_control_register::{DmaChannelControlRegister, SyncMode},
    dma_control_register::DmaControlRegister,
    dma_interrupt_register::DmaInterruptRegister,
};

#[derive(Copy, Clone)]
pub struct DmaChannel {
    pub base_address: u32,
    pub block_size: u32,
    pub num_blocks: u32,
    pub control: DmaChannelControlRegister,
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
            block_size: 0,
            num_blocks: 0,
            control: DmaChannelControlRegister::from_bits_retain(0),
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
                SyncMode::Burst => self.block_size,
                SyncMode::Slice => self.block_size & 0xffff | (self.num_blocks & 0xffff) << 16,
                SyncMode::LinkedList => 0,
            },
            8 => self.control.bits(),
            _ => panic!("invalid register given: {register}"),
        }
    }
}

pub struct Dma {
    channels: [DmaChannel; 7],
    pub dma_control: DmaControlRegister,
    pub dicr: DmaInterruptRegister,
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
                _ => panic!("invalid dma address given: 0x{:x}", address),
            }
        }
    }

    fn start_mdec_in_transfer(&mut self, ram: &mut [u8], mdec: &mut Mdec) {
        let dma_channel = &mut self.channels[MDEC_IN];

        assert_eq!(dma_channel.control.sync_mode(), SyncMode::Slice);

        let mut current_address = dma_channel.base_address & 0x1fffff;
        let block_size = dma_channel.block_size;
        let num_blocks = dma_channel.num_blocks;

        let num_words = block_size * num_blocks;

        for _ in 0..num_words {
            let word = unsafe { *(&ram[current_address as usize] as *const u8 as *const u32) };

            mdec.write_command(word);

            if dma_channel
                .control
                .contains(DmaChannelControlRegister::DECREMENT)
            {
                current_address -= 4;
            } else {
                current_address += 4;
            }
        }
    }

    fn start_mdec_out_transfer(&mut self) {
        todo!("mdec out transfer");
    }

    fn start_gpu_transfer(&mut self, ram: &mut [u8], gpu: &mut GPU) -> u32 {
        let dma_channel = &mut self.channels[GPU];

        if !dma_channel
            .control
            .contains(DmaChannelControlRegister::TRANSFER_DIR)
        {
            // to ram
            todo!("transfer to ram");
        } else {
            // from ram
            match dma_channel.control.sync_mode() {
                SyncMode::LinkedList => {
                    let mut current_address = dma_channel.base_address & 0x1ffffc;

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
                SyncMode::Burst => {
                    let mut current_address = dma_channel.base_address & 0x1fffff;

                    let num_words = dma_channel.block_size;

                    for _ in 0..num_words {
                        let word =
                            unsafe { *(&ram[current_address as usize] as *const u8 as *const u32) };
                        gpu.process_gp0_commands(word);

                        if dma_channel
                            .control
                            .contains(DmaChannelControlRegister::DECREMENT)
                        {
                            current_address -= 4;
                        } else {
                            current_address += 4;
                        }
                    }
                }
                SyncMode::Slice => {
                    let block_size = if dma_channel.block_size == 0 {
                        0x10000
                    } else {
                        dma_channel.block_size
                    };

                    let mut current_address = dma_channel.base_address;

                    for _ in 0..dma_channel.num_blocks {
                        for _ in 0..block_size {
                            let word = unsafe {
                                *(&ram[(current_address & 0x1ffffc) as usize] as *const u8
                                    as *const u32)
                            };

                            gpu.process_gp0_commands(word);

                            if dma_channel
                                .control
                                .contains(DmaChannelControlRegister::DECREMENT)
                            {
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

    fn start_cdrom_transfer(&mut self, ram: &mut [u8], cdrom: &mut CDRom) {
        let channel = &mut self.channels[CDROM];

        assert!(channel.control.sync_mode() == SyncMode::Burst);

        let mut current_address = channel.base_address;

        if channel
            .control
            .contains(DmaChannelControlRegister::TRANSFER_DIR)
        {
            panic!("only transfers from cdrom to RAM are supported");
        }

        for _ in 0..channel.block_size {
            let value = cdrom.read_data_buffer();

            unsafe { *(&mut ram[current_address as usize] as *mut u8 as *mut u32) = value };

            if channel
                .control
                .contains(DmaChannelControlRegister::DECREMENT)
            {
                current_address -= 4;
            } else {
                current_address += 4;
            }
        }
    }

    fn start_spu_transfer(&mut self, ram: &mut [u8], spu: &mut SPU) {
        let channel = &mut self.channels[SPU];

        let mut current_address = channel.base_address;

        assert_eq!(channel.control.sync_mode(), SyncMode::Slice);

        if !channel
            .control
            .contains(DmaChannelControlRegister::TRANSFER_DIR)
        {
            panic!("only transfers from ram to spu allowed");
        }

        let num_words = channel.num_blocks * channel.block_size;

        for _ in 0..num_words {
            let word = unsafe { *(&ram[current_address as usize] as *const u8 as *const u32) };

            spu.dma_write(word);

            current_address += 4;
        }
    }

    fn start_pio_transfer(&mut self) {
        todo!("pio transfer");
    }

    fn start_otc_transfer(&mut self, ram: &mut [u8], interrupt_stat: &mut InterruptRegister) {
        let channel = &mut self.channels[OTC];

        assert!(channel.control.sync_mode() == SyncMode::Burst);

        let mut current_address = channel.base_address & 0x1ffffc;

        for i in 0..channel.block_size {
            let value = if i == channel.block_size - 1 {
                0xffffff
            } else {
                current_address - 4
            };

            unsafe { *(&mut ram[current_address as usize] as *mut u8 as *mut u32) = value };

            if channel
                .control
                .contains(DmaChannelControlRegister::DECREMENT)
            {
                current_address -= 4;
            } else {
                current_address += 4;
            }
        }

        self.finish_transfer(OTC, interrupt_stat);
    }

    pub fn finish_transfer(&mut self, channel: usize, interrupt_stat: &mut InterruptRegister) {
        let dma_channel = &mut self.channels[channel];

        dma_channel
            .control
            .remove(DmaChannelControlRegister::START_TRANSFER);

        let shift = 24 + channel;

        let mut interrupt_bits = self.dicr.bits();

        interrupt_bits |= 1 << shift;

        self.dicr = DmaInterruptRegister::from_bits_retain(interrupt_bits);

        if self.dicr.master_interrupt_flag() {
            interrupt_stat.insert(InterruptRegister::DMA);
        }
    }

    pub fn write_registers(
        &mut self,
        address: usize,
        value: u32,
        scheduler: &mut Scheduler,
        ram: &mut [u8],
        gpu: &mut GPU,
        spu: &mut SPU,
        cdrom: &mut CDRom,
        mdec: &mut Mdec,
        interrupt_stat: &mut InterruptRegister,
    ) {
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
                _ => panic!("invalid dma address given: 0x{:x}", address),
            }

            return;
        };

        let shift = channel * 4 + 3;

        let dma_channel = &mut self.channels[channel];

        if dma_channel
            .control
            .contains(DmaChannelControlRegister::START_TRANSFER)
            && !previous_enable
            && (self.dma_control.bits() >> shift) & 0x1 == 1
        {
            let clocks = match channel {
                0 | 1 | 2 | 6 => 1,
                3 => 24,
                4 => 4,
                5 => 20,
                _ => panic!("Unknown DMA channel"),
            };

            let mut num_words = match dma_channel.control.sync_mode() {
                SyncMode::Burst => dma_channel.block_size,
                SyncMode::Slice => dma_channel.block_size * dma_channel.num_blocks,
                SyncMode::LinkedList => 0, // calculate this after the end of the linked list transfer
            };

            match channel {
                0 => self.start_mdec_in_transfer(ram, mdec),
                1 => self.start_mdec_out_transfer(),
                2 => match dma_channel.control.sync_mode() {
                    SyncMode::LinkedList => num_words = self.start_gpu_transfer(ram, gpu),
                    SyncMode::Burst | SyncMode::Slice => {
                        self.start_gpu_transfer(ram, gpu);
                    }
                },
                3 => self.start_cdrom_transfer(ram, cdrom),
                4 => self.start_spu_transfer(ram, spu),
                5 => self.start_pio_transfer(),
                6 => self.start_otc_transfer(ram, interrupt_stat),
                _ => todo!("dma transfer for channel {channel}"),
            }

            if channel != 6 {
                scheduler.schedule(
                    EventType::DmaFinished(channel),
                    (num_words * clocks) as usize,
                );
            }
        }
    }
}
