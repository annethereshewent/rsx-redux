use std::collections::VecDeque;

use super::{registers::interrupt_register::InterruptRegister, scheduler::{EventType, Scheduler}};

const CYCLES_PER_SCANLINE: usize = 3413;
const VBLANK_LINE_START: usize = 240;
const NUM_SCANLINES: usize = 262;

pub struct GPU {
    pub frame_finished: bool,
    pub current_line: usize,
    even_flag: u32,
    interlaced: bool,
    pub command_fifo: VecDeque<u32>
}

impl GPU {
    pub fn new(scheduler: &mut Scheduler) -> Self {
        scheduler.schedule(EventType::Hblank, (CYCLES_PER_SCANLINE as f32 * (7.0 / 11.0)) as usize);

        Self {
            frame_finished: false,
            current_line: 0,
            even_flag: 0,
            interlaced: false,
            command_fifo: VecDeque::with_capacity(16)
        }
    }

    pub fn handle_hblank(&mut self, interrupt_stat: &mut InterruptRegister, scheduler: &mut Scheduler, cycles_left: usize) {
        self.process_commands();
        if self.current_line < VBLANK_LINE_START {
            scheduler.schedule(EventType::Hblank, (CYCLES_PER_SCANLINE as f32 * (7.0 / 11.0)) as usize - cycles_left);
        } else {
            interrupt_stat.insert(InterruptRegister::VBLANK);

            scheduler.schedule(EventType::Vblank, (CYCLES_PER_SCANLINE as f32 * (7.0 / 11.0)) as usize - cycles_left);
        }

        if self.interlaced {
            self.even_flag = if self.even_flag == 0 { 1 } else { 0 };
        }

        self.current_line += 1;
    }

    fn process_commands(&mut self) {
        while !self.command_fifo.is_empty() {
            let word = self.command_fifo.pop_front().unwrap();
            let command = word >> 24;

            // process the command
        }
    }

    pub fn read_stat(&self) -> u32 {
        self.even_flag << 31
    }

    pub fn handle_vblank(&mut self, scheduler: &mut Scheduler, cycles_left: usize) {
        self.even_flag = 0;

        if self.current_line == NUM_SCANLINES {
            self.frame_finished = true;
            self.current_line = 0;
            scheduler.schedule(EventType::Hblank, (CYCLES_PER_SCANLINE as f32 * (7.0 / 11.0)) as usize - cycles_left);
        } else {
            scheduler.schedule(EventType::Vblank, (CYCLES_PER_SCANLINE as f32 * (7.0 / 11.0)) as usize - cycles_left);
            self.current_line += 1;
        }
    }
}