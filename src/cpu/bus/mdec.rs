use std::{collections::VecDeque, mem};

#[derive(Copy, Clone, PartialEq, Debug)]
enum OutputDepth {
    Bit4 = 0,
    Bit8 = 1,
    Bit24 = 2,
    Bit15 = 3,
}

#[derive(Copy, Clone, PartialEq)]
enum BlockType {
    Cr = 0,
    Cb = 1,
    Yb = 2,
}

const NUM_COLORS: usize = 256;

const ZIGZAG_TABLE: [usize; 64] = [
    0, 1, 5, 6, 14, 15, 27, 28, 2, 4, 7, 13, 16, 26, 29, 42, 3, 8, 12, 17, 25, 30, 41, 43, 9, 11,
    18, 24, 31, 40, 44, 53, 10, 19, 23, 32, 39, 45, 52, 54, 20, 22, 33, 38, 46, 51, 55, 60, 21, 34,
    37, 47, 50, 56, 59, 61, 35, 36, 48, 49, 57, 58, 62, 63,
];

pub struct Mdec {
    pub in_fifo: VecDeque<u32>,
    out_fifo: VecDeque<u8>,
    dma_in_enable: bool,
    dma_out_enable: bool,
    words_remaining: u32,
    command: Option<u32>,
    luminance_quant_table: [u8; 64],
    color_quant_table: [u8; 64],
    scale_table: [i16; 64],
    with_color: bool,
    current_block: usize,
    output_depth: OutputDepth,
    is_signed: bool,
    output_bit15: bool,
    blocks: [[i16; 64]; 3],
    zagzig_table: [usize; 64],
}

impl Default for Mdec {
    fn default() -> Self {
        Self::new()
    }
}

impl Mdec {
    pub fn new() -> Self {
        Self {
            in_fifo: VecDeque::new(),
            out_fifo: VecDeque::new(),
            luminance_quant_table: [0; 64],
            color_quant_table: [0; 64],
            scale_table: [0; 64],
            dma_in_enable: false,
            dma_out_enable: false,
            words_remaining: 0,
            command: None,
            with_color: false,
            current_block: 0,
            output_depth: OutputDepth::Bit8,
            is_signed: false,
            output_bit15: false,
            blocks: [[0; 64]; 3],
            zagzig_table: Self::populate_zagzig_table(),
        }
    }

    fn populate_zagzig_table() -> [usize; 64] {
        let mut table = [0; 64];
        for i in 0..64 {
            table[ZIGZAG_TABLE[i]] = i;
        }
        table
    }

    pub fn read(&self, address: usize) -> u32 {
        match address {
            0x1f801824 => self.read_status(),
            _ => todo!("(read)mdec address: 0x{:x}", address),
        }
    }

    pub fn write(&mut self, address: usize, value: u32) {
        match address {
            0x1f801820 => self.write_command(value),
            0x1f801824 => self.write_control(value),
            _ => todo!("(write)mdec address: 0x{:x}", address),
        }
    }

    pub fn write_command(&mut self, value: u32) {
        if let Some(command) = self.command {
            self.in_fifo.push_back(value);

            self.words_remaining -= 1;

            if self.words_remaining == 0 {
                self.command = None;
                match command {
                    0x1 => self.decode_macroblocks(),
                    0x2 => self.populate_quant_table(),
                    0x3 => self.populate_scale_table(),
                    _ => todo!("mdec command 0x{:x}", command),
                }
            }
        } else {
            self.command = Some((value >> 29) & 0x7);

            match (value >> 29) & 0x7 {
                0x1 => self.set_decode_macroblocks_params(value),
                0x2 => self.set_quant_table_word_size(value),
                0x3 => self.set_scale_table_word_size(value),
                _ => todo!("mdec command 0x{:x}", self.command.unwrap()),
            }
        }
    }

