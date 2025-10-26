use bitflags::bitflags;

#[derive(PartialEq, Copy, Clone)]
pub enum SoundRamTransferMode {
    Stop = 0,
    ManualWrite = 1,
    DMAWrite = 2,
    DMARead = 3,
}

/*
  15    SPU Enable              (0=Off, 1=On)       (Don't care for CD Audio)
  14    Mute SPU                (0=Mute, 1=Unmute)  (Don't care for CD Audio)
  13-10 Noise Frequency Shift   (0..0Fh = Low .. High Frequency)
  9-8   Noise Frequency Step    (0..03h = Step "4,5,6,7")
  7     Reverb Master Enable    (0=Disabled, 1=Enabled)
  6     IRQ9 Enable (0=Disabled/Acknowledge, 1=Enabled; only when Bit15=1)
  5-4   Sound RAM Transfer Mode (0=Stop, 1=ManualWrite, 2=DMAwrite, 3=DMAread)
  3     External Audio Reverb   (0=Off, 1=On)
  2     CD Audio Reverb         (0=Off, 1=On) (for CD-DA and XA-ADPCM)
  1     External Audio Enable   (0=Off, 1=On)
  0     CD Audio Enable         (0=Off, 1=On) (for CD-DA and XA-ADPCM)
*/
bitflags! {
    pub struct SpuControlRegister: u16 {
        const CD_AUDIO_ENABLE = 1 << 0;
        const EXTERNAL_AUDIO_ENABLE = 1 << 1;
        const CD_AUDIO_REVERB = 1 << 2;
        const EXTERNAL_AUDIO_REVER = 1 << 3;
        const IRQ9_ENABLE = 1 << 6;
        const REVERB_MASTER_ENABLE = 1 << 7;
        const MUTE_SPU = 1 << 14;
        const SPU_ENABLE = 1 << 15;
    }
}

impl SpuControlRegister {
    pub fn sound_ram_transfer_mode(&self) -> SoundRamTransferMode {
        match (self.bits() >> 4) & 0x3 {
            0 => SoundRamTransferMode::Stop,
            1 => SoundRamTransferMode::ManualWrite,
            2 => SoundRamTransferMode::DMAWrite,
            3 => SoundRamTransferMode::DMARead,
            _ => unreachable!(),
        }
    }
}
