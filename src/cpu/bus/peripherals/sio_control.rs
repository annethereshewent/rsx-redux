use bitflags::bitflags;

bitflags! {
    pub struct SIOControl: u16 {
        const TX_ENABLE = 1;
        const DTR_OUT = 1 << 1;
        const RX_ENABLE = 1 << 2;
        const TX_OUTPUT = 1 << 3;
        const ACK = 1 << 4; // TODO: Reset SIO_STAT.Bits 3,4,5,9
        const RTS_OUTPUT = 1 << 5;
        const RESET = 1 << 6;
        const UNKNOWN = 1 << 7;
        const TX_INTERRUPT_ENABLE = 1 << 10;
        const RX_INTERRUPT_ENABLE = 1 << 11;
        const DSR_INTERRUPT_ENABLE = 1 << 12;
        const SIO_PORT_SELECT = 1 << 13;
    }
}

impl SIOControl {
    pub fn interrupt_mode_num_bytes(&self) -> u16 {
        let value = (self.bits() >> 8) & 0x3;

        1 << value
    }
}
