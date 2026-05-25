use std::{array::from_fn, collections::VecDeque, mem};

use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
enum OutputDepth {
    Bit4 = 0,
    Bit8 = 1,
    Bit24 = 2,
    Bit15 = 3,
}

#[derive(Copy, Clone, PartialEq, Serialize, Deserialize)]
enum BlockType {
    Cr = 0,
    Cb = 1,
    Yb = 2,
}

#[derive(Debug)]
pub struct MdecDma {
    pub dma_in: bool,
    pub dma_out: bool
}

#[derive(Serialize, Deserialize, PartialEq)]
enum BlockStatus {
    BlocksPending,
    BlocksProcessed
}

const NUM_COLORS: usize = 256;
const MDEC_FIFO_SIZE_HALFWORDS: usize = 64;

const ZIGZAG_TABLE: [usize; 64] = [
    0, 1, 5, 6, 14, 15, 27, 28, 2, 4, 7, 13, 16, 26, 29, 42, 3, 8, 12, 17, 25, 30, 41, 43, 9, 11,
    18, 24, 31, 40, 44, 53, 10, 19, 23, 32, 39, 45, 52, 54, 20, 22, 33, 38, 46, 51, 55, 60, 21, 34,
    37, 47, 50, 56, 59, 61, 35, 36, 48, 49, 57, 58, 62, 63,
];

