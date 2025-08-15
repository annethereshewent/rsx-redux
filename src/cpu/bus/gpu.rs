use std::{collections::VecDeque, thread::sleep, time::{Duration, SystemTime, UNIX_EPOCH}};

use super::{registers::interrupt_register::InterruptRegister, scheduler::{EventType, Scheduler}, timer::{counter_mode_register::CounterModeRegister, ClockSource, Timer}};

const HBLANK_START: usize = 2813;
const CYCLES_PER_SCANLINE: usize = 3413;
const HBLANK_END: usize = CYCLES_PER_SCANLINE - HBLANK_START;
const VBLANK_LINE_START: usize = 240;
const NUM_SCANLINES: usize = 262;
pub const FPS_INTERVAL: u128 = 1000 / 60;

#[derive(Copy, Clone, PartialEq)]
enum DmaDirection {
    Off,
    Fifo,
    ToGP0,
    ToCPU
}

#[derive(Copy, Clone, PartialEq)]
enum DisplayMode {
    Ntsc = 0,
    Pal = 1
}

#[derive(Copy, Clone, PartialEq)]
enum DisplayDepth {
    Bit15 = 0,
    Bit24 = 1
}

#[derive(Copy, Clone, PartialEq)]
enum RectangleSize {
    Variable,
    Single,
    EightxEight,
    SixteenxSixteen
}

#[derive(Copy, Clone, PartialEq)]
enum TransferType {
    FromVram,
    ToVram
}

#[derive(Copy, Clone, Debug)]
pub enum TexturePageColors {
    Bit4 = 0,
    Bit8 = 1,
    Bit15 = 2
}

#[derive(Debug)]
pub struct Polygon {
    pub vertices: Vec<Vertex>,
    pub is_line: bool,
    pub texpage: Option<Texpage>,
    pub clut: (u32, u32)
}

impl Polygon {
    pub fn new(vertices: Vec<Vertex>, is_line: bool) -> Self {
        Self {
            vertices,
            is_line,
            texpage: None,
            clut: (0, 0)
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8
}

#[derive(Debug, Copy, Clone)]
pub struct Vertex {
    pub x: i32,
    pub y: i32,
    pub u: u32,
    pub v: u32,
    pub color: Color
}


#[derive(Debug, Copy, Clone)]
pub struct Texpage {
    pub x_base: u32,
    pub y_base1: u32,
    pub semi_transparency: u32,
    pub texture_page_colors: TexturePageColors,
    pub dither: bool,
    pub draw_to_display_area: bool,
    pub y_base2: u32,
    pub x_flip: bool,
    pub y_flip: bool,
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
    pub x1: u32,
    pub x2: u32,
    pub y1: u32,
    pub y2: u32,
    x_offset: i32,
    y_offset: i32,
    pub texture_window_mask_x: u32,
    pub texture_window_offset_x: u32,
    pub texture_window_mask_y: u32,
    pub texture_window_offset_y: u32,
    set_while_drawing: bool,
    check_before_drawing: bool,
    pub polygons: Vec<Polygon>,
    pub commands_ready: bool,
    num_vertices: usize,
    is_shaded: bool,
    is_textured: bool,
    clut_x: usize,
    clut_y: usize,
    transfer_x: u32,
    transfer_y: u32,
    transfer_width: u32,
    transfer_height: u32,
    transfer_type: Option<TransferType>,
    read_x: u32,
    read_y: u32,
    pub vram: Box<[u8]>,
    previous_time: u128,
    is_semitransparent: bool,
    modulate: bool,
    rectangle_size: RectangleSize,
    pub irq_enabled: bool,
    pub display_width: u32,
    pub display_height: u32,
    video_mode: DisplayMode,
    display_depth: DisplayDepth,
    horizontal_flip: bool,
    dma_direction: DmaDirection,
    horizontal_bits1: u32,
    horizontal_bit2: u32,
    display_start_x: u32,
    display_start_y: u32,
    display_range_x: (u32, u32),
    display_range_y: (u32, u32),
    display_on: bool,
    pub vram_dirty: bool,
    pub debug_on: bool
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
            clut_x: 0,
            clut_y: 0,
            transfer_height: 0,
            transfer_width: 0,
            transfer_x: 0,
            transfer_y: 0,
            transfer_type: None,
            read_x: 0,
            read_y: 0,
            vram: vec![0; 1024 * 512 * 2].into_boxed_slice(),
            previous_time: 0,
            is_semitransparent: false,
            modulate: false,
            rectangle_size: RectangleSize::Single,
            irq_enabled: false,
            display_height: 240,
            display_width: 320,
            video_mode: DisplayMode::Ntsc,
            display_depth: DisplayDepth::Bit15,
            horizontal_flip: false,
            dma_direction: DmaDirection::Off,
            horizontal_bit2: 0,
            horizontal_bits1: 0,
            display_start_x: 0,
            display_start_y: 0,
            display_range_x: (0, 0),
            display_range_y: (0, 0),
            display_on: true,
            vram_dirty: false,
            debug_on: false
        }
    }

