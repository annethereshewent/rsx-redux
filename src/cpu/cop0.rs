use super::StatusRegister;

pub struct COP0 {
    pub sr: StatusRegister,
    pub dcic: u32,
    pub bpc: u32,
    pub bda: u32,
    pub tar: u32,
    pub bdam: u32,
    pub bpcm: u32
}

impl COP0 {
    pub fn new() -> Self {
        Self {
            sr: StatusRegister::from_bits_retain(0),
            dcic: 0,
            bpc: 0,
            bda: 0,
            tar: 0,
            bdam: 0,
            bpcm: 0
        }
    }

    pub fn mfc0(&self, index: usize) -> u32 {
        match index {
            0xc => self.sr.bits(),
            _ => todo!("mfc0 index: {index}")
        }
    }

    pub fn mtc0(&mut self, index: usize, value: u32) {
        match index {
            0x3 => self.bpc = value,
            0x5 => self.bda = value,
            0x6 => (), // read only
            0x7 => self.dcic = value,
            0x9 => self.bdam = value,
            0xb => self.bpcm = value,
            0xc => self.sr = StatusRegister::from_bits_retain(value),
            0xd => (), // cause, read only
            _ => todo!("mtc0 index: 0x{:x}", index)
        }
    }

    pub fn rfe(&mut self) {
        todo!("rfe");
    }
}