    fn decode_macroblocks(&mut self) {
        let mut output = [0; 768];
        while !self.in_fifo.is_empty() {
            if self.current_block > 1 {
                let index = self.current_block - 2;

                let lower_bit = index & 1;
                let upper_bit = (index >> 1) & 1;

                let xx = lower_bit * 8;
                let yy = upper_bit * 8;

                self.decode_block(BlockType::Yb);
                self.yuv_to_rgb(&mut output, xx, yy);

                if self.current_block >= 5 {
                    let multiplier = match self.output_depth {
                        OutputDepth::Bit24 => 3,
                        OutputDepth::Bit15 => 2,
                        _ => todo!("output depth: {:?}", self.output_depth),
                    };

                    let num_bytes = multiplier * NUM_COLORS;

                    for i in 0..num_bytes {
                        self.out_fifo.push_back(output[i]);
                    }
                }
            } else {
                if self.current_block == 0 {
                    self.decode_block(BlockType::Cr);
                } else {
                    self.decode_block(BlockType::Cb);
                }
            }
        }

        self.current_block = 0;
    }

    fn yuv_to_rgb(&mut self, output: &mut [u8], xx: usize, yy: usize) {
        for y in 0..8 {
            for x in 0..8 {
                let index = ((x + xx) / 2) + ((y + yy) / 2) * 8;
                let mut r = self.blocks[BlockType::Cr as usize][index];
                let mut b = self.blocks[BlockType::Cb as usize][index];
                let mut g = ((-0.3437 * b as f32) + (-0.7143 * r as f32)) as i16;
                r = (1.402 * r as f32) as i16;
                b = (1.772 * b as f32) as i16;

                let y_l = self.blocks[BlockType::Yb as usize][x + y * 8];

                r = (r + y_l).clamp(-128, 127);
                g = (g + y_l).clamp(-128, 127);
                b = (b + y_l).clamp(-128, 127);

                if !self.is_signed {
                    r ^= 0x80;
                    g ^= 0x80;
                    b ^= 0x80;
                }

                match self.output_depth {
                    OutputDepth::Bit15 => {
                        let offset = ((x + xx) + (y + yy) * 16) * 2;

                        let r15 = (r as u8) >> 3;
                        let g15 = (g as u8) >> 3;
                        let b15 = (b as u8) >> 3;

                        let mut output15 = r15 as u16 | (g15 as u16) << 5 | (b15 as u16) << 10;

                        if self.output_bit15 {
                            output15 |= 1 << 15;
                        }

                        output[offset] = output15 as u8;
                        output[offset + 1] = (output15 >> 8) as u8;
                    }
                    OutputDepth::Bit24 => {
                        let offset = ((x + xx) + (y + yy) * 16) * 3;

                        output[offset] = r as u8;
                        output[offset + 1] = g as u8;
                        output[offset + 2] = b as u8;
                    }
                    _ => todo!("output depth: {:?}", self.output_depth),
                }
            }
        }
    }

    fn decode_block(&mut self, block_type: BlockType) {
        let mut is_upper_half = false;
        let mut word = self.in_fifo.pop_front().unwrap();

        let block = &mut self.blocks[block_type as usize];

        for i in 0..64 {
            block[i] = 0;
        }

        let mut halfword = word as u16;

        while halfword == 0xfe00 {
            if self.in_fifo.is_empty() {
                return;
            }
            is_upper_half = !is_upper_half;

            halfword = if is_upper_half {
                word >> 16
            } else {
                self.in_fifo.pop_front().unwrap()
            } as u16;
        }

        let mut k = 0;

        let quant_table = if [BlockType::Cr, BlockType::Cb].contains(&block_type) {
            &self.color_quant_table
        } else {
            &self.luminance_quant_table
        };

        let q_scale = ((halfword >> 10) & 0x3f) as i8 as i16;
        let mut val = Self::sign_extend_i10(halfword & 0x3ff) * quant_table[k] as i8 as i16;

        while k < 64 {
            if q_scale == 0 {
                val = (Self::sign_extend_i10(halfword & 0x3ff) * 2) * 2;
            }
            val = val.clamp(-0x400, 0x3ff);

            if q_scale > 0 {
                block[self.zagzig_table[k]] = val;
            } else {
                block[k] = val;
            }

            is_upper_half = !is_upper_half;

            halfword = if is_upper_half {
                (word >> 16) as u16
            } else {
                word = if let Some(next) = self.in_fifo.pop_front() {
                    next
                } else {
                    return;
                };

                word as u16
            };

            k += (((halfword as usize) >> 10) & 0x3f) + 1;

            if k < 64 {
                val = (Self::sign_extend_i10(halfword & 0x3ff)
                    * quant_table[k] as i8 as i16
                    * q_scale
                    + 4)
                    / 8;
            }
        }

        self.idct_core(block_type);

        self.current_block += 1;

        if self.current_block == 6 {
            self.current_block = 0;
        }
    }