    pub fn read_gpu(&mut self) -> u32 {
        if let Some(transfer_type) = self.transfer_type {
            if transfer_type == TransferType::FromVram {
                let lower = self.transfer_to_cpu();
                let upper = self.transfer_to_cpu();

                return lower as u32 | (upper as u32) << 16;
            }
        }

        self.gpuread
    }

    pub fn transfer_to_cpu(&mut self) -> u16 {
        let curr_x = self.transfer_x + self.read_x;
        let curr_y = self.transfer_y + self.read_y;

        let address = Self::get_vram_address(curr_x, curr_y);

        let value = unsafe { *(&self.vram[address as usize] as *const u8 as *const u16) };

        self.read_x += 1;

        if self.read_x == self.transfer_width {
            self.read_x = 0;

            self.read_y += 1;

            if self.read_y == self.transfer_height {
                self.transfer_type = None;
            }
        }

        value
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
            self.frame_finished = true;

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
            let is_semitransparent = (word >> 25) & 1 == 1;
            let is_shaded = (word >> 28) & 1 == 1;
            let modulate = (word >> 24) & 1 == 1;

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
            self.is_semitransparent = is_semitransparent;
            self.modulate = modulate;

            // return num_vertices * multiplier + 1;
            if !is_shaded {
                return num_vertices * multiplier + 1;
            }

            return num_vertices * multiplier;
        }

        if upper_bits == 0x3 {
            let mut num_words = 2;
            self.is_textured = (word >> 26) & 1 == 1;
            self.is_semitransparent = (word >> 25) & 1 == 1;
            self.modulate = (word >> 24) &1 == 1;

            self.rectangle_size = match (word >> 27) & 0x3 {
                0 => RectangleSize::Variable,
                1 => RectangleSize::Single,
                2 => RectangleSize::EightxEight,
                3 => RectangleSize::SixteenxSixteen,
                _ => unreachable!()
            };

            if self.is_textured {
                num_words += 1;
            }

            if self.rectangle_size == RectangleSize::Variable {
                num_words += 1;
            }

            return num_words;
        }