#[derive(Serialize, Deserialize)]
pub struct Mdec {
    in_fifo: VecDeque<u16>,
    pub out_fifo: VecDeque<u8>,
    dma_in_enable: bool,
    dma_out_enable: bool,
    words_remaining: u16,
    command: Option<u32>,
    luminance_quant_table: Box<[u8]>,
    color_quant_table: Box<[u8]>,
    scale_table: Box<[i16]>,
    with_color: bool,
    current_block: usize,
    output_depth: OutputDepth,
    is_signed: bool,
    output_bit15: bool,
    blocks: [Box<[i16]>; 3],
    zagzig_table: Box<[usize]>,
    k: usize,
    q_scale: u16,
    output: Box<[u8]>,
    block_status: BlockStatus,
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
            luminance_quant_table: vec![0; 64].into_boxed_slice(),
            color_quant_table: vec![0; 64].into_boxed_slice(),
            scale_table: vec![0; 64].into_boxed_slice(),
            dma_in_enable: false,
            dma_out_enable: false,
            words_remaining: 0,
            command: None,
            with_color: false,
            current_block: 0,
            output_depth: OutputDepth::Bit4,
            is_signed: false,
            output_bit15: false,
            blocks: from_fn(|_| vec![0; 64].into_boxed_slice()),
            zagzig_table: Self::populate_zagzig_table(),
            k: 64,
            q_scale: 0,
            output: vec![0; 768].into_boxed_slice(),
            block_status: BlockStatus::BlocksPending,
        }
    }

    fn populate_zagzig_table() -> Box<[usize]> {
        let mut table = vec![0; 64].into_boxed_slice();
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

    pub fn write(&mut self, address: usize, value: u32) -> MdecDma {
        match address {
            0x1f801820 => { self.write_command(value); },
            0x1f801824 => self.write_control(value),
            _ => todo!("(write)mdec address: 0x{:x}", address),
        }

        self.update_status()
    }

    fn write_command(&mut self, value: u32) {
        self.in_fifo.push_back(value as u16);
        self.in_fifo.push_back((value >> 16) as u16);

        self.words_remaining -= 1;

        self.execute();
    }

    pub fn dma_write(&mut self, value: u32) {
        self.in_fifo.push_back(value as u16);
        self.in_fifo.push_back((value >> 16) as u16);

        self.words_remaining -= 1;
    }

    pub fn execute(&mut self) -> MdecDma {
        while !self.in_fifo.is_empty() {
            if let Some(command) = self.command {
                match command {
                    0x1 => {
                        if self.decode_macroblocks() {
                            if self.words_remaining == 0 && self.in_fifo.is_empty() {
                                self.command = None;

                            }
                        } else if self.words_remaining == 0 && self.block_status != BlockStatus::BlocksProcessed {
                            self.command = None;
                            self.current_block = 0;
                            self.q_scale = 0;
                            self.k = 64;
                        }

                        break;
                    }
                    0x2 => if self.words_remaining == 0 {
                        self.populate_quant_table();
                        self.command = None;
                    } else {
                        println!("[WARN]: attempting to populate quant table but words remaining is non-zero");
                        break;
                    }
                    0x3 => if self.words_remaining == 0 {
                        self.populate_scale_table();
                        self.command = None;
                    } else {
                        println!("[WARN]: attempting to populate scale table but words remaining is non-zero");
                        break;
                    }
                    _ => panic!("invalid mdec command 0x{command:x}")
                }

            } else {
                let word = self.in_fifo.pop_front().unwrap() as u32 | (self.in_fifo.pop_front().unwrap() as u32) << 16;
                self.command = Some((word >> 29) & 0x7);

                match self.command.unwrap() {
                    0x1 => self.set_decode_macroblocks_params(word),
                    0x2 => self.set_quant_table_word_size(word),
                    0x3 => self.set_scale_table_word_size(word),
                    _ => panic!("invalid mdec command 0x{:x}", self.command.unwrap()),
                }
            }
        }

        self.update_status()
    }

    pub fn update_status(&self) -> MdecDma {
        let in_full = self.in_fifo.len() >= MDEC_FIFO_SIZE_HALFWORDS;
        let out_empty = self.out_fifo.len() < 4;
        let data_in_request = self.dma_in_enable && !in_full;
        let data_out_request = self.dma_out_enable && !out_empty;

        MdecDma {
            dma_in: data_in_request,
            dma_out: data_out_request
        }
    }

    pub fn read_out_fifo(&mut self) -> u32 {
        let mut value = 0;

        for i in 0..4 {
            value |= (self.out_fifo.pop_front().unwrap() as u32) << (i * 8)
        }

        value
    }

    fn decode_macroblocks(&mut self) -> bool {
        while !self.in_fifo.is_empty() {
            if self.current_block > 1 {
                let index = self.current_block - 2;

                let lower_bit = index & 1;
                let upper_bit = (index >> 1) & 1;

                let xx = lower_bit * 8;
                let yy = upper_bit * 8;

                let is_final_block = self.current_block == 5;

                if !self.decode_block(BlockType::Yb) {
                    return false;
                }
                self.yuv_to_rgb( xx, yy);

                if is_final_block {
                    let multiplier = match self.output_depth {
                        OutputDepth::Bit24 => 3,
                        OutputDepth::Bit15 => 2,
                        _ => todo!("output depth: {:?}", self.output_depth),
                    };

                    let num_bytes = multiplier * NUM_COLORS;

                    for byte in self.output.iter().take(num_bytes) {
                        self.out_fifo.push_back(*byte);
                    }

                    return true;
                }
            } else if self.current_block == 0 {
                if !self.decode_block(BlockType::Cr) {
                    return false;
                }
            } else {
                if !self.decode_block(BlockType::Cb) {
                    return false;
                }
            }
        }

        true
    }

    fn yuv_to_rgb(&mut self, xx: usize, yy: usize) {
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

                        self.output[offset] = output15 as u8;
                        self.output[offset + 1] = (output15 >> 8) as u8;
                    }
                    OutputDepth::Bit24 => {
                        let offset = ((x + xx) + (y + yy) * 16) * 3;

                        self.output[offset] = r as u8;
                        self.output[offset + 1] = g as u8;
                        self.output[offset + 2] = b as u8;
                    }
                    _ => todo!("output depth: {:?}", self.output_depth),
                }
            }
        }
    }

    fn decode_block(&mut self, block_type: BlockType) -> bool {
        self.block_status = BlockStatus::BlocksPending;
        let block = &mut self.blocks[block_type as usize];

        let quant_table = if block_type == BlockType::Yb {
            &self.luminance_quant_table
        } else {
            &self.color_quant_table
        };

        if self.k == 64 {
            for element in block.iter_mut().take(64) {
                *element = 0;
            }
            let mut halfword = self.in_fifo.pop_front().unwrap();
            while halfword == 0xfe00 {
                if self.in_fifo.is_empty() {
                    return false;
                }

                halfword = self.in_fifo.pop_front().unwrap();

            }

            self.k = 0;
            self.q_scale = (halfword >> 10) & 0x3f;

            if self.q_scale == 0 {
                let val = (Self::sign_extend_i10(halfword & 0x3ff) * 2).clamp(-0x400, 0x3ff);
                block[self.k] = val;
            }  else {
                let val = (Self::sign_extend_i10(halfword & 0x3ff) * quant_table[self.k] as i16).clamp(-0x400, 0x3ff);
                block[self.zagzig_table[self.k]] = val;
            }
        }

        loop {
            let halfword = if let Some(next) = self.in_fifo.pop_front() {
                next
            } else {
                return false;
            };

            self.k += (((halfword as usize) >> 10) & 0x3f) + 1;

            if self.k < 64 {
                let mut val = if self.q_scale == 0 {
                    Self::sign_extend_i10(halfword & 0x3ff) * 2
                } else {(Self::sign_extend_i10(halfword & 0x3ff)
                    * quant_table[self.k] as i16
                    * self.q_scale as i16
                    + 4)
                    / 8
                };
                val = val.clamp(-0x400, 0x3ff);

                if self.q_scale > 0 {
                    block[self.zagzig_table[self.k]] = val;
                } else {
                    block[self.k] = val;
                }
            }

            if self.k >= 63 {
                break;
            }
        }

        self.k = 64;

        self.idct_core(block_type);

        self.current_block += 1;

        if self.current_block == 6 {
            self.block_status = BlockStatus::BlocksProcessed;
            self.current_block = 0;
        }

        true
    }

    fn idct_core(&mut self, block_type: BlockType) {
        let block = &mut self.blocks[block_type as usize];

        let mut dest = vec![0; 64].into_boxed_slice();

        for _ in 0..2 {
            for x in 0..8 {
                for y in 0..8 {
                    let mut sum = 0;

                    for z in 0..8 {
                        sum += block[y + z * 8] as i32 * (self.scale_table[x + z * 8] as i32 / 8);
                    }
                    dest[x + y * 8] = ((sum + 0xfff) / 0x2000) as i16;
                }
            }
            mem::swap(block, &mut dest);
        }
    }

    fn sign_extend_i10(value: u16) -> i16 {
        ((value << 6) as i16) >> 6
    }

    fn set_decode_macroblocks_params(&mut self, value: u32) {
        self.output_depth = match (value >> 27) & 0x3 {
            0 => OutputDepth::Bit4,
            1 => OutputDepth::Bit8,
            2 => OutputDepth::Bit24,
            3 => OutputDepth::Bit15,
            _ => unreachable!(),
        };

        self.is_signed = (value >> 26) & 0x1 == 1;
        self.output_bit15 = (value >> 25) & 0x1 == 1;

        self.words_remaining = value as u16;
    }

    fn populate_scale_table(&mut self) {
        for i in 0..64 {
            let halfword = self.in_fifo.pop_front().unwrap();

            self.scale_table[i] = halfword as i16;
        }
    }

    fn populate_quant_table(&mut self) {
        for i in 0..32 {
            let halfword = self.in_fifo.pop_front().unwrap();

            let index = i * 2;

            self.luminance_quant_table[index] = halfword as u8;
            self.luminance_quant_table[index + 1] = (halfword >> 8) as u8;
        }

        if self.with_color {
            for i in 0..32 {
                let halfword = self.in_fifo.pop_front().unwrap();

                let index = i * 2;

                self.color_quant_table[index] = halfword as u8;
                self.color_quant_table[index + 1] = (halfword >> 8) as u8;
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
        let in_full = self.in_fifo.len() >= MDEC_FIFO_SIZE_HALFWORDS;
        let out_empty = self.out_fifo.len() < 4;
        let data_in_request = self.dma_in_enable && !in_full;
        let data_out_request = self.dma_out_enable && !out_empty;

        (out_empty as u32) << 31
            | (in_full as u32) << 30
            | (self.command.is_some() as u32) << 29
            | (data_in_request as u32) << 28
            | (data_out_request as u32) << 27
            | (self.output_depth as u32) << 25
            | (self.is_signed as u32) << 24
            | (self.output_bit15 as u32) << 23
            | ((self.current_block as u32 + 4) % 6) << 16
            | (self.words_remaining - 1) as u32
    }

    fn write_control(&mut self, value: u32) {
        if (value >> 31) & 1 == 1 {
            self.out_fifo.clear();
            self.in_fifo.clear();
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
