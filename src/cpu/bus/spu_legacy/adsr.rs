pub const DECAY_STEP: i32 = -8;
pub const RELEASE_STEP: i32 = -8;

#[derive(PartialEq)]
pub enum AdsrMode {
  Linear,
  Exponential
}

#[derive(Copy, Clone, PartialEq)]
pub enum AdsrState {
  Disabled,
  Attack,
  Decay,
  Sustain,
  Release
}

#[derive(Copy, Clone, PartialEq)]
pub enum AdsrDirection {
  Increasing,
  Decreasing
}

#[derive(Copy, Clone)]
pub struct Adsr {
  pub value: u32,
  pub current_volume: i16,
  pub attack_direction: AdsrDirection,
  pub decay_direction: AdsrDirection,
  pub release_direction: AdsrDirection,
  pub endx: bool,
  pub state: AdsrState,
  pub cycles: u32,
}

impl Adsr {
  pub fn new() -> Self {
    Self {
      value: 0,
      current_volume: 0,
      attack_direction: AdsrDirection::Increasing,
      decay_direction: AdsrDirection::Increasing,
      release_direction: AdsrDirection::Increasing,
      endx: false,
      state: AdsrState::Disabled,
      cycles: 0
    }
  }

  pub fn attack_mode(&self) -> AdsrMode {
    match self.value & 0b1 {
      0 => AdsrMode::Linear,
      1 => AdsrMode::Exponential,
      _ => unreachable!("can't happen")
    }
  }

  pub fn tick(&mut self) {
    if self.cycles > 0 {
      self.cycles -= 1;
    }

    let mut shift: i32 = 0;
    let mut mode = AdsrMode::Linear;
    let mut direction = AdsrDirection::Increasing;
    let mut step: i32 = 0;
    let mut target: i32 = -1;
    let mut next = AdsrState::Disabled;

    match self.state {
      AdsrState::Disabled => (), // do nothing, the default values are above
      AdsrState::Attack => {
        shift = self.attack_shift() as i32;
        mode = self.attack_mode();
        direction = AdsrDirection::Increasing;
        target = 0x7fff;
        next = AdsrState::Decay;
        step = self.attack_step() as i32;
      }
      AdsrState::Decay => {
        shift = self.decay_shift() as i32;
        mode = AdsrMode::Exponential;
        direction = AdsrDirection::Decreasing;
        target = self.sustain_level() as i32;
        next = AdsrState::Sustain;
        step = -8;
      }
      AdsrState::Release => {
        shift = self.release_shift() as i32;
        mode = self.release_mode();
        direction = AdsrDirection::Decreasing;
        target = 0;
        next = AdsrState::Disabled;
        step = -8;
      }
      AdsrState::Sustain => {
        shift = self.sustain_shift() as i32;
        mode = self.sustain_mode();
        direction = self.sustain_direction();
        target = -1;
        next = AdsrState::Sustain;
        step = self.sustain_step();
      }
    }

    let mut cycle_shift = shift - 11;
    let mut step_shift = 11 - shift;
    if cycle_shift < 0 {
      cycle_shift = 0;
    }
    if step_shift < 0 {
      step_shift = 0;
    }

    let mut cycles = 1 << cycle_shift;
    step = step << step_shift;


    if mode == AdsrMode::Exponential {
      if direction == AdsrDirection::Increasing {
        if self.current_volume > 0x6000 {
          cycles *= 4;
        }
      } else {
        step = (step * self.current_volume as i32) >> 15;
      }
    }

    if self.cycles <= 0 {
      self.cycles += cycles;

      let mut new_volume = (self.current_volume as i32) + step;

      if new_volume > 0x7fff {
        new_volume = 0x7fff;
      }
      if new_volume < 0 {
        new_volume = 0;
      }

      self.current_volume = new_volume as i16;

      if target < 0 {
        return;
      }

      if (direction == AdsrDirection::Increasing && self.current_volume >= target as i16) || (direction == AdsrDirection::Decreasing && self.current_volume <= target as i16) {
        self.cycles = 0;
        self.state = next;
      }
    }
  }

  pub fn attack_shift(&self) -> u32 {
    (self.value >> 10) & 0b11111
  }

  pub fn attack_step(&self) -> i32 {
    (7 - ((self.value >> 8) & 0b11)) as i32
  }

  pub fn decay_shift(&self) -> u32 {
    (self.value >> 4) & 0xf
  }

  pub fn sustain_level(&self) -> u32 {
    ((self.value & 0xf) + 1) * 0x800
  }

  pub fn sustain_mode(&self) -> AdsrMode {
    match (self.value >> 31) & 0b1 {
      0 => AdsrMode::Linear,
      1 => AdsrMode::Exponential,
      _ => unreachable!("can't happen")
    }
  }

  pub fn sustain_direction(&self) -> AdsrDirection {
    match (self.value >> 30) & 0b1 {
      0 => AdsrDirection::Increasing,
      1 => AdsrDirection::Decreasing,
      _ => unreachable!("can't happen")
    }
  }

  pub fn sustain_shift(&self) -> u32 {
    (self.value >> 24) & 0x1f
  }

  pub fn sustain_step(&self) -> i32 {
    let step = ((self.value >> 22) & 0b11) as i32;

    match self.sustain_direction() {
      AdsrDirection::Decreasing => {
        -8 + step
      }
      AdsrDirection::Increasing => {
        7 - step
      }
    }
  }

  pub fn release_mode(&self) -> AdsrMode {
    match (self.value >> 21) & 0b1 {
      0 => AdsrMode::Linear,
      1 => AdsrMode::Exponential,
      _ => unreachable!("can't happen")
    }
  }

  pub fn release_shift(&self) -> u32 {
    (self.value >> 16) & 0x1f
  }

}