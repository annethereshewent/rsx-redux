use super::{dma_channel_control_register::{DmaChannelControlRegister, SyncMode}, dma_control_register::DmaControlRegister, dma_interrupt_register::DmaInterruptRegister};


#[derive(Copy, Clone)]
pub struct DmaChannel {
    pub base_address: u32,
    pub num_words: u32,
    pub block_size: u32,
    pub num_blocks: u32,
    pub control: DmaChannelControlRegister
}

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
            dma_control: DmaControlRegister::from_bits_retain(0),
            dicr: DmaInterruptRegister::from_bits_retain(0)
        }
    }

    pub fn read_registers(&self, address: usize) -> u32 {
        let channel = (address - 0x1f801080) / 0x10;
        let register = address & 0xf;

        println!("address = 0x{:x} channel = {channel} register = {register}", address);

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

    pub fn write_registers(&mut self, address: usize, value: u32) {
        let channel = (address - 0x1f801080) / 0x10;
        let register = address & 0xf;

        println!("address = 0x{:x} channel = {channel} register = {register}", address);

        if channel < 7 {
            self.channels[channel].write(register, value);
        } else {
            match address {
                0x1f8010f0 => self.dma_control = DmaControlRegister::from_bits_retain(value),
                0x1f8010f4 => self.dicr = DmaInterruptRegister::from_bits_retain(value),
                _ => panic!("invalid dma address given: 0x{:x}", address)
            }
        }
    }
}