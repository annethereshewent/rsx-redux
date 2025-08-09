use std::{collections::VecDeque, thread::sleep, time::{Duration, SystemTime, UNIX_EPOCH}};

use super::{registers::interrupt_register::InterruptRegister, scheduler::{EventType, Scheduler}, timer::{counter_mode_register::CounterModeRegister, ClockSource, Timer}};

const HBLANK_START: usize = 2813;
const CYCLES_PER_SCANLINE: usize = 3413;
const HBLANK_END: usize = CYCLES_PER_SCANLINE - HBLANK_START;
const VBLANK_LINE_START: usize = 240;
const NUM_SCANLINES: usize = 262;
pub const FPS_INTERVAL: u128 = 1000 / 60;

#[derive(Copy, Clone, PartialEq)]
enum TransferType {
    FromVram,
    ToVram
}

#[derive(Copy, Clone, Debug)]
enum TexturePageColors {
    Bit4 = 0,
    Bit8 = 1,
    Bit15 = 2
}

#[derive(Debug)]
pub struct Polygon {
    pub vertices: Vec<Vertex>,
    pub is_line: bool
}

impl Polygon {
    pub fn new(vertices: Vec<Vertex>, is_line: bool) -> Self {
        Self {
            vertices,
            is_line
        }
    }
}

#[derive(Debug)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: bool
}

#[derive(Debug)]
pub struct Vertex {
    pub x: i32,
    pub y: i32,
    pub u: Option<u32>,
    pub v: Option<u32>,
    pub color: Color,
    pub texpage: Option<Texpage>
}


#[derive(Debug, Copy, Clone)]
pub struct Texpage {
    x_base: u32,
    y_base1: u32,
    semi_transparency: u32,
    texture_page_colors: TexturePageColors,
    dither: bool,
    draw_to_display_area: bool,
    y_base2: u32,
    x_flip: bool,
    y_flip: bool,
    pub value: u32
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
            y_flip: false,
            value: 0
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
    y2: u32,
    x_offset: i32,
    y_offset: i32,
    texture_window_mask_x: u32,
    texture_window_offset_x: u32,
    texture_window_mask_y: u32,
    texture_window_offset_y: u32,
    set_while_drawing: bool,
    check_before_drawing: bool,
    pub polygons: Vec<Polygon>,
    pub commands_ready: bool,
    num_vertices: usize,
    is_shaded: bool,
    is_textured: bool,
    clut_index: usize,
    destination_x: u32,
    destination_y: u32,
    transfer_width: u32,
    transfer_height: u32,
    transfer_type: Option<TransferType>,
    read_x: u32,
    read_y: u32,
    vram: Box<[u8]>,
    previous_time: u128
}