    fn idct_core(&mut self, block_type: BlockType) {
        let block = &mut self.blocks[block_type as usize];

        let mut dest = [0; 64];

        for _ in 0..2 {
            for x in 0..8 {
                for y in 0..8 {
                    let mut sum = 0;

                    for z in 0..8 {
                        sum += block[y + z * 8] * (self.scale_table[x + z * 8] / 8);
                    }
                    dest[x + y * 8] = (sum + 0xfff) / 0x2000;
                }
            }
            mem::swap(block, &mut dest);
        }
    }

    fn sign_extend_i10(value: u16) -> i16 {
        ((value as i16) << 6) >> 6
    }

    fn set_decode_macroblocks_params(&mut self, value: u32) {
        self.output_depth = match (value >> 28) & 0x3 {
            0 => OutputDepth::Bit4,
            1 => OutputDepth::Bit8,
            2 => OutputDepth::Bit24,
            3 => OutputDepth::Bit15,
            _ => unreachable!(),
        };

        self.is_signed = (value >> 26) & 0x1 == 1;
        self.output_bit15 = (value >> 25) & 0x1 == 1;

        self.words_remaining = value & 0xffff;
    }

    fn populate_scale_table(&mut self) {
        for i in (0..64).step_by(2) {
            let word = self.in_fifo.pop_front().unwrap();

            self.scale_table[i] = word as i16;
            self.scale_table[i + 1] = (word >> 16) as i16;
        }
    }

    fn populate_quant_table(&mut self) {
        for i in (0..64).step_by(4) {
            let word = self.in_fifo.pop_front().unwrap();

            self.luminance_quant_table[i] = word as u8;
            self.luminance_quant_table[i + 1] = (word >> 8) as u8;
            self.luminance_quant_table[i + 2] = (word >> 16) as u8;
            self.luminance_quant_table[i + 3] = (word >> 24) as u8;
        }

        if self.with_color {
            for i in (0..64).step_by(4) {
                let word = self.in_fifo.pop_front().unwrap();

                self.color_quant_table[i] = word as u8;
                self.color_quant_table[i + 1] = (word >> 8) as u8;
                self.color_quant_table[i + 2] = (word >> 16) as u8;
                self.color_quant_table[i + 3] = (word >> 24) as u8;
            }
        }
    }

    fn set_quant_table_word_size(&mut self, value: u32) {
        // The command word is followed by 64 unsigned parameter bytes for the Luminance Quant Table
        // (used for Y1..Y4), and if Command.Bit0 was set, by another 64 unsigned parameter bytes
        // for the Color Quant Table (used for Cb and Cr).
        (self.words_remaining, self.with_color) = if value & 1 == 0 {
            (16, false)
        } else {
            (32, true)
        }
    }

    fn set_scale_table_word_size(&mut self, _value: u32) {
        // The command is followed by 64 signed halfwords with 14bit fractional part,
        // the values should be usually/always the same values (based on the standard JPEG constants,
        // although, MDEC(3) allows to use other values than that constants).

        self.words_remaining = 32;
    }

    fn read_status(&self) -> u32 {
        (self.out_fifo.is_empty() as u32) << 31
            | (self.command.is_some() as u32) << 29
            | (self.dma_in_enable as u32) << 28
            | (self.dma_out_enable as u32) << 27
            | (self.output_depth as u32) << 25
            | (self.is_signed as u32) << 24
            | (self.output_bit15 as u32) << 23
            | ((self.current_block as u32 + 4) % 6) << 16
            | (self.words_remaining - 1)
    }

    fn write_control(&mut self, value: u32) {
        if (value >> 31) & 1 == 1 {
            self.out_fifo.clear();
            self.current_block = 0;
            self.words_remaining = 0;
            self.command = None;

            self.dma_in_enable = false;
            self.dma_out_enable = false;

            return;
        }

        self.dma_in_enable = (value >> 30) & 1 == 1;
        self.dma_out_enable = (value >> 29) & 1 == 1;
    }
}
