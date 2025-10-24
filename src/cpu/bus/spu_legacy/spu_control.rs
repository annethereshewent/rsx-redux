#[derive(PartialEq)]
pub enum RamTransferMode {
    Stop,
    ManualWrite,
    DmaWrite,
    DmaRead,
}

pub struct SpuControlRegister {
    val: u16,
}

impl SpuControlRegister {
    pub fn new() -> Self {
        Self { val: 0 }
    }

    pub fn spu_enable(&self) -> bool {
        (self.val >> 15) & 0b1 == 1
    }

    pub fn mute_spu(&self) -> bool {
        (self.val >> 14) & 0b1 == 0
    }

    pub fn noise_frequency_shift(&self) -> u16 {
        (self.val >> 10) & 0xf
    }

    pub fn noise_frequency_step(&self) -> u16 {
        (self.val >> 8) & 0x3
    }

    pub fn reverb_master_enable(&self) -> bool {
        (self.val >> 7) & 0b1 == 1
    }

    pub fn irq9_enable(&self) -> bool {
        (self.val >> 6) == 1 && self.spu_enable()
    }

    pub fn transfer_mode(&self) -> RamTransferMode {
        match (self.val >> 4) & 0x3 {
            0 => RamTransferMode::Stop,
            1 => RamTransferMode::ManualWrite,
            2 => RamTransferMode::DmaWrite,
            3 => RamTransferMode::DmaRead,
            _ => unreachable!("can't happen"),
        }
    }

    pub fn external_audio_reverb(&self) -> bool {
        (self.val >> 3) & 0b1 == 1
    }

    pub fn cd_audio_reverb(&self) -> bool {
        (self.val >> 2) & 0b1 == 1
    }

    pub fn external_audio_enable(&self) -> bool {
        (self.val >> 1) & 0b1 == 1
    }

    pub fn cd_audio_enable(&self) -> bool {
        self.val & 0b1 == 1
    }

    pub fn write(&mut self, val: u16) {
        self.val = val;
    }

    pub fn read(&self) -> u16 {
        self.val
    }
}
