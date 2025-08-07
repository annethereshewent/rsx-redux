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
    pub current_command_buffer: VecDeque<u32>,
    texpage: Texpage,
    pub gpuread: u32,
    words_left: usize,
    is_polyline: bool,
    x1: u32,
    x2: u32,
    y1: u32,
    y2: u32
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
            current_command_buffer: VecDeque::new(),
            texpage: Texpage::new(),
            gpuread: 0,
            words_left: 0,
            is_polyline: false,
            x1: 0,
            x2: 0,
            y1: 0,
            y2: 0
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

    fn get_words_left(word: u32) -> usize {
        let upper_bits = word >> 29;

        if upper_bits == 0x1 {
            // render polygon command
            let is_textured = (word >> 26) & 1 == 1;
            let is_shaded = (word >> 28) & 1 == 1;

            let num_vertices = if (word >> 27) == 1 { 4 } else { 3 };

            let mut multiplier = 1;

            if is_shaded {
                multiplier += 1;
            }
            if is_textured {
                multiplier += 1;
            }

            return num_vertices * multiplier
        }

        1
    }

    fn render_polygon(&mut self) {

    }

    fn render_rectangle(&mut self) {

    }

    fn set_drawing_area(&mut self, word: u32, is_bottom_right: bool) {
        if is_bottom_right {
            self.x2 = word & 0x3ff;
            self.y2 = (word >> 10) & 0x3ff;
        } else {
            self.x1 = word & 0x3ff;
            self.y1 = (word >> 10) & 0x3ff;
        }
    }

    fn execute_command(&mut self, word: u32) {
        let command = word >> 24;
        let upper = word >> 29;

        match upper {
            1 => self.render_polygon(),
            2 => unreachable!("shouldn't happen"),
            3 => self.render_rectangle(),
            _ => {
                match command {
                    0x0 => (),
                    0xe1 => self.texpage(word),
                    0xe3 => self.set_drawing_area(word, false),
                    0xe4 => self.set_drawing_area(word, true),
                    _ => todo!("command: 0x{:x}", command)
                }
            }
        }


    }

    fn draw_line(&mut self) {
        todo!("draw line");
    }

    fn draw_polyline(&mut self) {
        todo!("draw polyline");
    }

    fn process_commands(&mut self) {
        let len = self.command_fifo.len();
        if len > 0 {
            println!("command fifo length: {len}");
        }
        while !self.command_fifo.is_empty() {
            let word = self.command_fifo.pop_front().unwrap();
            let upper = word >> 29;

            self.current_command_buffer.push_back(word);

            if self.words_left == 0 {

                println!("upper = 0x{:x}", upper);
                println!("command = 0x{:x}", word >> 24);
                if upper == 0x2 {
                    if (word >> 27) & 1 == 1 {
                        self.is_polyline = true;
                    }
                } else {
                    self.words_left = Self::get_words_left(word);
                }
            }

            if self.words_left == 1 {
                let word = self.current_command_buffer[0];
                self.execute_command(word);

                self.current_command_buffer = VecDeque::new();
            }

            if self.words_left == 0 && upper == 0x2 || self.is_polyline {
                if self.is_polyline {
                    if (word & 0xf000f000) == 0x50005000 {
                        self.is_polyline = false;

                        self.draw_polyline();

                        self.current_command_buffer = VecDeque::new();
                    }
                } else {
                    self.draw_line();
                    self.current_command_buffer = VecDeque::new();
                }
            }

            if self.words_left > 1 {
                self.words_left -= 1;
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