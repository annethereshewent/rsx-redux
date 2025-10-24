#[derive(Copy, Clone)]
pub enum BusWidth {
    Bit8 = 0,
    Bit16 = 1,
}

#[derive(Copy, Clone)]
pub enum DmaTiming {
    Normal = 0,
    UseBits = 1,
}

#[derive(Copy, Clone)]
pub enum WideDma {
    Normal = 0,
    Override = 1,
}
pub struct DelayRegister {
    pub write_delay: u32,
    pub read_delay: u32,
    pub recovery_period: bool,
    pub hold_period: bool,
    pub floating_period: bool,
    pub prestrobe_period: bool,
    pub bus_width: BusWidth,
    pub auto_increment: bool,
    pub num_addr_bits: u32,
    pub unknown: u32,
    pub timing_override: u32,
    pub error_flag: bool,
    pub dma_timing: DmaTiming,
    pub wide_dma: WideDma,
    pub wait: bool,
    pub dma_timing_override: u32,
}

impl DelayRegister {
    pub fn new() -> Self {
        Self {
            write_delay: 0,
            read_delay: 0,
            recovery_period: false,
            hold_period: false,
            floating_period: false,
            prestrobe_period: false,
            bus_width: BusWidth::Bit8,
            auto_increment: false,
            unknown: 0,
            num_addr_bits: 0,
            timing_override: 0,
            error_flag: false,
            dma_timing_override: 0,
            dma_timing: DmaTiming::Normal,
            wide_dma: WideDma::Normal,
            wait: false,
        }
    }

    pub fn write(&mut self, value: u32) {
        self.write_delay = value & 0xf;
        self.read_delay = (value >> 4) & 0xf;
        self.recovery_period = (value >> 8) & 1 == 1;
        self.hold_period = (value >> 9) & 1 == 1;
        self.floating_period = (value >> 10) & 1 == 1;
        self.prestrobe_period = (value >> 11) & 1 == 1;
        self.bus_width = match (value >> 12) & 1 {
            0 => BusWidth::Bit8,
            1 => BusWidth::Bit16,
            _ => unreachable!(),
        };
        self.auto_increment = (value >> 13) & 1 == 1;
        self.unknown = (value >> 14) & 0x3;
        self.num_addr_bits = (value >> 16) & 0x1f;
        self.dma_timing_override = (value >> 24) & 0xf;
        if (value >> 28) & 1 == 1 {
            self.error_flag = false;
        }
        self.dma_timing = match (value >> 29) & 1 {
            0 => DmaTiming::Normal,
            1 => DmaTiming::UseBits,
            _ => unreachable!(),
        };
        self.wide_dma = match (value >> 30) & 1 {
            0 => WideDma::Normal,
            1 => WideDma::Override,
            _ => unreachable!(),
        };
        self.wait = (value >> 31) == 1;
    }

    pub fn read(&self) -> u32 {
        self.write_delay
            | self.read_delay << 4
            | (self.recovery_period as u32) << 8
            | (self.hold_period as u32) << 9
            | (self.floating_period as u32) << 10
            | (self.prestrobe_period as u32) << 11
            | (self.bus_width as u32) << 12
            | (self.auto_increment as u32) << 13
            | self.unknown << 14
            | self.num_addr_bits << 16
            | self.dma_timing_override << 24
            | (self.dma_timing as u32) << 29
            | (self.wide_dma as u32) << 30
            | (self.wait as u32) << 31
    }
}
