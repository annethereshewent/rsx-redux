use bitflags::bitflags;

bitflags! {
    pub struct DmaInterruptRegister: u32 {
        const BUS_ERROR = 1 << 15;
        const MASTER_CHANNEL_INTERRUPT = 1 << 23;
    }
}

impl DmaInterruptRegister {
    pub fn interrupt_mask(&self) -> u32 {
        (self.bits() >> 16) & 0x7f
    }

    pub fn interrupt_flags(&self) -> u32 {
        (self.bits() >> 24) & 0x7f
    }

    pub fn master_interrupt_flag(&self) -> bool {
        self.contains(DmaInterruptRegister::BUS_ERROR)
            || (self.contains(Self::MASTER_CHANNEL_INTERRUPT)
                && (self.interrupt_flags() & self.interrupt_mask()) != 0)
    }

    pub fn read(&self) -> u32 {
        self.bits() | (self.master_interrupt_flag() as u32) << 31
    }
}
