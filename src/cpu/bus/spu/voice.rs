use bitflags::bitflags;

/*
  ____lower 16bit (at 1F801C08h+N*10h)___________________________________
  15    Attack Mode       (0=Linear, 1=Exponential)
  -     Attack Direction  (Fixed, always Increase) (until Level 7FFFh)
  14-10 Attack Shift      (0..1Fh = Fast..Slow)
  9-8   Attack Step       (0..3 = "+7,+6,+5,+4")
  -     Decay Mode        (Fixed, always Exponential)
  -     Decay Direction   (Fixed, always Decrease) (until Sustain Level)
  7-4   Decay Shift       (0..0Fh = Fast..Slow)
  -     Decay Step        (Fixed, always "-8")
  3-0   Sustain Level     (0..0Fh)  ;Level=(N+1)*800h
  ____upper 16bit (at 1F801C0Ah+N*10h)___________________________________
  31    Sustain Mode      (0=Linear, 1=Exponential)
  30    Sustain Direction (0=Increase, 1=Decrease) (until Key OFF flag)
  29    Not used?         (should be zero)
  28-24 Sustain Shift     (0..1Fh = Fast..Slow)
  23-22 Sustain Step      (0..3 = "+7,+6,+5,+4" or "-8,-7,-6,-5") (inc/dec)
  21    Release Mode      (0=Linear, 1=Exponential)
  -     Release Direction (Fixed, always Decrease) (until Level 0000h)
  20-16 Release Shift     (0..1Fh = Fast..Slow)
  -     Release Step      (Fixed, always "-8")
*/
bitflags! {
    #[derive(Copy, Clone)]
    pub struct Adsr: u32 {

    }
}

#[derive(Copy, Clone)]
pub struct Voice {
    volume_left: u16,
    volume_right: u16,
    start_address: u32,
    sample_rate: u16,
    repeat_address: u32,
    adsr: Adsr,
    current_adsr_volume: i16

}

impl Voice {
    pub fn new() -> Self {
        Self {
            adsr: Adsr::from_bits_retain(0),
            volume_left: 0,
            volume_right: 0,
            start_address: 0,
            sample_rate: 0,
            repeat_address: 0,
            current_adsr_volume: 0
        }
    }

    pub fn write(&mut self, channel: usize, value: u16) {
        match channel {
            0x0 => self.volume_left = value * 2,
            0x2 => self.volume_right = value * 2,
            0x4 => self.sample_rate = value,
            0x6 => self.start_address = (value as u32) * 8,
            0x8 => self.adsr = Adsr::from_bits_retain((self.adsr.bits() & 0xffff000) | (value as u32)),
            0xa => self.adsr = Adsr::from_bits_retain((self.adsr.bits() & 0xffff) | (value as u32) << 16),
            0xc => self.current_adsr_volume = value as i16,
            0xe => self.repeat_address = value as u32 * 8,
            _ => panic!("invalid channel given: 0x{:x}", channel)
        }
    }

    pub fn read(&self, channel: usize) -> u16 {
        match channel {
            0x0 => (self.volume_left / 2) as u16,
            0x2 => (self.volume_right / 2) as u16,
            0x4 => self.sample_rate,
            0x6 => (self.start_address / 8) as u16,
            0x8 => (self.adsr.bits() & 0xffff) as u16,
            0xa => (self.adsr.bits() >> 16) as u16,
            0xc => self.current_adsr_volume as u16,
            0xe => (self.repeat_address / 8) as u16,
            _ => panic!("invalid channel given: 0x{:x}", channel)
        }
    }
}