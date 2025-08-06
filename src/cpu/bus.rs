use registers::delay_register::DelayRegister;

pub mod registers;

pub struct Bus {
    bios: Vec<u8>,
    bios_delay: DelayRegister,
    ram_size: u32
}

impl Bus {
    pub fn new() -> Self {
        Self {
            bios: Vec::new(),
            bios_delay: DelayRegister::new(),
            ram_size: 0
        }
    }

    pub fn load_bios(&mut self, bios: Vec<u8>) {
        self.bios = bios;
    }

    pub fn translate_address(address: u32) -> usize {
        match address >> 28 {
            0x8 | 0x9 => (address & 0xfffffff) as usize,
            _ => (address & 0x1fffffff) as usize
        }
    }

    pub fn mem_read32(&self, address: u32) -> u32 {
        let address = Self::translate_address(address);

        match address {
            0x1fc00000..=0x1fc80000 => unsafe { *(&self.bios[address - 0x1fc00000] as *const u8 as *const u32 ) },
            _ => panic!("address not implemented: 0x{:x}", address)
        }
    }

    pub fn mem_write32(&mut self, address: u32, value: u32) {
        let address = Self::translate_address(address);

        match address {
            0x1f801010 => self.bios_delay.write(value),
            0x1f801060 => self.ram_size = value, // TODO: actually implement
            _ => panic!("address not implemented: 0x{:x}", address)
        }
    }
}