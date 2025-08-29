
#[derive(Copy, Clone)]
pub enum AdsrMode {
    Linear = 0,
    Exponential = 1
}

#[derive(Copy, Clone)]
pub enum SustainDirection {
    Increase = 0,
    Decrease = 1
}

#[derive(Copy, Clone)]
pub enum AdsrPhase {
    Attack,
    Sustain,
    Decay,
    Release,
    Idle
}

#[derive(Copy, Clone)]
pub struct Adsr {
    phase: AdsrPhase,
    attack_mode: AdsrMode,
    attack_shift: u16,
    attack_step: u16,
    decay_shift: u16,
    sustain_level: u16,
    sustain_mode: AdsrMode,
    sustain_direction: SustainDirection,
    sustain_shift: u16,
    sustain_step: u16,
    release_mode: AdsrMode,
    release_shift: u16,
    value: u32
}

impl Adsr {
    pub fn new() -> Self {
        Self {
            attack_mode: AdsrMode::Linear,
            attack_shift: 0,
            attack_step: 0,
            decay_shift: 0,
            sustain_direction: SustainDirection::Decrease,
            sustain_level: 0,
            sustain_mode: AdsrMode::Linear,
            sustain_shift: 0,
            sustain_step: 0,
            release_mode: AdsrMode::Linear,
            release_shift: 0,
            phase: AdsrPhase::Idle,
            value: 0
        }
    }
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
    pub fn write_lower(&mut self, value: u16) {
        self.value = (self.value & 0xffff0000) | value as u32;
        self.sustain_level = value & 0xf;
        self.decay_shift = (value >> 4) & 0xf;
        self.attack_step = match (value >> 8) & 0x3 {
            0 => 7,
            1 => 6,
            2 => 5,
            3 => 4,
            _ => unreachable!()
        };

        self.attack_shift = (value >> 10) & 0x1f;
        self.attack_mode = match value >> 15 {
            0 => AdsrMode::Linear,
            1 => AdsrMode::Exponential,
            _ => unreachable!()
        };
    }

    pub fn write_upper(&mut self, value: u16) {
        self.value = (self.value & 0xffff) | (value as u32) << 16;
        self.release_shift = value & 0x1f;
        self.release_mode = match (value >> 21) & 1 {
            0 => AdsrMode::Linear,
            1 => AdsrMode::Exponential,
            _ => unreachable!()
        };
        self.sustain_step = match (value >> 22) & 0x3 {
            0 => 7,
            1 => 6,
            2 => 5,
            3 => 4,
            _ => unreachable!()
        };

        self.sustain_shift = (value >> 24) & 0x1f;
        self.sustain_direction = match (value >> 30) & 1 {
            0 => SustainDirection::Increase,
            1 => SustainDirection::Decrease,
            _ => unreachable!()
        };

        self.sustain_mode = match value >> 31 {
            0 => AdsrMode::Linear,
            1 => AdsrMode::Exponential,
            _ => unreachable!()
        };

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
    current_adsr_volume: i16,
    current_address: u32,
    enabled: bool
}

impl Voice {
    pub fn new() -> Self {
        Self {
            adsr: Adsr::new(),
            volume_left: 0,
            volume_right: 0,
            start_address: 0,
            sample_rate: 0,
            repeat_address: 0,
            current_adsr_volume: 0,
            current_address: 0,
            enabled: false
        }
    }

    pub fn write(&mut self, channel: usize, value: u16) {
        match channel {
            0x0 => self.volume_left = value * 2,
            0x2 => self.volume_right = value * 2,
            0x4 => self.sample_rate = value,
            0x6 => self.start_address = (value as u32) * 8,
            0x8 => self.adsr.write_lower(value),
            0xa => self.adsr.write_upper(value),
            0xc => self.current_adsr_volume = value as i16,
            0xe => self.repeat_address = value as u32 * 8,
            _ => panic!("invalid channel given: 0x{:x}", channel)
        }
    }

    pub fn generate_samples(&mut self) {

    }

    pub fn update_keyon(&mut self) {
        self.current_address = self.start_address;
        self.enabled = true;
        self.adsr.phase = AdsrPhase::Attack;
    }

    pub fn update_keyoff(&mut self) {
        if self.enabled {
            self.adsr.phase = AdsrPhase::Release;
        }
    }

    pub fn read(&self, channel: usize) -> u16 {
        match channel {
            0x0 => (self.volume_left / 2) as u16,
            0x2 => (self.volume_right / 2) as u16,
            0x4 => self.sample_rate,
            0x6 => (self.start_address / 8) as u16,
            0x8 => self.adsr.value as u16,
            0xa => (self.adsr.value >> 16) as u16,
            0xc => self.current_adsr_volume as u16,
            0xe => (self.repeat_address / 8) as u16,
            _ => panic!("invalid channel given: 0x{:x}", channel)
        }
    }
}