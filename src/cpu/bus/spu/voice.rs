use std::{cmp, i16};

use crate::cpu::bus::{registers::interrupt_register::InterruptRegister, spu::{SoundRam, SPU}};

pub const VOLUME_MIN: i32 = -0x8000;
pub const VOLUME_MAX: i32 = 0x7fff;

const POS_FILTER_TABLE: [i8; 16] = [0, 60, 115, 98, 122, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
const NEG_FILTER_TABLE: [i8; 16] = [0, 0, -52, -55, -60, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

const NUM_BLOCK_SAMPLES: usize = 28;

const GAUSSIAN_TABLE: [i32; 0x200] = [
    -0x001, -0x001, -0x001, -0x001, -0x001, -0x001, -0x001, -0x001, //
    -0x001, -0x001, -0x001, -0x001, -0x001, -0x001, -0x001, -0x001, //
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0001, //
    0x0001, 0x0001, 0x0001, 0x0002, 0x0002, 0x0002, 0x0003, 0x0003, //
    0x0003, 0x0004, 0x0004, 0x0005, 0x0005, 0x0006, 0x0007, 0x0007, //
    0x0008, 0x0009, 0x0009, 0x000A, 0x000B, 0x000C, 0x000D, 0x000E, //
    0x000F, 0x0010, 0x0011, 0x0012, 0x0013, 0x0015, 0x0016, 0x0018, // entry
    0x0019, 0x001B, 0x001C, 0x001E, 0x0020, 0x0021, 0x0023, 0x0025, // 000..07F
    0x0027, 0x0029, 0x002C, 0x002E, 0x0030, 0x0033, 0x0035, 0x0038, //
    0x003A, 0x003D, 0x0040, 0x0043, 0x0046, 0x0049, 0x004D, 0x0050, //
    0x0054, 0x0057, 0x005B, 0x005F, 0x0063, 0x0067, 0x006B, 0x006F, //
    0x0074, 0x0078, 0x007D, 0x0082, 0x0087, 0x008C, 0x0091, 0x0096, //
    0x009C, 0x00A1, 0x00A7, 0x00AD, 0x00B3, 0x00BA, 0x00C0, 0x00C7, //
    0x00CD, 0x00D4, 0x00DB, 0x00E3, 0x00EA, 0x00F2, 0x00FA, 0x0101, //
    0x010A, 0x0112, 0x011B, 0x0123, 0x012C, 0x0135, 0x013F, 0x0148, //
    0x0152, 0x015C, 0x0166, 0x0171, 0x017B, 0x0186, 0x0191, 0x019C, //
    0x01A8, 0x01B4, 0x01C0, 0x01CC, 0x01D9, 0x01E5, 0x01F2, 0x0200, //
    0x020D, 0x021B, 0x0229, 0x0237, 0x0246, 0x0255, 0x0264, 0x0273, //
    0x0283, 0x0293, 0x02A3, 0x02B4, 0x02C4, 0x02D6, 0x02E7, 0x02F9, //
    0x030B, 0x031D, 0x0330, 0x0343, 0x0356, 0x036A, 0x037E, 0x0392, //
    0x03A7, 0x03BC, 0x03D1, 0x03E7, 0x03FC, 0x0413, 0x042A, 0x0441, //
    0x0458, 0x0470, 0x0488, 0x04A0, 0x04B9, 0x04D2, 0x04EC, 0x0506, //
    0x0520, 0x053B, 0x0556, 0x0572, 0x058E, 0x05AA, 0x05C7, 0x05E4, // entry
    0x0601, 0x061F, 0x063E, 0x065C, 0x067C, 0x069B, 0x06BB, 0x06DC, // 080..0FF
    0x06FD, 0x071E, 0x0740, 0x0762, 0x0784, 0x07A7, 0x07CB, 0x07EF, //
    0x0813, 0x0838, 0x085D, 0x0883, 0x08A9, 0x08D0, 0x08F7, 0x091E, //
    0x0946, 0x096F, 0x0998, 0x09C1, 0x09EB, 0x0A16, 0x0A40, 0x0A6C, //
    0x0A98, 0x0AC4, 0x0AF1, 0x0B1E, 0x0B4C, 0x0B7A, 0x0BA9, 0x0BD8, //
    0x0C07, 0x0C38, 0x0C68, 0x0C99, 0x0CCB, 0x0CFD, 0x0D30, 0x0D63, //
    0x0D97, 0x0DCB, 0x0E00, 0x0E35, 0x0E6B, 0x0EA1, 0x0ED7, 0x0F0F, //
    0x0F46, 0x0F7F, 0x0FB7, 0x0FF1, 0x102A, 0x1065, 0x109F, 0x10DB, //
    0x1116, 0x1153, 0x118F, 0x11CD, 0x120B, 0x1249, 0x1288, 0x12C7, //
    0x1307, 0x1347, 0x1388, 0x13C9, 0x140B, 0x144D, 0x1490, 0x14D4, //
    0x1517, 0x155C, 0x15A0, 0x15E6, 0x162C, 0x1672, 0x16B9, 0x1700, //
    0x1747, 0x1790, 0x17D8, 0x1821, 0x186B, 0x18B5, 0x1900, 0x194B, //
    0x1996, 0x19E2, 0x1A2E, 0x1A7B, 0x1AC8, 0x1B16, 0x1B64, 0x1BB3, //
    0x1C02, 0x1C51, 0x1CA1, 0x1CF1, 0x1D42, 0x1D93, 0x1DE5, 0x1E37, //
    0x1E89, 0x1EDC, 0x1F2F, 0x1F82, 0x1FD6, 0x202A, 0x207F, 0x20D4, //
    0x2129, 0x217F, 0x21D5, 0x222C, 0x2282, 0x22DA, 0x2331, 0x2389, // entry
    0x23E1, 0x2439, 0x2492, 0x24EB, 0x2545, 0x259E, 0x25F8, 0x2653, // 100..17F
    0x26AD, 0x2708, 0x2763, 0x27BE, 0x281A, 0x2876, 0x28D2, 0x292E, //
    0x298B, 0x29E7, 0x2A44, 0x2AA1, 0x2AFF, 0x2B5C, 0x2BBA, 0x2C18, //
    0x2C76, 0x2CD4, 0x2D33, 0x2D91, 0x2DF0, 0x2E4F, 0x2EAE, 0x2F0D, //
    0x2F6C, 0x2FCC, 0x302B, 0x308B, 0x30EA, 0x314A, 0x31AA, 0x3209, //
    0x3269, 0x32C9, 0x3329, 0x3389, 0x33E9, 0x3449, 0x34A9, 0x3509, //
    0x3569, 0x35C9, 0x3629, 0x3689, 0x36E8, 0x3748, 0x37A8, 0x3807, //
    0x3867, 0x38C6, 0x3926, 0x3985, 0x39E4, 0x3A43, 0x3AA2, 0x3B00, //
    0x3B5F, 0x3BBD, 0x3C1B, 0x3C79, 0x3CD7, 0x3D35, 0x3D92, 0x3DEF, //
    0x3E4C, 0x3EA9, 0x3F05, 0x3F62, 0x3FBD, 0x4019, 0x4074, 0x40D0, //
    0x412A, 0x4185, 0x41DF, 0x4239, 0x4292, 0x42EB, 0x4344, 0x439C, //
    0x43F4, 0x444C, 0x44A3, 0x44FA, 0x4550, 0x45A6, 0x45FC, 0x4651, //
    0x46A6, 0x46FA, 0x474E, 0x47A1, 0x47F4, 0x4846, 0x4898, 0x48E9, //
    0x493A, 0x498A, 0x49D9, 0x4A29, 0x4A77, 0x4AC5, 0x4B13, 0x4B5F, //
    0x4BAC, 0x4BF7, 0x4C42, 0x4C8D, 0x4CD7, 0x4D20, 0x4D68, 0x4DB0, //
    0x4DF7, 0x4E3E, 0x4E84, 0x4EC9, 0x4F0E, 0x4F52, 0x4F95, 0x4FD7, // entry
    0x5019, 0x505A, 0x509A, 0x50DA, 0x5118, 0x5156, 0x5194, 0x51D0, // 180..1FF
    0x520C, 0x5247, 0x5281, 0x52BA, 0x52F3, 0x532A, 0x5361, 0x5397, //
    0x53CC, 0x5401, 0x5434, 0x5467, 0x5499, 0x54CA, 0x54FA, 0x5529, //
    0x5558, 0x5585, 0x55B2, 0x55DE, 0x5609, 0x5632, 0x565B, 0x5684, //
    0x56AB, 0x56D1, 0x56F6, 0x571B, 0x573E, 0x5761, 0x5782, 0x57A3, //
    0x57C3, 0x57E2, 0x57FF, 0x581C, 0x5838, 0x5853, 0x586D, 0x5886, //
    0x589E, 0x58B5, 0x58CB, 0x58E0, 0x58F4, 0x5907, 0x5919, 0x592A, //
    0x593A, 0x5949, 0x5958, 0x5965, 0x5971, 0x597C, 0x5986, 0x598F, //
    0x5997, 0x599E, 0x59A4, 0x59A9, 0x59AD, 0x59B0, 0x59B2, 0x59B3  //
];


#[derive(Copy, Clone, PartialEq, Debug)]
pub enum EnvelopeMode {
    Linear = 0,
    Exponential = 1
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum EnvelopeDirection {
    Increase = 0,
    Decrease = 1
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum AdsrPhase {
    Attack,
    Sustain,
    Decay,
    Release,
    Idle
}

#[derive(Copy, Clone)]
struct Envelope {
    counter: u32,
    increment: u16,
    step: i16,
    rate: u8,
    direction: EnvelopeDirection,
    mode: EnvelopeMode,
    invert_phase: bool,
    pub volume: i16
}

impl Envelope {
    pub fn new() -> Self {
        Self {
            counter: 0,
            increment: 0,
            step: 0,
            rate: 0,
            direction: EnvelopeDirection::Decrease,
            mode: EnvelopeMode::Linear,
            invert_phase: false,
            volume: 0
        }
    }
    // per duckstation
    fn reset(&mut self, rate: u8, shift: i8, step: i8, rate_mask: u8, mode: EnvelopeMode, direction: EnvelopeDirection, invert: bool) {
        self.counter = 0;
        self.increment = 0x8000;
        self.rate = rate as u8;
        self.invert_phase = invert;
        self.direction = direction;

        let decreasing = direction == EnvelopeDirection::Decrease;

        let invert_phase = self.invert_phase && !(decreasing && mode == EnvelopeMode::Exponential);

        self.step = if invert_phase {
            !step as i16
        } else {
            step as i16
        };

        self.mode = mode;

        if rate < 44 {
            self.step <<= 11 - shift;
        } else if rate >= 48 {
            self.increment >>= shift - 11;

            if (rate & rate_mask) != rate_mask {
                self.increment = cmp::max(self.increment, 1);
            }
        }
    }

    fn tick(&mut self) -> bool {
        let (actual_step, actual_increment) = if self.mode == EnvelopeMode::Exponential {
            if self.direction == EnvelopeDirection::Decrease {
                (((self.step as i32 * self.volume as i32) >> 15) as i16, self.increment)
            } else {
                if self.volume >= 0x6000 {
                    if self.rate < 40 {
                        (self.step >> 2, self.increment)
                    } else if self.rate >= 44 {
                        (self.step, self.increment >> 2)
                    } else {
                        (self.step >> 1, self.increment >> 1)
                    }
                } else {
                    (self.step, self.increment)
                }
            }
        } else {
            (self.step, self.increment)
        };

        self.counter += actual_increment as u32;

        if (self.counter >> 15) & 1 == 0 {
            return true;
        }

        self.counter = 0;

        let new_volume = self.volume as i32 + actual_step as i32;

        if self.direction == EnvelopeDirection::Increase {
            self.volume = SPU::clamp(new_volume, VOLUME_MIN, VOLUME_MAX);

            if self.step < 0 {
                self.volume as i32 != VOLUME_MIN
            } else {
                self.volume as i32 != VOLUME_MAX
            }
        } else {
            if self.invert_phase {
                self.volume = SPU::clamp(new_volume, VOLUME_MIN, 0);
            } else {
                self.volume = cmp::max(new_volume, 0) as i16;
            }

            self.volume == 0
        }
    }
}

#[derive(Copy, Clone)]
pub struct Adsr {
    pub phase: AdsrPhase,
    attack_mode: EnvelopeMode,
    attack_step: i8,
    attack_shift: u8,
    attack_rate: u8,
    decay_shift: u8,
    sustain_level: u16,
    sustain_mode: EnvelopeMode,
    sustain_direction: EnvelopeDirection,
    sustain_step: i8,
    sustain_shift: u8,
    sustain_rate: u8,
    release_mode: EnvelopeMode,
    release_shift: u8,
    value: u32,
    current_target: i16,
    envelope: Envelope
}

impl Adsr {
    pub fn new() -> Self {
        Self {
            attack_mode: EnvelopeMode::Linear,
            attack_step: 0,
            // attack rate is attack_step and attack_shift (per PSX-SPX) concatenated together (per Duckstation)
            attack_rate: 0,
            decay_shift: 0,
            sustain_direction: EnvelopeDirection::Decrease,
            sustain_level: 0,
            sustain_mode: EnvelopeMode::Linear,
            sustain_step: 0,
            // sustain rate is sustain_step and sustain_shift (per PSX-SPX) concatenated together (per Duckstation)
            sustain_rate: 0,
            release_mode: EnvelopeMode::Linear,
            release_shift: 0,
            phase: AdsrPhase::Idle,
            value: 0,
            current_target: 0,
            attack_shift: 0,
            sustain_shift: 0,
            envelope: Envelope::new()
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
        self.sustain_level = ((value & 0xf) + 1) * 0x800;

        self.attack_step = (7 - ((value >> 8) & 0x3)) as i8;

        self.attack_shift = ((value >> 10) & 0x1f) as u8;
        self.attack_rate = ((value >> 8) & 0x7f) as u8;
        self.attack_mode = match value >> 15 {
            0 => EnvelopeMode::Linear,
            1 => EnvelopeMode::Exponential,
            _ => unreachable!()
        };


    }

    pub fn write_upper(&mut self, value: u16) {
        self.value = (self.value & 0xffff) | (value as u32) << 16;
        self.release_shift = (value & 0x1f) as u8;
        self.release_mode = match (value >> 5) & 1 {
            0 => EnvelopeMode::Linear,
            1 => EnvelopeMode::Exponential,
            _ => unreachable!()
        };

        self.sustain_direction = match (value >> 14) & 1 {
            0 => EnvelopeDirection::Increase,
            1 => EnvelopeDirection::Decrease,
            _ => unreachable!()
        };

        self.sustain_step = if self.sustain_direction == EnvelopeDirection::Increase {
            7 - ((value >> 6) & 0x3) as i8
        } else {
            -8 + ((value >> 6) & 0x3) as i8
        };

        self.sustain_rate = (((value >> 6)) & 0x7f) as u8;
        self.sustain_shift = (((value >> 8) & 0x1f)) as u8;

        self.sustain_mode = match value >> 15 {
            0 => EnvelopeMode::Linear,
            1 => EnvelopeMode::Exponential,
            _ => unreachable!()
        };

    }

    pub fn tick(&mut self) {
        if self.envelope.increment > 0 {
            self.envelope.tick();
        }

        if self.current_target < 0 {
            return;
        }

        let reached_target = match self.phase {
            AdsrPhase::Attack | AdsrPhase::Idle => self.envelope.volume >= self.current_target,
            AdsrPhase::Decay | AdsrPhase::Release => self.envelope.volume <= self.current_target,
            AdsrPhase::Sustain => if self.sustain_direction == EnvelopeDirection::Decrease {
                self.envelope.volume <= self.current_target
            } else {
                self.envelope.volume >= self.current_target
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

    pub fn update_envelope(&mut self) {
        match self.phase {
            AdsrPhase::Attack => {
                self.current_target = 0x7fff;
                self.envelope.reset(
                    self.attack_rate,
                    self.attack_shift as i8,
                    self.attack_step as i8,
                    0x7f,
                    self.attack_mode,
                    EnvelopeDirection::Increase,
                    false
                );
            }
            AdsrPhase::Decay => {
                self.current_target = self.sustain_level as i16;
                self.envelope.reset(
                    self.decay_shift << 2,
                    self.decay_shift as i8,
                    -8,
                    0x1f << 2,
                    EnvelopeMode::Exponential,
                    EnvelopeDirection::Decrease,
                    false
                );
            }
            AdsrPhase::Sustain => {
                self.current_target = -1;
                self.envelope.reset(
                    self.sustain_rate,
                    self.sustain_shift as i8,
                    if self.sustain_direction == EnvelopeDirection::Decrease { !self.sustain_step as i8 } else { self.sustain_step as i8 },
                    0x7f,
                    self.sustain_mode,
                    self.sustain_direction,
                    false
                );
            }
            AdsrPhase::Release => {
                self.current_target = 0;
                self.envelope.reset(
                    self.release_shift << 2,
                    self.release_shift as i8,
                    -8,
                    0x1f << 2,
                    self.release_mode,
                    EnvelopeDirection::Decrease,
                    false
                );
            }
            AdsrPhase::Idle => {
                self.current_target = 0;
                self.envelope.reset(0, 0, 0 ,0, EnvelopeMode::Linear, EnvelopeDirection::Increase, false);
            }
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct ADPCMBlock {
    pub filter: u8,
    pub shift: u8,
    pub loop_end: bool,
    pub loop_repeat: bool,
    pub loop_start: bool,
    pub sample_blocks: [u8; 14]
}

impl ADPCMBlock {
    pub fn new() -> Self {
        Self {
            filter: 0,
            shift: 0,
            loop_end: false,
            loop_repeat: false,
            loop_start: false,
            sample_blocks: [0; 14]
        }
    }
}

#[derive(Copy, Clone)]
pub struct Voice {
    start_address: u32,
    sample_rate: u16,
    repeat_address: u32,
    pub adsr: Adsr,
    current_address: u32,
    pitch_counter: u32,
    has_samples: bool,
    is_first_block: bool,
    last_decoded_samples: [i16; 2],
    last_gaussian_samples: [i16; 4],
    current_samples: [i16; NUM_BLOCK_SAMPLES],
    current_block: ADPCMBlock,
    pub last_volume: i32,
    left_envelope: Envelope,
    right_envelope: Envelope,
    using_left_envelope: bool,
    using_right_envelope: bool,
    ignore_loop_address: bool
}

impl Voice {
    pub fn new() -> Self {
        Self {
            adsr: Adsr::new(),
            start_address: 0,
            sample_rate: 0,
            repeat_address: 0,
            current_address: 0,
            pitch_counter: 0,
            has_samples: false,
            is_first_block: false,
            last_decoded_samples: [0; 2],
            current_samples: [0; NUM_BLOCK_SAMPLES],
            current_block: ADPCMBlock::new(),
            last_volume: 0,
            left_envelope: Envelope::new(),
            right_envelope: Envelope::new(),
            using_left_envelope: false,
            using_right_envelope: false,
            last_gaussian_samples: [0; 4],
            ignore_loop_address: false
        }
    }

    fn get_sweep_params(value: u16) -> (u8, i8, i8, EnvelopeMode, EnvelopeDirection, bool) {


        let sweep_shift = (value >> 2) & 0x1f;
        let sweep_rate = value & 0x7f;

        let invert_phase = (value >> 12) & 1 == 1;

        let direction = match (value >> 13) & 1 {
            0 => EnvelopeDirection::Increase,
            1 => EnvelopeDirection::Decrease,
            _ => unreachable!()
        };

        let sweep_step = if direction == EnvelopeDirection::Increase {
            7 - ((value & 0x3) as i8)
        } else {
            -8 + ((value & 0x3) as i8)
        };

        let mode = match (value >> 14) & 1 {
            0 => EnvelopeMode::Linear,
            1 => EnvelopeMode::Exponential,
            _ => unreachable!()
        };

        (sweep_rate as u8, sweep_shift as i8, sweep_step, mode, direction, invert_phase)
    }

    pub fn write(&mut self, channel: usize, value: u16) {
        match channel {
            0x0 => {
                if (value >> 15) & 1 == 1 {
                    let (sweep_rate, sweep_shift, sweep_step, mode, direction, invert_phase) = Self::get_sweep_params(value);

                    self.left_envelope.reset(sweep_rate as u8, sweep_shift as i8, sweep_step, 0x7f, mode, direction, invert_phase);
                    self.using_right_envelope = self.right_envelope.increment > 0;

                    self.using_left_envelope = self.left_envelope.increment > 0;
                } else {
                    self.using_left_envelope = false;
                    self.left_envelope.volume = (value * 2) as i16;

                }
            }
            0x2 => {
                if (value >> 15) & 1 == 1 {
                    let (sweep_rate, sweep_shift, sweep_step, mode, direction, invert_phase) = Self::get_sweep_params(value);

                    self.right_envelope.reset(sweep_rate as u8, sweep_shift as i8, sweep_step, 0x7f, mode, direction, invert_phase);
                    self.using_right_envelope = self.right_envelope.increment > 0;
                } else {
                    self.using_right_envelope = false;
                    self.right_envelope.volume = (value * 2) as i16;
                }
            }
            0x4 => self.sample_rate = value,
            0x6 => self.start_address = (value as u32) * 8,
            0x8 => {
                self.adsr.write_lower(value);

                if self.adsr.phase != AdsrPhase::Idle {
                    self.adsr.update_envelope();
                }
            }
            0xa => {
                self.adsr.write_upper(value);

                if self.adsr.phase != AdsrPhase::Idle {
                    self.adsr.update_envelope();
                }
            }
            0xc => self.adsr.envelope.volume = value as i16,
            0xe => {
                self.ignore_loop_address = !self.is_first_block && self.adsr.phase == AdsrPhase::Idle;
                self.repeat_address = value as u32 * 8;
            }
            _ => panic!("invalid channel given: 0x{:x}", channel)
        }
    }

    fn interpolate(&mut self, interpolation_index: usize, sample_index: usize) -> i32 {
        let oldest = self.get_interpolate_sample(sample_index as isize - 3);
        let older = self.get_interpolate_sample(sample_index as isize - 2);
        let old = self.get_interpolate_sample(sample_index as isize - 1);
        let new = self.get_interpolate_sample(sample_index as isize);

        let mut out = (GAUSSIAN_TABLE[0xff - interpolation_index] * oldest as i32) >> 15;
        out += (GAUSSIAN_TABLE[0x1ff - interpolation_index] * older) >> 15;
        out += (GAUSSIAN_TABLE[0x100 - interpolation_index] * old) >> 15;
        out += (GAUSSIAN_TABLE[interpolation_index] * new) >> 15;

        out
    }

    pub fn generate_samples(
        &mut self,
        sound_ram: &SoundRam,
        irq_address: u32,
        irq9_enable: bool,
        interrupt_register: &mut InterruptRegister,
        pitch_modulate: bool,
        previous_volume: i32,
        noise_enable: bool
    ) -> (i32, i32, bool) {
        if self.adsr.phase == AdsrPhase::Idle && !irq9_enable {
            return (0, 0, false);
        }

        let mut endx = false;

        if !self.has_samples {
            if irq9_enable && (self.current_address == irq_address || ((self.current_address + 8) & 0x7_ffff) == irq_address) {
                interrupt_register.insert(InterruptRegister::SPU);
            }
            let block = self.read_adpcm_block(sound_ram);
            self.decode_adpcm_block(&block);

            self.has_samples = true;

            if self.current_block.loop_start && !self.ignore_loop_address {
                self.repeat_address = self.current_address;
            }
        }

        let interpolation_index = (self.pitch_counter >> 4) & 0xff;
        let sample_index = self.pitch_counter >> 12;

        let volume = if self.adsr.envelope.volume > 0 {
            let sample = if noise_enable {
                todo!("noise");
            } else {
                self.interpolate(interpolation_index as usize, sample_index as usize)
            };

            (sample * self.adsr.envelope.volume as i32) >> 15
        } else {
            0
        };

        self.last_volume = volume;

        let mut step = self.sample_rate as u32;

        if self.adsr.phase != AdsrPhase::Idle {
            self.adsr.tick();
        }

        if pitch_modulate {
            let factor = (SPU::clamp(previous_volume, -0x8000, 0x7fff) as i32 + 0x80000) as u32;

            step = ((step * factor) >> 15) as i16 as u16 as u32;
        }

        if step > 0x3fff {
            step = 0x4000;
        }

        self.pitch_counter += step;

        let mut sample_index = self.pitch_counter >> 12;

        if sample_index >= NUM_BLOCK_SAMPLES as u32{
            self.is_first_block = false;
            sample_index -= NUM_BLOCK_SAMPLES as u32;

            self.has_samples = false;

            self.pitch_counter &= 0xfff;
            self.pitch_counter |= sample_index << 12;

            // self.current_address = (self.current_address + 2) & 0x7_ffff;

            if self.current_block.loop_end {
                endx = true;

                self.current_address = self.repeat_address;

                if !self.current_block.loop_repeat && !noise_enable {
                    self.adsr.envelope.volume = 0;
                    self.adsr.phase = AdsrPhase::Idle;
                }
            }
        }

        let left = (volume * self.left_envelope.volume as i32) >> 15;
        let right = (volume * self.right_envelope.volume as i32) >> 15;

        if self.using_left_envelope {
            self.using_left_envelope = self.left_envelope.tick();
        }
        if self.using_right_envelope {
            self.using_right_envelope = self.right_envelope.tick();
        }

        (left, right, endx)
    }

    fn get_interpolate_sample(&self, index: isize) -> i32 {
        if index < 0 {
            self.last_gaussian_samples[(index + 3) as usize] as i32
        } else {
            self.current_samples[index as usize] as i32
        }
    }

    fn decode_adpcm_block(&mut self, block: &ADPCMBlock) {
        let positive_filter = POS_FILTER_TABLE[block.filter as usize];
        let negative_filter = NEG_FILTER_TABLE[block.filter as usize];

        let mut j = 0;

        for i in 24..self.current_samples.len() {
            self.last_gaussian_samples[j] = self.current_samples[i];
            j += 1;
        }

        for i in 0..NUM_BLOCK_SAMPLES {
            let byte = block.sample_blocks[i / 2];

            let nibble = if i & 1 == 0 {
                byte & 0xf
            } else {
                byte >> 4
            };

            let mut sample = (((nibble as i16) << 12) as i32) >> block.shift as i32;

            sample += ((self.last_decoded_samples[0] * positive_filter as i16) >> 6) as i32;
            sample += ((self.last_decoded_samples[1] * negative_filter as i16) >> 6) as i32;

            self.last_decoded_samples[1] = self.last_decoded_samples[0];
            self.last_decoded_samples[0] = SPU::clamp(sample, -0x8000, 0x7fff);

            self.current_samples[i] = self.last_decoded_samples[0];
        }


        self.current_block = *block;
    }

    fn read_adpcm_block(&mut self, sound_ram: &SoundRam) -> ADPCMBlock {
        let mut block = ADPCMBlock::new();

        let shift_filter = sound_ram.read8(self.current_address as usize);

        block.shift = shift_filter & 0xf;
        block.filter = (shift_filter >> 4) & 0xf;

        self.current_address = (self.current_address + 1) & 0x7_ffff;

        let flags = sound_ram.read8(self.current_address as usize);

        block.loop_end = flags & 1 == 1;
        block.loop_repeat = (flags >> 1) & 1 == 1;
        block.loop_start = (flags >> 2) & 1 == 1;

        self.current_address = (self.current_address + 1) & 0x7_ffff;

        for i in 0..14 {
            block.sample_blocks[i] = sound_ram.read8(self.current_address as usize);

            self.current_address = (self.current_address + 1) & 0x7_ffff;
        }

        block
    }

    pub fn update_keyon(&mut self) {
        self.current_address = self.start_address;

        self.adsr.phase = AdsrPhase::Attack;
        self.adsr.envelope.volume = 0;
        self.is_first_block = true;
        self.has_samples = false;

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
            0x0 => (self.left_envelope.volume / 2) as u16,
            0x2 => (self.right_envelope.volume / 2) as u16,
            0x4 => self.sample_rate,
            0x6 => (self.start_address / 8) as u16,
            0x8 => self.adsr.value as u16,
            0xa => (self.adsr.value >> 16) as u16,
            0xc => self.adsr.envelope.volume as u16,
            0xe => (self.repeat_address / 8) as u16,
            _ => panic!("invalid channel given: 0x{:x}", channel)
        }
    }
}