impl GPU {
    pub fn new(scheduler: &mut Scheduler) -> Self {
        scheduler.schedule(EventType::HblankStart, (CYCLES_PER_SCANLINE as f32 * (7.0 / 11.0)) as usize);

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
            y2: 0,
            x_offset: 0,
            y_offset: 0,
            texture_window_mask_x: 0,
            texture_window_mask_y: 0,
            texture_window_offset_x: 0,
            texture_window_offset_y: 0,
            set_while_drawing: false,
            check_before_drawing: false,
            commands_ready: false,
            polygons: Vec::new(),
            num_vertices: 0,
            is_shaded: false,
            is_textured: false,
            clut_index: 0,
            transfer_height: 0,
            transfer_width: 0,
            destination_x: 0,
            destination_y: 0,
            transfer_type: None,
            read_x: 0,
            read_y: 0,
            vram: vec![0; 1024 * 512 * 2].into_boxed_slice(),
            previous_time: 0
        }
    }

    pub fn handle_hblank_start(
        &mut self,
        scheduler: &mut Scheduler,
        timers: &mut [Timer],
        cycles_left: usize
    ) {
        timers[0].in_xblank = true;
        timers[1].in_xblank = false;

        scheduler.schedule(EventType::HblankEnd, HBLANK_END - cycles_left);
    }

    pub fn handle_hblank(
        &mut self,
        scheduler: &mut Scheduler,
        interrupt_stat: &mut InterruptRegister,
        timers: &mut [Timer],
        cycles_left: usize
    ) {
        timers[0].in_xblank = false;
        self.process_commands();

        if timers[0].counter_register.contains(CounterModeRegister::SYNC_ENABLE) {
            match timers[0].counter_register.sync_mode() {
                0 => timers[0].is_active = false,
                1 => timers[0].counter = 0,
                2 => {
                    timers[0].is_active = true;
                    if self.current_line == 0 {
                        timers[0].counter =  0;
                    } else {
                        timers[0].tick(1, scheduler, interrupt_stat);
                    }
                }
                3 => if let Some(_) = &mut timers[0].switch_free_run {
                    timers[0].is_active = true;
                } else {
                    timers[0].switch_free_run = Some(true);
                }
                _ => unreachable!()
            }
        }

        if timers[1].clock_source == ClockSource::Hblank {
            timers[1].tick(1, scheduler, interrupt_stat);
        }

        if self.current_line < VBLANK_LINE_START {
            scheduler.schedule(EventType::HblankStart, (HBLANK_START as f32 * (7.0 / 11.0)) as usize - cycles_left);
        } else {
            timers[1].in_xblank = true;
            interrupt_stat.insert(InterruptRegister::VBLANK);

            scheduler.schedule(EventType::Vblank, (CYCLES_PER_SCANLINE as f32 * (7.0 / 11.0)) as usize - cycles_left);
        }

        if self.interlaced {
            self.even_flag = if self.even_flag == 0 { 1 } else { 0 };
        }

        self.current_line += 1;
    }

    fn get_words_left(&mut self, word: u32) -> usize {
        let upper_bits = word >> 29;

        if upper_bits == 0x1 {
            // render polygon command
            let is_textured = (word >> 26) & 1 == 1;
            let is_shaded = (word >> 28) & 1 == 1;

            let num_vertices = if (word >> 27) & 1 == 1 { 4 } else { 3 };

            let mut multiplier = 1;

            if is_shaded {
                multiplier += 1;
            }
            if is_textured {
                multiplier += 1;
            }

            self.num_vertices = num_vertices;
            self.is_shaded = is_shaded;
            self.is_textured = is_textured;

            return num_vertices * multiplier + 1;
        }

        if upper_bits == 4 {
            return 4;
        }

        if [5,6].contains(&upper_bits) {
            return 3;
        }

        1
    }

    fn parse_texpage(word: u32) -> Texpage {
        let mut texpage = Texpage::new();

        texpage.x_base = word & 0xf;
        texpage.y_base1 = (word >> 4) & 0x1;
        texpage.semi_transparency = (word >> 5) & 0x3;
        texpage.texture_page_colors = match (word >> 7) & 0x3 {
            0 => TexturePageColors::Bit4,
            1 => TexturePageColors::Bit8,
            2 | 3 => TexturePageColors::Bit15,
            _ => panic!("reserved value for texpage colors")
        };
        texpage.dither = (word >> 9) & 0x1 == 1;
        texpage.draw_to_display_area = (word >> 10) & 0x1 == 1;
        texpage.y_base2 = (word >> 11) & 0x1;
        texpage.x_flip = (word >> 12) & 0x1 == 1;
        texpage.y_flip = (word >> 13) & 0x1 == 1;

        // for debugging purposes
        texpage.value = word;

        texpage
    }

    fn parse_color(word: u32) -> Color {
        let r = word as u8;
        let g = (word >> 8) as u8;
        let b = (word >> 16) as u8;

        Color {
            r,
            g,
            b,
            a: true
        }
    }

    fn render_polygon(&mut self) {
        let mut command_index = 0;

        let mut vertices: Vec<Vertex> = Vec::new();

        let color0 = self.current_command_buffer[0];

        for i in 0..self.num_vertices {
            let mut vertex = Vertex {
                x: 0,
                y: 0,
                u: if self.is_textured { Some(0) } else { None },
                v: if self.is_textured { Some(0) } else { None },
                color: Self::parse_color(color0),
                texpage: None
            };

            if i == 0 || self.is_shaded {
                let word = self.current_command_buffer[command_index];

                let color = &mut vertex.color;
                color.r = word as u8;
                color.g = (word >> 8) as u8;
                color.b = (word >> 16) as u8;

                command_index += 1;
            }

            let word = self.current_command_buffer[command_index];

            let mut x = word as i16 as i32;
            let mut y = (word >> 16) as i16 as i32;

            x = (x << 21) >> 21;
            y = (y << 21) >> 21;

            vertex.x = x + self.x_offset;
            vertex.y = y + self.y_offset;

            command_index += 1;

            if self.is_textured {
                let word = self.current_command_buffer[command_index];

                let u = vertex.u.as_mut().unwrap();

                *u = word & 0xff;

                let v = vertex.u.as_mut().unwrap();

                *v = (word >> 16) & 0xff;

                if i == 0 {
                    self.clut_index = (word >> 16) as usize;
                } else if i == 1 {
                    vertex.texpage = Some(Self::parse_texpage(word));
                }

                command_index += 1;
            }

            vertices.push(vertex);
        }

        self.polygons.push(Polygon::new(vertices, false));

        self.commands_ready = true;
        self.num_vertices = 0;
    }

    fn render_rectangle(&mut self) {
        self.commands_ready = true;
        todo!("render rectangle");
    }

    fn set_drawing_offset(&mut self, word: u32) {
        self.x_offset = (((word & 0x7ff) as i32) << 21) >> 21;
        self.y_offset = ((((word >> 11) & 0x7ff) as i32) << 21 ) >> 21;
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

    fn texture_window(&mut self, word: u32) {
        self.texture_window_mask_x = word & 0x1f;
        self.texture_window_mask_y = (word >> 5) & 0x1f;
        self.texture_window_offset_x = (word >> 10) & 0x1f;
        self.texture_window_offset_y = (word >> 15) & 0x1f;
    }

    fn mask_bit(&mut self, word: u32) {
        self.set_while_drawing = word & 1 == 1;
        self.check_before_drawing = (word >> 1) & 1 == 1;
    }

    fn vram_to_cpu_transfer(&mut self) {
        todo!("vram to cpu");
    }

    fn cpu_to_vram_transfer(&mut self) {
        self.transfer_type = Some(TransferType::ToVram);

        let destination = self.current_command_buffer.pop_front().unwrap();
        let dimensions = self.current_command_buffer.pop_front().unwrap();

        self.destination_x = destination & 0x3ff;
        self.destination_y = (destination >> 16) & 0x1ff;

        self.transfer_width = dimensions & 0x3ff;
        self.transfer_height = (dimensions >> 16) & 0x1ff;

        if self.transfer_width == 0 {
            self.transfer_width = 0x400;
        }
        if self.transfer_height == 0 {
            self.transfer_height = 0x200;
        }
    }

    fn vram_to_vram_transfer(&mut self) {
        todo!("vram to vram");
    }

    fn execute_command(&mut self, word: u32) {
        let command = word >> 24;
        let upper = word >> 29;

        match upper {
            1 => self.render_polygon(),
            2 => unreachable!("shouldn't happen"),
            3 => self.render_rectangle(),
            4 => self.vram_to_vram_transfer(),
            5 => self.cpu_to_vram_transfer(),
            6 => self.vram_to_cpu_transfer(),
            _ => {
                match command {
                    0x0 => (), // NOP
                    0x1 => self.command_fifo = VecDeque::with_capacity(16),
                    0x3..=0x1e => (), // NOP
                    0xe1 => self.texpage(word),
                    0xe2 => self.texture_window(word),
                    0xe3 => self.set_drawing_area(word, false),
                    0xe4 => self.set_drawing_area(word, true),
                    0xe5 => self.set_drawing_offset(word),
                    0xe6 => self.mask_bit(word),
                    _ => todo!("command: 0x{:x}", command)
                }
            }
        }


    }

    fn draw_line(&mut self) {
        self.commands_ready = true;
        todo!("draw line");
    }

    fn draw_polyline(&mut self) {
        self.commands_ready = true;
        todo!("draw polyline");
    }

    fn transfer_to_vram(&mut self, halfword: u16) {
        let curr_x = self.destination_x + self.read_x;

        self.read_x += 1;

        let curr_y = self.destination_y + self.read_y;

        let address = Self::get_vram_address(curr_x, curr_y);

        unsafe { *(&mut self.vram[address] as *mut u8 as *mut u16) = halfword };

        if self.read_x == self.transfer_width {
            self.read_x = 0;

            self.read_y += 1;

            if self.read_y == self.transfer_height {
                self.transfer_type = None;

                self.read_y = 0;
                self.transfer_width = 0;
                self.transfer_height = 0;
                self.destination_x = 0;
                self.destination_y = 0;
            }
        }
    }

    fn get_vram_address(x: u32, y: u32) -> usize {
        2 * ((x & 0x3ff) + 1024 * (y & 0x1ff)) as usize
    }

    fn process_commands(&mut self) {
        while !self.command_fifo.is_empty() {
            if let Some(transfer_type) = self.transfer_type {
                if transfer_type == TransferType::ToVram {
                    let word = self.command_fifo.pop_front().unwrap();
                    self.transfer_to_vram(word as u16);

                    if self.transfer_type.is_some() {
                        self.transfer_to_vram((word >> 16) as u16);
                    }

                    continue;
                }
            }
            let word = self.command_fifo.pop_front().unwrap();
            let upper = word >> 29;

            self.current_command_buffer.push_back(word);

            if self.words_left == 0 {
                if upper == 0x2 {
                    if (word >> 27) & 1 == 1 {
                        self.is_polyline = true;
                    }
                } else {
                    self.words_left = self.get_words_left(word);
                }
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

                return;
            }

            if self.words_left == 1 {
                let word = self.current_command_buffer[0];

                self.execute_command(word);

                self.current_command_buffer = VecDeque::new();
            }

            if self.words_left > 0 {
                self.words_left -= 1;
            }
        }
    }

    fn texpage(&mut self, word: u32) {
        self.texpage = Self::parse_texpage(word);
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

    pub fn cap_fps(&mut self) {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("an error occurred")
            .as_millis();

        if self.previous_time != 0 {
            let diff = current_time - self.previous_time;
            if diff < FPS_INTERVAL {
                sleep(Duration::from_millis((FPS_INTERVAL - diff) as u64));
            }
        }

        self.previous_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("an error occurred")
            .as_millis();
    }

    pub fn handle_vblank(
        &mut self,
        scheduler: &mut Scheduler,
        interrupt_stat: &mut InterruptRegister,
        timers: &mut [Timer],
        cycles_left: usize
    ) {
        self.even_flag = 0;

        timers[1].in_xblank = true;

        if timers[1].counter_register.contains(CounterModeRegister::SYNC_ENABLE) {
            match timers[1].counter_register.sync_mode() {
                0 => timers[1].is_active = false,
                1 => timers[1].counter = 0,
                2 => {
                    timers[1].is_active = true;
                    if self.current_line == 0 {
                        timers[1].counter =  0;
                    } else {
                        timers[1].tick(1, scheduler, interrupt_stat);
                    }
                }
                3 => if let Some(_) = &mut timers[0].switch_free_run {
                    timers[1].is_active = true;
                } else {
                    timers[1].switch_free_run = Some(true);
                }
                _ => unreachable!()
            }
        }

        if self.current_line == NUM_SCANLINES {
            self.frame_finished = true;
            self.current_line = 0;
            scheduler.schedule(EventType::HblankStart, (HBLANK_START as f32 * (7.0 / 11.0)) as usize - cycles_left);
        } else {
            scheduler.schedule(EventType::Vblank, (CYCLES_PER_SCANLINE as f32 * (7.0 / 11.0)) as usize - cycles_left);
            self.current_line += 1;
        }
    }
}