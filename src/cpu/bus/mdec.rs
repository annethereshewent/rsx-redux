use std::collections::VecDeque;

pub struct Mdec {
    in_fifo: VecDeque<u16>,
    out_fifo: VecDeque<u8>,
    dma_in_enable: bool,
    dma_out_enable: bool,
    words_remaining: u32,
    command: Option<u32>
}

impl Mdec {
    pub fn new() -> Self {
        Self {
            in_fifo: VecDeque::new(),
            out_fifo: VecDeque::new(),
            dma_in_enable: false,
            dma_out_enable: false,
            words_remaining: 0,
            command: None
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

    fn write_command(&mut self, value: u32) {
        if let Some(_) = self.command {

            self.in_fifo.push_back(value as u16);
            self.in_fifo.push_back((value >> 16) as u16);
        } else {
            self.command = Some(value);

            match (value >> 29) & 0x7 {
                1 => self.decode_macroblocks(),
                2 => self.set_quant_table(),
                3 => self.set_scale_table(),
                _ => ()
            }
        }
    }

    fn decode_macroblocks(&mut self) {
        todo!("decode macroblocks");
    }

    fn set_quant_table(&mut self) {
        todo!("set quant table");
    }

    fn set_scale_table(&mut self) {
        todo!("set scale table");
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