use bitflags::bitflags;

bitflags! {
    pub struct HntmaskRegister: u8 {
        const ENBFEMPT = 1 << 3;
        const ENBFWRDY = 1 << 4;
    }
}

impl HntmaskRegister {
    pub fn read(&self) -> u8 {
        self.bits() | (0x7 << 5)
    }

    pub fn write(&mut self, value: u8) {
        *self = Self::from_bits_retain(value)
    }

    pub fn enable_irq(&self) -> u8 {
        self.bits() & 0x1f
    }
}