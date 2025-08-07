use std::collections::VecDeque;

use super::{registers::interrupt_register::InterruptRegister, scheduler::{EventType, Scheduler}};

const CYCLES_PER_SCANLINE: usize = 3413;
const VBLANK_LINE_START: usize = 240;
const NUM_SCANLINES: usize = 262;

#[derive(Copy, Clone, Debug)]
enum TexturePageColors {
    Bit4 = 0,
    Bit8 = 1,
    Bit15 = 2
}


#[derive(Debug)]
struct Texpage {
    x_base: u32,
    y_base1: u32,
    semi_transparency: u32,
    texture_page_colors: TexturePageColors,
    dither: bool,
    draw_to_display_area: bool,
    y_base2: u32,
    x_flip: bool,
    y_flip: bool
}

impl Texpage {
    pub fn new() -> Self {
        Self {
            x_base: 0,
            y_base1: 0,
            semi_transparency: 0,
            texture_page_colors: TexturePageColors::Bit4,
            dither: false,
            draw_to_display_area: false,
            y_base2: 0,
            x_flip: false,
            y_flip: false
        }
    }
}

pub struct GPU {
    pub frame_finished: bool,
    pub current_line: usize,
    even_flag: u32,
    interlaced: bool,
    pub command_fifo: VecDeque<u32>,
    texpage: Texpage
}

impl GPU {
    pub fn new(scheduler: &mut Scheduler) -> Self {
        scheduler.schedule(EventType::Hblank, (CYCLES_PER_SCANLINE as f32 * (7.0 / 11.0)) as usize);

        Self {
            frame_finished: false,
            current_line: 0,
            even_flag: 0,
            interlaced: false,
            command_fifo: VecDeque::with_capacity(16),
            texpage: Texpage::new()
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
            match command {
                0xe1 => self.texpage(word),
                _ => todo!("command: 0x{:x}", command)
            }
        }
    }

    /*
        0-3   Texture page X Base   (N*64) (ie. in 64-halfword steps)    ;GPUSTAT.0-3
        4     Texture page Y Base 1 (N*256) (ie. 0, 256, 512 or 768)     ;GPUSTAT.4
        5-6   Semi-transparency     (0=B/2+F/2, 1=B+F, 2=B-F, 3=B+F/4)   ;GPUSTAT.5-6
        7-8   Texture page colors   (0=4bit, 1=8bit, 2=15bit, 3=Reserved);GPUSTAT.7-8
        9     Dither 24bit to 15bit (0=Off/strip LSBs, 1=Dither Enabled) ;GPUSTAT.9
        10    Drawing to display area (0=Prohibited, 1=Allowed)          ;GPUSTAT.10
        11    Texture page Y Base 2 (N*512) (only for 2 MB VRAM)         ;GPUSTAT.15
        12    Textured Rectangle X-Flip   (BIOS does set this bit on power-up...?)
        13    Textured Rectangle Y-Flip   (BIOS does set it equal to GPUSTAT.13...?)
        14-23 Not used (should be 0)
        24-31 Command  (E1h)
    */
    fn texpage(&mut self, word: u32) {
        self.texpage.x_base = word & 0xf;
        self.texpage.y_base1 = (word >> 4) & 0x1;
        self.texpage.semi_transparency = (word >> 5) & 0x3;
        self.texpage.texture_page_colors = match (word >> 7) & 0x3 {
            0 => TexturePageColors::Bit4,
            1 => TexturePageColors::Bit8,
            2 => TexturePageColors::Bit15,
            _ => panic!("reserved value for texpage colors")
        };
        self.texpage.dither = (word >> 9) & 0x1 == 1;
        self.texpage.draw_to_display_area = (word >> 10) & 0x1 == 1;
        self.texpage.y_base2 = (word >> 11) & 0x1;
        self.texpage.x_flip = (word >> 12) & 0x1 == 1;
        self.texpage.y_flip = (word >> 13) & 0x1 == 1;
    }

    pub fn read_stat(&self) -> u32 {
        self.even_flag << 31 |
            self.texpage.x_base |
            self.texpage.y_base1 << 4 |
            self.texpage.semi_transparency << 5 |
            (self.texpage.texture_page_colors as u32) << 7 |
            (self.texpage.dither as u32) << 9 |
            (self.texpage.draw_to_display_area as u32) << 10 |
            self.texpage.y_base2 << 15 |
            0x7 << 26
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