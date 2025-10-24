use bitflags::bitflags;

bitflags! {
    pub struct CauseRegister: u32 {
        const BT = 1 << 30;
        const BD = 1 << 31;
    }
}

impl CauseRegister {
    pub fn ex_code(&self) -> u32 {
        (self.bits() >> 2) & 0x1f
    }

    pub fn ip(&self) -> u32 {
        (self.bits() >> 10) & 0x3f
    }

    pub fn ce(&self) -> bool {
        (self.bits() >> 30) & 0x1 == 1
    }

    pub fn write_exception_code(&mut self, code: u32) {
        let code = code << 2;

        *self = Self::from_bits_retain((self.bits() & !code) | code)
    }

    pub fn write(&mut self, value: u32) {
        let writable = ((value >> 8) & 0x3) << 8;

        *self = CauseRegister::from_bits_retain((self.bits() & !writable) | writable)
    }
}

pub struct COP0 {
    pub sr: StatusRegister,
    pub dcic: u32,
    pub bpc: u32,
    pub bda: u32,
    pub tar: u32,
    pub bdam: u32,
    pub bpcm: u32,
    pub cause: CauseRegister,
    pub epc: u32,
    pub bad_addr: u32,
}

bitflags! {
    pub struct StatusRegister: u32 {
        const IEC = 1 << 0;
        const KUC = 1 << 1;
        const IEP = 1 << 2;
        const KUP = 1 << 3;
        const IEO = 1 << 4;
        const KUO = 1 << 5;
        const ISOLATE_CACHE = 1 << 16;
        const SWC = 1 << 17;
        const PZ = 1 << 18;
        const CM = 1 << 19;
        const PE = 1 << 20;
        const BEV = 1 << 22;
        const COP0_ENABLE = 1 << 28;
        const GTE_ENABLE = 1 << 30;
    }
}

impl StatusRegister {
    pub fn interrupt_mask(&self) -> u32 {
        (self.bits() >> 8) & 0xff
    }

    pub fn return_from_exception(&mut self) {
        let bits23 = self.bits23();

        let bits45 = (self.bits45()) << 2;

        let mut sr_bits = self.bits();

        sr_bits &= !0xf;

        sr_bits |= bits23;
        sr_bits |= bits45;

        *self = Self::from_bits_retain(sr_bits);
    }

    pub fn bits23(&self) -> u32 {
        (self.bits() >> 2) & 0x3
    }

    pub fn bits45(&self) -> u32 {
        (self.bits() >> 4) & 0x3
    }
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
            bpcm: 0,
            cause: CauseRegister::from_bits_retain(0),
            epc: 0,
            bad_addr: 0,
        }
    }

    pub fn mfc0(&self, index: usize) -> u32 {
        match index {
            0x6 => self.tar,
            0x7 => self.dcic,
            0x8 => self.bad_addr,
            0xc => self.sr.bits(),
            0xd => self.cause.bits(),
            0xe => self.epc,
            0xf => 0x0000_0002,
            _ => todo!("mfc0 index: {index}"),
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
            0xd => self.cause.write(value),
            _ => todo!("mtc0 index: 0x{:x}", index),
        }
    }

    pub fn rfe(&mut self) {
        self.sr.return_from_exception();
    }
}
