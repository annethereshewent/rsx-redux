use std::cmp;

pub const VOLUME_MIN: i32 = -0x8000;
pub const VOLUME_MAX: i32 = 0x7fff;


#[derive(Copy, Clone, PartialEq)]
pub enum AdsrMode {
    Linear = 0,
    Exponential = 1
}

#[derive(Copy, Clone, PartialEq)]
pub enum SustainDirection {
    Increase = 0,
    Decrease = 1
}

#[derive(Copy, Clone, PartialEq)]
pub enum AdsrPhase {
    Attack,
    Sustain,
    Decay,
    Release,
    Idle
}

#[derive(Copy, Clone)]
pub struct Adsr {
    pub phase: AdsrPhase,
    attack_mode: AdsrMode,
    attack_step: u8,
    attack_shift: u8,
    attack_rate: u8,
    decay_shift: u8,
    sustain_level: u16,
    sustain_mode: AdsrMode,
    sustain_direction: SustainDirection,
    sustain_step: u8,
    sustain_shift: u8,
    sustain_rate: u8,
    release_mode: AdsrMode,
    release_shift: u8,
    value: u32,
    cycles: u16,
    volume: i16,
    current_target: i16,
    current_step: i16,
    current_rate: u16,
    current_mode: AdsrMode,
    invert_phase: bool,
    increment: u16
}

