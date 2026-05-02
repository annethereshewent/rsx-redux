use bitflags::bitflags;
use serde::{Deserialize, Serialize};

bitflags! {
    #[derive(Serialize, Deserialize)]
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
                && ((self.interrupt_flags() & self.interrupt_mask()) != 0))
    }

    pub fn read(&self) -> u32 {
        self.bits() | (self.master_interrupt_flag() as u32) << 31
    }

    pub fn set_channel_irq_if_enabled(&mut self, channel: usize) {
        let value: u32 = self.bits();
        let is_enabled_shift = 16 + channel;
        if value >> is_enabled_shift & 0x1 == 1 {
            let set_shift = 24 + channel;

            let mut interrupt_bits = value;

            interrupt_bits |= 1 << set_shift;

            *self = DmaInterruptRegister::from_bits_retain(interrupt_bits);
        }
    }
}