        if upper_bits == 0x4 {
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

    fn parse_color(&self, word: u32) -> Color {
        let r = word as u8;
        let g = (word >> 8) as u8;
        let b = (word >> 16) as u8;

        Color {
            r,
            g,
            b,
            a: 255
        }
    }

    fn push_polygon(&mut self) {
        let mut command_index = 0;

        let mut texpage: Option<Texpage> = None;

        let mut vertices: Vec<Vertex> = Vec::new();

        let color0 = self.current_command_buffer[0];

        for i in 0..self.num_vertices {
            let mut vertex = Vertex {
                x: 0,
                y: 0,
                u: 0,
                v: 0,
                color: self.parse_color(color0)
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

            let x = word as i16 as i32;
            let y = (word >> 16) as i16 as i32;

            vertex.x = x + self.x_offset;
            vertex.y = y + self.y_offset;

            command_index += 1;

            if self.is_textured {
                let word = self.current_command_buffer[command_index];

                vertex.u = word & 0xff;

                vertex.v = (word >> 8) & 0xff;

                if i == 0 {
                    (self.clut_x, self.clut_y) = Self::parse_clut(word >> 16);
                } else if i == 1 {
                    texpage = Some(Self::parse_texpage(word >> 16))
                }

                command_index += 1;
            }

            vertices.push(vertex);
        }

        let mut polygons: Vec<Polygon> = Vec::new();

        if vertices.len() > 3 {
            // split up into two polygons
            let vertices1 = vec![vertices[0], vertices[1], vertices[2]];
            let vertices2 = vec![vertices[1], vertices[2], vertices[3]];

            polygons.push(Polygon {
                vertices: vertices1,
                is_line: false,
                texpage: texpage.clone(),
                clut: (self.clut_x as u32, self.clut_y as u32)
            });

            polygons.push(Polygon {
                vertices: vertices2,
                is_line: false,
                texpage,
                clut: (self.clut_x as u32, self.clut_y as u32)
            });
        } else {
            polygons.push(Polygon {
                vertices,
                is_line: false,
                texpage,
                clut: (self.clut_x as u32, self.clut_y as u32)
            });
        }

        self.polygons.append(&mut polygons);

        self.commands_ready = true;
        self.num_vertices = 0;
    }

    fn parse_clut(word: u32) -> (usize, usize) {
        let x = (word & 0x3f) * 16;
        let y = (word >> 6) & 0x1ff;

        (x as usize, y as usize)
    }

    fn push_rectangle(&mut self) {
        self.commands_ready = true;

        let word = self.current_command_buffer.pop_front().unwrap();

        let color = self.parse_color(word);

        let word = self.current_command_buffer.pop_front().unwrap();

        let x = word as i16 as i32;
        let y = (word >> 16) as i16 as i32;

        let mut u = 0;
        let mut v = 0;

        if self.is_textured {
            let word = self.current_command_buffer.pop_front().unwrap();

            u = word & 0xff;
            v = (word >> 8) & 0xff;
        }

        let (width, height) = match self.rectangle_size {
            RectangleSize::Variable => {
                let word = self.current_command_buffer.pop_front().unwrap();

                let width = (word & 0x3ff) as i32;
                let height = ((word >> 16) & 0x1ff) as i32;

                (width, height)
            }
            RectangleSize::EightxEight => (8, 8),
            RectangleSize::SixteenxSixteen => (16, 16),
            RectangleSize::Single => (1, 1)
        };

        // calculate the other vertices and push this to polygons!

        let v0 = Vertex {
            x,
            y,
            u,
            v,
            color
        };

        let v1 = Vertex {
            x: x + width,
            y,
            u: u + width as u32,
            v,
            color
        };

        let v2 = Vertex {
            x,
            y: y + height,
            u,
            v: v + height as u32 ,
            color
        };

        let v3 = Vertex {
            x: x + width,
            y: y + height,
            u: u + width as u32,
            v: v + height as u32,
            color
        };

        let vertices = vec![v0, v1, v2, v3];

        let vertices1 = vec![vertices[0], vertices[1], vertices[2]];
        let vertices2 = vec![vertices[1], vertices[2], vertices[3]];

        self.polygons.push(Polygon {
            vertices: vertices1,
            is_line: false,
            texpage: Some(self.texpage.clone()),
            clut: (self.clut_x as u32, self.clut_y as u32)
        });
        self.polygons.push(Polygon {
            vertices: vertices2,
            is_line: false,
            texpage: Some(self.texpage.clone()),
            clut: (self.clut_x as u32, self.clut_y as u32)
        });

        self.num_vertices = 0;

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
        self.transfer_type = Some(TransferType::FromVram);

        self.current_command_buffer.pop_front().unwrap();

        let source = self.current_command_buffer.pop_front().unwrap();
        let dimensions = self.current_command_buffer.pop_front().unwrap();

        self.transfer_x = source & 0x3ff;
        self.transfer_y = (source >> 16) & 0x1ff;

        self.transfer_width = dimensions & 0x3ff;
        self.transfer_height = (dimensions >> 16) & 0x1ff;

        self.read_y = 0;
        self.read_x = 0;
    }

    fn cpu_to_vram_transfer(&mut self) {
        // this is a dumb hack because i dont pop the actual command in execute_command, so i do it here
        self.current_command_buffer.pop_front().unwrap();

        self.transfer_type = Some(TransferType::ToVram);

        let destination = self.current_command_buffer.pop_front().unwrap();
        let dimensions = self.current_command_buffer.pop_front().unwrap();

        self.transfer_x = destination & 0x3ff;
        self.transfer_y = (destination >> 16) & 0x1ff;

        self.transfer_width = dimensions & 0x3ff;
        self.transfer_height = (dimensions >> 16) & 0x1ff;

        self.read_y = 0;
        self.read_x = 0;

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

    pub fn cross_product(v: &Vec<Vertex>) -> i32 {
        (v[1].x - v[0].x) * (v[2].y - v[0].y) - (v[1].y - v[0].y) * (v[2].x - v[0].x)
    }
    fn execute_command(&mut self, word: u32) {
        let command = word >> 24;
        let upper = word >> 29;

        if self.debug_on {
            println!("got word 0x{:x}", word);
        }

        match upper {
            1 => self.push_polygon(),
            2 => unreachable!("shouldn't happen"),
            3 => self.push_rectangle(),
            4 => self.vram_to_vram_transfer(),
            5 => self.cpu_to_vram_transfer(),
            6 => self.vram_to_cpu_transfer(),
            _ => {
                match command {
                    0x0 => (), // NOP
                    0x1 => (), // TODO: invalidate cache
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
        let curr_x = self.transfer_x + self.read_x;

        self.read_x += 1;

        let curr_y = self.transfer_y + self.read_y;

        let address = Self::get_vram_address(curr_x, curr_y);

        unsafe { *(&mut self.vram[address] as *mut u8 as *mut u16) = halfword };

        self.vram_dirty = true;

        if self.read_x == self.transfer_width {
            self.read_x = 0;

            self.read_y += 1;

            if self.read_y == self.transfer_height {
                self.transfer_type = None;

                self.read_y = 0;
                self.transfer_width = 0;
                self.transfer_height = 0;
                self.transfer_x = 0;
                self.transfer_y = 0;
            }
        }
    }

    fn get_vram_address(x: u32, y: u32) -> usize {
        2 * ((x & 0x3ff) + 1024 * (y & 0x1ff)) as usize
    }

    pub fn process_gp1_commands(&mut self, word: u32) {
        let command = word >> 24;
        match command {
            0x0 => self.reset_gpu(word),
            0x1 => self.current_command_buffer.clear(),
            0x2 => self.irq_enabled = false,
            0x3 => self.display_on = word & 1 == 1,
            0x4 => self.dma_direction = match word & 0x3 {
                0 => DmaDirection::Off,
                1 => DmaDirection::Fifo,
                2 => DmaDirection::ToGP0,
                3 => DmaDirection::ToCPU,
                _ => unreachable!()
            },
            0x5 => self.display_area_start(word),
            0x6 => self.display_range_horizontal(word),
            0x7 => self.display_range_vertical(word),
            0x8 => self.display_mode(word),
            _ => todo!("gp1 0x{:x}", command)
        }
    }

    fn display_range_horizontal(&mut self, word: u32) {
        self.display_range_x = (word & 0xfff, (word >> 12) & 0x1ff);
    }

    fn display_range_vertical(&mut self, word: u32) {
        self.display_range_y = (word & 0x3ff, (word >> 10) & 0x3ff);
    }

    fn display_area_start(&mut self, word: u32) {
        self.display_start_x = word & 0x3ff;
        self.display_start_y = (word >> 10) & 0x1ff;
    }

    /*
    0-1   Horizontal Resolution 1     (0=256, 1=320, 2=512, 3=640) ;GPUSTAT.17-18
    2     Vertical Resolution         (0=240, 1=480, when Bit5=1)  ;GPUSTAT.19
    3     Video Mode                  (0=NTSC/60Hz, 1=PAL/50Hz)    ;GPUSTAT.20
    4     Display Area Color Depth    (0=15bit, 1=24bit)           ;GPUSTAT.21
    5     Vertical Interlace          (0=Off, 1=On)                ;GPUSTAT.22
    6     Horizontal Resolution 2     (0=256/320/512/640, 1=368)   ;GPUSTAT.16
    7     Flip screen horizontally    (0=Off, 1=On, v1 only)       ;GPUSTAT.14
    8-23  Not used (zero)
    */
    fn display_mode(&mut self, word: u32) {
        self.display_width = if (word >> 6) & 0x1 == 1 {
            368
        }  else {
            match word & 0x3 {
                0 => 256,
                1 => 320,
                2 => 512,
                3 => 640,
                _ => unreachable!()
            }
        };

        self.horizontal_bits1 = word & 0x3;
        self.horizontal_bit2 = (word >> 6) & 0x1;


        self.interlaced = (word >> 5) & 0x1 == 1;

        self.display_height = match (word >> 2) & 0x1 {
            0 => 240,
            1 => 480,
            _ => unreachable!()
        };

        if !self.interlaced {
            self.display_height = 240;
        }

        self.video_mode = match (word >> 3) & 0x1 {
            0 => DisplayMode::Ntsc,
            1 => DisplayMode::Pal,
            _ => unreachable!()
        };

        self.display_depth = match (word >> 4) & 0x1 {
            0 => DisplayDepth::Bit15,
            1 => DisplayDepth::Bit24,
            _ => unreachable!()
        };

        self.horizontal_flip = (word >> 7) & 0x1 == 1;
    }

    pub fn reset_gpu(&mut self, _: u32) {
        self.current_command_buffer.clear();
        self.transfer_type = None;

        self.texture_window_mask_x = 0;
        self.texture_window_mask_y = 0;
        self.texture_window_offset_x = 0;
        self.texture_window_offset_y = 0;

        self.texpage = Texpage::new();
    }

    pub fn process_gp0_commands(&mut self, word: u32) {
        if self.debug_on {
            println!("received word 0x{:x}, words remaining = {}", word, self.words_left);
        }
        if let Some(transfer_type) = self.transfer_type {
            if transfer_type == TransferType::ToVram {
                self.transfer_to_vram(word as u16);

                if self.transfer_type.is_some() {
                    self.transfer_to_vram((word >> 16) as u16);
                }
                return;
            }
        }
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

    fn texpage(&mut self, word: u32) {
        self.texpage = Self::parse_texpage(word);
    }

    pub fn read_stat(&self) -> u32 {
        let vertical_bits = match self.display_height {
            240 => 0,
            480 => 1,
            _ => 0
        };

        let bit31 = if self.current_line < VBLANK_LINE_START {
            self.even_flag
        } else {
            0
        };

        let value = self.even_flag << 31 |
            self.texpage.x_base |
            self.texpage.y_base1 << 4 |
            self.texpage.semi_transparency << 5 |
            (self.texpage.texture_page_colors as u32) << 7 |
            (self.texpage.dither as u32) << 9 |
            (self.texpage.draw_to_display_area as u32) << 10 |
            (self.set_while_drawing as u32) << 11 |
            (self.check_before_drawing as u32) << 12 |
            (self.interlaced as u32) << 13 |
            (self.horizontal_flip as u32) << 14 |
            self.texpage.y_base2 << 15 |
            self.horizontal_bit2 << 16 |
            self.horizontal_bits1 << 17 |
            vertical_bits << 19 |
            (self.video_mode as u32) << 20 |
            (self.display_depth as u32) << 21 |
            (self.interlaced as u32) << 22 |
            (self.display_on as u32) << 23 | // TODO: display enable
            (self.irq_enabled as u32) << 24 |
            0x7 << 26 |
            (self.dma_direction as u32) << 29 |
            bit31 << 31;

        value
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
            self.current_line = 0;
            scheduler.schedule(EventType::HblankStart, (HBLANK_START as f32 * (7.0 / 11.0)) as usize - cycles_left);
        } else {
            scheduler.schedule(EventType::Vblank, (CYCLES_PER_SCANLINE as f32 * (7.0 / 11.0)) as usize - cycles_left);
            self.current_line += 1;
        }
    }
}