impl Adsr {
    fn clamp(value: i32, min: i32, max: i32) -> i16 {
        if value < min {
            return min as i16;
        }
        if value > max {
            return max as i16;
        }

        value as i16
    }
    pub fn new() -> Self {
        Self {
            attack_mode: AdsrMode::Linear,
            attack_step: 0,
            // attack rate is attack_step and attack_shift (per PSX-SPX) concatenated together (per Duckstation)
            attack_rate: 0,
            decay_shift: 0,
            sustain_direction: SustainDirection::Decrease,
            sustain_level: 0,
            sustain_mode: AdsrMode::Linear,
            sustain_step: 0,
            // sustain rate is sustain_step and sustain_shift (per PSX-SPX) concatenated together (per Duckstation)
            sustain_rate: 0,
            release_mode: AdsrMode::Linear,
            release_shift: 0,
            phase: AdsrPhase::Idle,
            value: 0,
            cycles: 0,
            volume: 0,
            current_target: 0,
            current_step: 0,
            attack_shift: 0,
            sustain_shift: 0,
            current_rate: 0,
            current_mode: AdsrMode::Linear,
            increment: 0,
            invert_phase: false
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
        self.attack_step = match (value >> 8) & 0x3 {
            0 => 7,
            1 => 6,
            2 => 5,
            3 => 4,
            _ => unreachable!()
        };
        self.attack_shift = ((value >> 10) & 0x1f) as u8;
        self.attack_rate = ((value >> 8) & 0x7f) as u8;
        self.attack_mode = match value >> 15 {
            0 => AdsrMode::Linear,
            1 => AdsrMode::Exponential,
            _ => unreachable!()
        };


    }

    pub fn write_upper(&mut self, value: u16) {
        self.value = (self.value & 0xffff) | (value as u32) << 16;
        self.release_shift = (value & 0x1f) as u8;
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

        self.sustain_rate = (((value >> 22)) & 0x7f) as u8;
        self.sustain_shift = (((value >> 24) & 0x1f)) as u8;
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

    pub fn tick_adsr(&mut self) {
        self.cycles -= 1;

        if self.increment > 0 {
            self.tick_envelope();
        }

        let reached_target = match self.phase {
            AdsrPhase::Attack | AdsrPhase::Idle => self.volume >= self.current_target,
            AdsrPhase::Decay | AdsrPhase::Release => self.volume <= self.current_target,
            AdsrPhase::Sustain => if self.sustain_direction == SustainDirection::Decrease {
                self.volume <= self.current_target
            } else {
                self.volume >= self.current_target
            },
        };

        if reached_target {
            self.phase = match self.phase {
                AdsrPhase::Attack => AdsrPhase::Decay,
                AdsrPhase::Decay => AdsrPhase::Sustain,
                AdsrPhase::Sustain => AdsrPhase::Sustain,
                AdsrPhase::Idle => AdsrPhase::Idle,
                AdsrPhase::Release => AdsrPhase::Idle,
            };
            self.update_envelope();
        }
    }

    fn tick_envelope(&mut self) {
        let decreasing= match self.phase {
            AdsrPhase::Attack => false,
            AdsrPhase::Decay => true,
            AdsrPhase::Sustain => self.sustain_direction == SustainDirection::Decrease,
            AdsrPhase::Release => true,
            AdsrPhase::Idle => false
        };
        let (actual_step, actual_increment) = if self.current_mode == AdsrMode::Exponential {
            if decreasing {
                // actual_step = (actual_step * self.volume as i16) >> 15;
                ((self.current_step * self.volume as i16) >> 15, self.increment)
            } else {
                if self.volume >= 0x6000 {
                    if self.current_rate < 40 {
                        (self.current_step >> 2, self.increment)
                    } else if self.current_rate >= 44 {
                        (self.current_step, self.increment >> 2)
                    } else {
                        (self.current_step >> 1, self.increment >> 1)
                    }
                } else {
                    (self.current_step, self.increment)
                }
            }
        } else {
            (self.current_step, self.increment)
        };


        self.cycles += actual_increment;

        if (self.cycles >> 15) & 1 == 0 {
            return;
        }

        self.cycles = 0;

        let new_volume = self.volume as i32 + actual_step as i32;

        if !decreasing {
            self.volume = Self::clamp(new_volume, VOLUME_MIN, VOLUME_MAX);
        } else {
            if self.invert_phase {
                self.volume = Self::clamp(new_volume, VOLUME_MIN, 0);
            } else {
                self.volume = cmp::max(new_volume, 0) as i16;
            }
        }

    }

    pub fn update_envelope(&mut self) {
        match self.phase {
            AdsrPhase::Attack => {
                self.current_target = 0x7fff;
                self.reset_envelope(self.attack_rate, self.attack_shift as i8, self.attack_step as i8, 0x7f, self.attack_mode, false, false);
            }
            AdsrPhase::Decay => {
                self.current_target = 0;
                self.reset_envelope(self.decay_shift << 2, self.decay_shift as i8, -8, 0x1f << 2, AdsrMode::Exponential, true, false);
            }
            AdsrPhase::Sustain => {
                self.current_target = 0;
                self.reset_envelope(
                    self.sustain_rate,
                    self.sustain_shift as i8,
                    self.sustain_step as i8,
                    0x7f,
                    self.sustain_mode,
                    self.sustain_direction == SustainDirection::Decrease,
                    false
                );
            }
            AdsrPhase::Release => {
                self.current_target = 0;
                self.reset_envelope(self.release_shift << 2,  self.release_shift as i8, -8, 0x1f << 2, self.release_mode, true, false);
            }
            AdsrPhase::Idle => {
                self.current_target = 0;
                self.reset_envelope(0, 0, 0 ,0, AdsrMode::Linear, false, false);
            }
        }
    }

    // per duckstation
    fn reset_envelope(&mut self, rate: u8, shift: i8, step: i8, rate_mask: u8, mode: AdsrMode, decreasing: bool, invert: bool) {
        self.cycles = 0;
        self.increment = 0x8000;
        self.current_rate = rate as u16;
        self.invert_phase = invert;

        self.current_step = if decreasing != invert || (decreasing && mode == AdsrMode::Exponential) { step as i16 } else { !step as i16};
        self.current_mode = mode;

        if rate < 44 {
            self.current_step <<= 11 - shift;
        } else if rate >= 48 {
            self.increment >>= shift - 11;

            if (rate & rate_mask) != rate_mask {
                self.increment = cmp::max(self.increment, 1);
            }
        }

    }
}

#[derive(Copy, Clone)]
pub struct Voice {
    volume_left: u16,
    volume_right: u16,
    start_address: u32,
    sample_rate: u16,
    repeat_address: u32,
    pub adsr: Adsr,
    current_adsr_volume: i16,
    current_address: u32,
    pub enabled: bool
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
            0x8 => {
                self.adsr.write_lower(value);

                if self.enabled {
                    self.adsr.update_envelope();
                }
            }
            0xa => {
                self.adsr.write_upper(value);

                if self.enabled {
                    self.adsr.update_envelope();
                }
            }
            0xc => self.current_adsr_volume = value as i16,
            0xe => self.repeat_address = value as u32 * 8,
            _ => panic!("invalid channel given: 0x{:x}", channel)
        }
    }

    pub fn generate_sample(&mut self) -> i16 {
        0
    }

    pub fn update_keyon(&mut self) {
        self.current_address = self.start_address;
        self.enabled = true;
        self.adsr.phase = AdsrPhase::Attack;

        self.adsr.update_envelope();
    }

    pub fn update_keyoff(&mut self) {
        if self.adsr.phase != AdsrPhase::Release || self.adsr.phase != AdsrPhase::Idle {
            self.adsr.phase = AdsrPhase::Release;

            self.adsr.update_envelope();
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