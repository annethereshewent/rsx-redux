use std::collections::VecDeque;

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
    with_color: bool
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
            with_color: false
        }
    }
    pub fn read(&self, address: usize) -> u32 {
        match address {
            0x1f801824 => self.read_status(),
            _ => todo!("(read)mdec address: 0x{:x}", address)
        }
    }

    pub fn write(&mut self, address: usize, value: u32) {
        match address {
            0x1f801820 => self.write_command(value),
            0x1f801824 => self.write_control(value),
            _ => todo!("(write)mdec address: 0x{:x}", address)
        }
    }

    pub fn write_command(&mut self, value: u32) {
        if let Some(command) = self.command {

            self.in_fifo.push_back(value);

            self.words_remaining -= 1;

            if self.words_remaining == 0 {
                self.command = None;
                match command {
                    0x2 => self.populate_quant_table(),
                    0x3 => self.populate_scale_table(),
                    _ => todo!("mdec command 0x{:x}", command)
                }
            }
        } else {
            self.command = Some((value >> 29) & 0x7);

            match (value >> 29) & 0x7 {
                0x1 => self.decode_macroblocks(),
                0x2 => self.set_quant_table(value),
                0x3 => self.set_scale_table(),
                _ => ()
            }
        }
    }

    fn decode_macroblocks(&mut self) {
        todo!("decode macroblocks");
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

    fn set_quant_table(&mut self, value: u32) {
        // The command word is followed by 64 unsigned parameter bytes for the Luminance Quant Table
        // (used for Y1..Y4), and if Command.Bit0 was set, by another 64 unsigned parameter bytes
        // for the Color Quant Table (used for Cb and Cr).
        (self.words_remaining, self.with_color) = if value & 1 == 0 {

            (16, false)
        } else {
            (32, true)
        }
    }

    fn set_scale_table(&mut self) {
        // The command is followed by 64 signed halfwords with 14bit fractional part,
        // the values should be usually/always the same values (based on the standard JPEG constants,
        // although, MDEC(3) allows to use other values than that constants).

        self.words_remaining = 32;
    }

    fn read_status(&self) -> u32 {
        (self.out_fifo.is_empty() as u32) << 31 |
            (self.dma_in_enable as u32) << 28 |
            (self.dma_out_enable as u32) << 27 |
            self.words_remaining
    }

    fn write_control(&mut self, value: u32) {
        if (value >> 31) & 1 == 1 {
            self.dma_in_enable = false;
            self.dma_out_enable = false;
            self.out_fifo.clear();

            // TODO: add current block
        } else {
            self.dma_in_enable = (value >> 30) & 1 == 1;
            self.dma_out_enable = (value >> 29) & 1 == 1;
        }
    }
}