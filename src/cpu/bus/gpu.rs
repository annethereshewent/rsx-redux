use std::{
    array::from_fn,
    collections::VecDeque,
    thread::sleep,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::cpu::CPU_FREQUENCY;

use super::{
    registers::interrupt_register::InterruptRegister,
    scheduler::{EventType, Scheduler},
    timer::{ClockSource, Timer, counter_mode_register::CounterModeRegister},
};

pub mod deltas;
#[cfg(feature = "software_gpu")]
pub mod render;

const HBLANK_START: usize = 2813;
const CYCLES_PER_SCANLINE: usize = 3413;
const HBLANK_END: usize = CYCLES_PER_SCANLINE - HBLANK_START;
const VBLANK_LINE_START: usize = 240;
const NUM_SCANLINES: usize = 262;
#[cfg(feature = "software_gpu")]
const VRAM_SIZE: usize = 2 * 1024 * 512;
pub const VRAM_WIDTH: usize = 1024;
pub const VRAM_HEIGHT: usize = 512;
pub const FPS_INTERVAL: u128 = 1000 / 60;

pub const SCREEN_WIDTH: usize = 640;
pub const SCREEN_HEIGHT: usize = 480;

pub const GPU_FREQUENCY: f64 = 53_693_181.818;
pub const GPU_CYCLES_TO_CPU_CYCLES: f64 = CPU_FREQUENCY / GPU_FREQUENCY;
pub const CPU_CYCLES_TO_GPU_CYCLES: f64 = GPU_FREQUENCY / CPU_FREQUENCY;
// these are the old cycle conversions, uncomment if new one causes issues
// pub const GPU_CYCLES_TO_CPU_CYCLES: f64 = 7.0 / 11.0
// pub const CPU_CYCLES_TO_GPU_CYCLES: f64 = 11.0 / 7.0

// per https://psx-spx.consoledev.net/graphicsprocessingunitgpu/#24bit-rgb-to-15bit-rgb-dithering-enabled-in-texpage-attribute
const DITHER_OFFSETS: [[i16; 4]; 4] = [
    [-4, 0, -3, 1],
    [2, -2, 3, -1],
    [-3, 1, -4, 0],
    [3, -1, 2, -2],
];

#[derive(Clone, Serialize, Deserialize)]
pub enum GPUCommand {
    CPUtoVram(VRamTransferParams),
    VRAMtoCPU(CPUTransferParams),
    FillVRAM(FillVramParams),
    VramToVram(VramToVramTransferParams),
    RenderPolygon(Polygon),
}

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct VramToVramTransferParams {
    pub source_start_x: u32,
    pub source_start_y: u32,
    pub destination_start_x: u32,
    pub destination_start_y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct VRamTransferParams {
    pub halfwords: Vec<u16>,
    pub start_x: u32,
    pub start_y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct CPUTransferParams {
    pub start_x: u32,
    pub start_y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct FillVramParams {
    pub start_x: u32,
    pub start_y: u32,
    pub width: u32,
    pub height: u32,
    pub pixel: u16,
}

#[derive(Copy, Clone, PartialEq, Serialize, Deserialize)]
enum DmaDirection {
    Off,
    Fifo,
    ToGP0,
    ToCPU,
}

#[derive(Copy, Clone, PartialEq, Serialize, Deserialize)]
enum DisplayMode {
    Ntsc = 0,
    Pal = 1,
}

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum DisplayDepth {
    Bit15 = 0,
    Bit24 = 1,
}

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Semitransparency {
    Half = 0,
    Add = 1,
    Subtract = 2,
    Quarter = 3,
}

#[derive(Copy, Clone, PartialEq, Serialize, Deserialize)]
enum RectangleSize {
    Variable,
    Single,
    EightxEight,
    SixteenxSixteen,
}

#[derive(Copy, Clone, PartialEq, Serialize, Deserialize)]
enum TransferType {
    FromVram,
    ToVram,
}

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TexturePageColors {
    Bit4 = 0,
    Bit8 = 1,
    Bit15 = 2,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Polygon {
    pub vertices: Vec<Vertex>,
    pub is_line: bool,
    pub is_shaded: bool,
    pub semitransparent: bool,
    pub textured: bool,
    pub texpage: Option<Texpage>,
    pub modulate: bool,
    pub transparent_mode: u32,
    pub clut: (u32, u32),
    pub texture_mask_x: u32,
    pub texture_mask_y: u32,
    pub texture_offset_x: u32,
    pub texture_offset_y: u32,
    pub x1: u32,
    pub x2: u32,
    pub y1: u32,
    pub y2: u32,
    pub force_mask_bit: bool,
    pub preserve_masked_pixels: bool,
}

impl Polygon {
    pub fn new(vertices: Vec<Vertex>, is_line: bool) -> Self {
        Self {
            vertices,
            is_line,
            semitransparent: false,
            textured: false,
            is_shaded: false,
            texpage: None,
            clut: (0, 0),
            transparent_mode: 0,
            modulate: false,
            ..Default::default()
        }
    }
}

#[derive(Debug, Copy, Clone, Default, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub fn translate15bit_to_24(val: u16) -> Color {
        let mut r = (val & 0x1f) as u8;
        let mut g = ((val >> 5) & 0x1f) as u8;
        let mut b = ((val >> 10) & 0x1f) as u8;
        let a = ((val >> 15) & 0b1) as u8;

        r = (r << 3) | (r >> 2);
        g = (g << 3) | (g >> 2);
        b = (b << 3) | (b >> 2);

        Self { r, g, b, a }
    }
    pub fn color_to_u16(color: Color) -> u16 {
        let mut pixel = 0;

        pixel |= ((color.r as u16) & 0xf8) >> 3;
        pixel |= ((color.g as u16) & 0xf8) << 2;
        pixel |= ((color.b as u16) & 0xf8) << 7;
        pixel |= (color.a as u16) << 15;

        pixel
    }
}

#[derive(Debug, Copy, Clone, Default, Serialize, Deserialize)]
pub struct Vertex {
    pub x: i32,
    pub y: i32,
    pub u: u32,
    pub v: u32,
    pub color: Color,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Texpage {
    pub x_base: u32,
    pub y_base1: u32,
    pub semi_transparency: Semitransparency,
    pub texture_page_colors: TexturePageColors,
    pub dither: bool,
    pub draw_to_display_area: bool,
    pub y_base2: u32,
    pub x_flip: bool,
    pub y_flip: bool,
    pub value: u32,
}

impl Texpage {
    pub fn new() -> Self {
        Self {
            x_base: 0,
            y_base1: 0,
            semi_transparency: Semitransparency::Half,
            texture_page_colors: TexturePageColors::Bit4,
            dither: false,
            draw_to_display_area: false,
            y_base2: 0,
            x_flip: false,
            y_flip: false,
            value: 0,
        }
    }
}

impl Default for Texpage {
    fn default() -> Self {
        Self::new()
    }
}
#[derive(Serialize, Deserialize)]
pub struct GPU {
    pub frame_finished: bool,
    pub current_line: usize,
    even_flag: u32,
    pub interlaced: bool,
    pub current_command_buffer: VecDeque<u32>,
    texpage: Texpage,
    pub gpuread: u32,
    words_left: usize,
    is_polyline: bool,
    previous_line_vertex: Option<Vertex>,
    previous_line_color: Option<Color>,
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
    pub force_mask_bit: bool,
    pub preserve_masked_pixels: bool,
    #[cfg(any(feature = "hardware_gpu", feature = "hardware_gpu_web"))]
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
    previous_time: u128,
    is_semitransparent: bool,
    modulate: bool,
    rectangle_size: RectangleSize,
    pub irq_enabled: bool,
    pub display_width: u32,
    pub display_height: u32,
    video_mode: DisplayMode,
    pub display_depth: DisplayDepth,
    horizontal_flip: bool,
    dma_direction: DmaDirection,
    horizontal_bits1: u32,
    horizontal_bit2: u32,
    pub display_start_x: u32,
    pub display_start_y: u32,
    display_range_x: (u32, u32),
    display_range_y: (u32, u32),
    display_on: bool,
    dither_table: [[Box<[u8]>; 4]; 4],
    pub debug_on: bool,
    pub gpu_commands: Vec<GPUCommand>,
    pub gpuread_fifo: VecDeque<u16>,
    pub vram_transfer_halfwords: Vec<u16>,
    pub transfer_params: Option<CPUTransferParams>,
    pub resolution_changed: bool,
    #[cfg(feature = "software_gpu")]
    pub vram: Box<[u8]>,
    #[cfg(any(feature = "hardware_gpu", feature = "hardware_gpu_web"))]
    pub vram_read_tex: Box<[u8]>,
    #[cfg(any(feature = "hardware_gpu", feature = "hardware_gpu_web"))]
    pub vram_write_tex: Box<[u8]>,
    dotclock_cycles: usize,
    cpu_transfer_x: u32,
    cpu_transfer_y: u32,
    cpu_transfer_width: u32,
    cpu_transfer_height: u32,
    #[cfg(feature = "software_gpu")]
    pub picture: Box<[u8]>,
}

impl GPU {
    pub fn new(scheduler: &mut Scheduler) -> Self {
        let mut dither_table = from_fn(|_| from_fn(|_| vec![0; 0x200].into_boxed_slice()));

        for x in 0..4 {
            for y in 0..4 {
                for i in 0..0x200 {
                    let out = i + DITHER_OFFSETS[x][y];

                    let out = if out < 0 {
                        0
                    } else if out > 0xff {
                        0xff
                    } else {
                        out as u8
                    };

                    dither_table[x][y][i as usize] = out;
                }
            }
        }
        scheduler.schedule(
            EventType::HblankStart,
            (CYCLES_PER_SCANLINE as f64 * (GPU_CYCLES_TO_CPU_CYCLES)) as usize,
        );

        Self {
            frame_finished: false,
            current_line: 0,
            even_flag: 0,
            interlaced: false,
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
            force_mask_bit: false,
            preserve_masked_pixels: false,
            #[cfg(any(feature = "hardware_gpu", feature = "hardware_gpu_web"))]
            commands_ready: false,
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
            display_range_x: (512, 3072),
            display_range_y: (16, 256),
            display_on: true,
            debug_on: false,
            gpu_commands: Vec::new(),
            gpuread_fifo: VecDeque::new(),
            vram_transfer_halfwords: Vec::new(),
            transfer_params: None,
            resolution_changed: false,
            dotclock_cycles: 0,
            #[cfg(feature = "software_gpu")]
            vram: vec![0; VRAM_SIZE].into_boxed_slice(),
            cpu_transfer_x: 0,
            cpu_transfer_y: 0,
            cpu_transfer_width: 0,
            cpu_transfer_height: 0,
            #[cfg(feature = "software_gpu")]
            picture: vec![0; VRAM_WIDTH * VRAM_HEIGHT * 3].into_boxed_slice(),
            dither_table,
            #[cfg(any(feature = "hardware_gpu", feature = "hardware_gpu_web"))]
            vram_read_tex: vec![0; VRAM_WIDTH * VRAM_HEIGHT * 2].into_boxed_slice(),
            #[cfg(any(feature = "hardware_gpu", feature = "hardware_gpu_web"))]
            vram_write_tex: vec![0; VRAM_WIDTH * VRAM_HEIGHT * 4].into_boxed_slice(),
            previous_line_vertex: None,
            previous_line_color: None,
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

    pub fn get_dimensions(&self) -> (u32, u32) {
        let dotclock = self.get_dotclock() as u32;
        let mut w = if self.display_range_x.0 <= self.display_range_x.1 {
            self.display_range_x.1 - self.display_range_x.0
        } else {
            50
        };

        w = ((w / dotclock) + 2) & !0b11;
        let mut h = self.display_range_y.1 - self.display_range_y.0;

        if self.interlaced {
            h *= 2;
        }

        (w, h)
    }

    #[cfg(feature = "software_gpu")]
    fn transfer_to_cpu(&mut self) -> u16 {
        let curr_x = self.cpu_transfer_x + self.read_x;
        let curr_y = self.cpu_transfer_y + self.read_y;

        self.read_x += 1;

        if self.read_x == self.cpu_transfer_width {
            self.read_x = 0;

            self.read_y += 1;

            if self.read_y == self.cpu_transfer_height {
                self.transfer_type = None;
            }
        }

        let vram_address = GPU::get_vram_address(curr_x & 0x3ff, curr_y & 0x1ff);

        unsafe { *(&self.vram[vram_address] as *const u8 as *const u16) }
    }

    #[cfg(any(feature = "hardware_gpu", feature = "hardware_gpu_web"))]
    fn transfer_to_cpu(&mut self) -> u16 {
        if !self.gpuread_fifo.is_empty() {
            let value = self.gpuread_fifo.pop_front().unwrap();

            if self.gpuread_fifo.is_empty() {
                self.transfer_type = None;
            }

            return value;
        }

        return 0;
    }

    pub fn handle_hblank_start(
        &mut self,
        scheduler: &mut Scheduler,
        timers: &mut [Timer],
        cycles_left: usize,
    ) {
        timers[0].in_xblank = true;
        timers[1].in_xblank = false;

        scheduler.schedule(
            EventType::HblankEnd,
            (HBLANK_END as f64 * (GPU_CYCLES_TO_CPU_CYCLES)) as usize - cycles_left,
        );
    }

    fn get_dotclock(&self) -> usize {
        match self.display_width {
            320 => 8,
            640 => 4,
            256 => 10,
            512 => 5,
            368 => 7,
            _ => unreachable!(),
        }
    }

    pub fn handle_hblank(
        &mut self,
        scheduler: &mut Scheduler,
        interrupt_stat: &mut InterruptRegister,
        timers: &mut [Timer],
        cycles_left: usize,
    ) {
        timers[0].in_xblank = false;

        if timers[0]
            .counter_register
            .contains(CounterModeRegister::SYNC_ENABLE)
        {
            timers[0].handle_xblank_sync();
        }

        let dotclock = self.get_dotclock();

        let elapsed =
            CYCLES_PER_SCANLINE + (cycles_left as f64 * (CPU_CYCLES_TO_GPU_CYCLES)) as usize;

        self.dotclock_cycles += elapsed;

        if timers[0].clock_source == ClockSource::DotClock {
            timers[0].tick(self.dotclock_cycles / dotclock, interrupt_stat);
        }

        self.dotclock_cycles %= dotclock;

        if timers[1].clock_source == ClockSource::Hblank {
            timers[1].tick(1, interrupt_stat);
        }

        if self.current_line < VBLANK_LINE_START {
            scheduler.schedule(
                EventType::HblankStart,
                (HBLANK_START as f64 * (GPU_CYCLES_TO_CPU_CYCLES)) as usize - cycles_left,
            );
        } else {
            timers[1].in_xblank = true;
            self.frame_finished = true;

            interrupt_stat.insert(InterruptRegister::VBLANK);

            scheduler.schedule(
                EventType::Vblank,
                (CYCLES_PER_SCANLINE as f64 * (GPU_CYCLES_TO_CPU_CYCLES)) as usize - cycles_left,
            );
        }

        if self.interlaced {
            self.even_flag ^= 1;
        } else {
            self.even_flag = 1;
        }

        self.current_line += 1;
    }

    fn get_words_left(&mut self, word: u32) -> usize {
        let upper_bits = word >> 29;

        match upper_bits {
            0x1 => {
                // render polygon command
                let is_textured = (word >> 26) & 1 == 1;
                let is_semitransparent = (word >> 25) & 1 == 1;
                let is_shaded = (word >> 28) & 1 == 1;
                let modulate = (word >> 24) & 1 == 0;

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

                if !is_shaded {
                    return num_vertices * multiplier + 1;
                }

                num_vertices * multiplier
            }
            0x2 => {
                self.is_shaded = (word >> 28) & 1 == 1;
                self.is_polyline = (word >> 27) & 1 == 1;
                self.is_semitransparent = (word >> 25) & 1 == 1;

                self.previous_line_color = None;
                self.previous_line_vertex = None;

                if self.is_shaded { 4 } else { 3 }
            }
            0x3 => {
                let mut num_words = 2;
                self.is_textured = (word >> 26) & 1 == 1;
                self.is_semitransparent = (word >> 25) & 1 == 1;
                self.modulate = (word >> 24) & 1 == 0;

                self.rectangle_size = match (word >> 27) & 0x3 {
                    0 => RectangleSize::Variable,
                    1 => RectangleSize::Single,
                    2 => RectangleSize::EightxEight,
                    3 => RectangleSize::SixteenxSixteen,
                    _ => unreachable!(),
                };

                if self.is_textured {
                    num_words += 1;
                }

                if self.rectangle_size == RectangleSize::Variable {
                    num_words += 1;
                }

                num_words
            }
            0x4 => 4,
            5 | 6 => 3,
            _ => {
                if (word >> 24) == 0x2 {
                    3
                } else {
                    1
                }
            }
        }
    }

    fn parse_texpage(word: u32) -> Texpage {
        let mut texpage = Texpage::new();

        texpage.x_base = word & 0xf;
        texpage.y_base1 = word & 0x10;
        texpage.semi_transparency = match (word >> 5) & 0x3 {
            0 => Semitransparency::Half,
            1 => Semitransparency::Add,
            2 => Semitransparency::Subtract,
            3 => Semitransparency::Quarter,
            _ => unreachable!(),
        };
        texpage.texture_page_colors = match (word >> 7) & 0x3 {
            0 => TexturePageColors::Bit4,
            1 => TexturePageColors::Bit8,
            2 | 3 => TexturePageColors::Bit15,
            _ => panic!("reserved value for texpage colors"),
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

        Color { r, g, b, a: 0 }
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
                color: Self::parse_color(color0),
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

            let (x, y) = self.parse_position(word);

            vertex.x = x;
            vertex.y = y;

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

        #[cfg(any(feature = "hardware_gpu", feature = "hardware_gpu_web"))]
        {
            if vertices.len() > 3 {
                // split up into two triangles
                let vertices1 = vec![vertices[0], vertices[1], vertices[2]];
                let vertices2 = vec![vertices[1], vertices[2], vertices[3]];

                self.gpu_commands.push(GPUCommand::RenderPolygon(Polygon {
                    vertices: vertices1,
                    is_line: false,
                    textured: self.is_textured,
                    texpage,
                    transparent_mode: if let Some(texpage) = texpage {
                        texpage.semi_transparency as u32
                    } else {
                        self.texpage.semi_transparency as u32
                    },
                    clut: (self.clut_x as u32, self.clut_y as u32),
                    semitransparent: self.is_semitransparent,
                    is_shaded: self.is_shaded,
                    modulate: self.modulate,
                    texture_mask_x: self.texture_window_mask_x,
                    texture_mask_y: self.texture_window_mask_y,
                    texture_offset_x: self.texture_window_offset_x,
                    texture_offset_y: self.texture_window_offset_y,
                    x1: self.x1,
                    x2: self.x2,
                    y1: self.y1,
                    y2: self.y2,
                    force_mask_bit: self.force_mask_bit,
                    preserve_masked_pixels: self.preserve_masked_pixels,
                }));

                self.gpu_commands.push(GPUCommand::RenderPolygon(Polygon {
                    vertices: vertices2,
                    is_line: false,
                    texpage,
                    clut: (self.clut_x as u32, self.clut_y as u32),
                    semitransparent: self.is_semitransparent,
                    textured: self.is_textured,
                    transparent_mode: if let Some(texpage) = texpage {
                        texpage.semi_transparency as u32
                    } else {
                        self.texpage.semi_transparency as u32
                    },
                    is_shaded: self.is_shaded,
                    modulate: self.modulate,
                    texture_mask_x: self.texture_window_mask_x,
                    texture_mask_y: self.texture_window_mask_y,
                    texture_offset_x: self.texture_window_offset_x,
                    texture_offset_y: self.texture_window_offset_y,
                    x1: self.x1,
                    x2: self.x2,
                    y1: self.y1,
                    y2: self.y2,
                    force_mask_bit: self.force_mask_bit,
                    preserve_masked_pixels: self.preserve_masked_pixels,
                }));
            } else {
                self.gpu_commands.push(GPUCommand::RenderPolygon(Polygon {
                    vertices,
                    is_line: false,
                    texpage,
                    clut: (self.clut_x as u32, self.clut_y as u32),
                    semitransparent: self.is_semitransparent,
                    textured: self.is_textured,
                    transparent_mode: if let Some(texpage) = texpage {
                        texpage.semi_transparency as u32
                    } else {
                        self.texpage.semi_transparency as u32
                    },
                    is_shaded: self.is_shaded,
                    modulate: self.modulate,
                    texture_mask_x: self.texture_window_mask_x,
                    texture_mask_y: self.texture_window_mask_y,
                    texture_offset_x: self.texture_window_offset_x,
                    texture_offset_y: self.texture_window_offset_y,
                    x1: self.x1,
                    x2: self.x2,
                    y1: self.y1,
                    y2: self.y2,
                    force_mask_bit: self.force_mask_bit,
                    preserve_masked_pixels: self.preserve_masked_pixels,
                }));
            }

            self.commands_ready = true;
        }
        #[cfg(feature = "software_gpu")]
        {
            if vertices.len() == 4 {
                // split up into two triangles
                let vertices1 = vec![vertices[0], vertices[1], vertices[2]];

                let mut polygon = Polygon {
                    vertices: vertices1,
                    is_line: false,
                    texpage,
                    clut: (self.clut_x as u32, self.clut_y as u32),
                    semitransparent: self.is_semitransparent,
                    textured: self.is_textured,
                    transparent_mode: if let Some(texpage) = texpage {
                        texpage.semi_transparency as u32
                    } else {
                        self.texpage.semi_transparency as u32
                    },
                    is_shaded: self.is_shaded,
                    modulate: self.modulate,
                    texture_mask_x: self.texture_window_mask_x,
                    texture_mask_y: self.texture_window_mask_y,
                    texture_offset_x: self.texture_window_offset_x,
                    texture_offset_y: self.texture_window_offset_y,
                    ..Default::default()
                };

                self.rasterize_triangle(&mut polygon);

                let vertices2 = vec![vertices[1], vertices[2], vertices[3]];

                let mut polygon2 = Polygon {
                    vertices: vertices2,
                    is_line: false,
                    texpage,
                    clut: (self.clut_x as u32, self.clut_y as u32),
                    semitransparent: self.is_semitransparent,
                    textured: self.is_textured,
                    transparent_mode: if let Some(texpage) = texpage {
                        texpage.semi_transparency as u32
                    } else {
                        self.texpage.semi_transparency as u32
                    },
                    is_shaded: self.is_shaded,
                    modulate: self.modulate,
                    texture_mask_x: self.texture_window_mask_x,
                    texture_mask_y: self.texture_window_mask_y,
                    texture_offset_x: self.texture_window_offset_x,
                    texture_offset_y: self.texture_window_offset_y,
                    ..Default::default()
                };

                self.rasterize_triangle(&mut polygon2);
            } else {
                let mut polygon = Polygon {
                    vertices,
                    is_line: false,
                    texpage,
                    clut: (self.clut_x as u32, self.clut_y as u32),
                    semitransparent: self.is_semitransparent,
                    textured: self.is_textured,
                    transparent_mode: if let Some(texpage) = texpage {
                        texpage.semi_transparency as u32
                    } else {
                        self.texpage.semi_transparency as u32
                    },
                    is_shaded: self.is_shaded,
                    modulate: self.modulate,
                    texture_mask_x: self.texture_window_mask_x,
                    texture_mask_y: self.texture_window_mask_y,
                    texture_offset_x: self.texture_window_offset_x,
                    texture_offset_y: self.texture_window_offset_y,
                    ..Default::default()
                };

                self.rasterize_triangle(&mut polygon);
            }
        }

        self.num_vertices = 0;
    }

    fn parse_clut(word: u32) -> (usize, usize) {
        let x = (word & 0x3f) * 16;
        let y = (word >> 6) & 0x1ff;

        (x as usize, y as usize)
    }

    fn parse_position(&self, word: u32) -> (i32, i32) {
        let mut x = (word & 0xffff) as i32;
        let mut y = (word >> 16) as i32;

        x = (x << 21) >> 21;
        y = (y << 21) >> 21;

        x += self.x_offset;
        y += self.y_offset;

        (x, y)
    }

    fn push_rectangle(&mut self) {
        #[cfg(any(feature = "hardware_gpu", feature = "hardware_gpu_web"))]
        {
            self.commands_ready = true;
        }

        let word = self.current_command_buffer.pop_front().unwrap();

        let color = Self::parse_color(word);

        let word = self.current_command_buffer.pop_front().unwrap();

        let (x, y) = self.parse_position(word);

        let mut u = 0;
        let mut v = 0;

        if self.is_textured {
            let word = self.current_command_buffer.pop_front().unwrap();

            u = word & 0xff;
            v = (word >> 8) & 0xff;

            (self.clut_x, self.clut_y) = Self::parse_clut(word >> 16);
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
            RectangleSize::Single => (1, 1),
        };

        // calculate the other vertices and push this to polygons!

        let v0 = Vertex { x, y, u, v, color };

        let v1 = Vertex {
            x: x + width,
            y,
            u: u + width as u32,
            v,
            color,
        };

        let v2 = Vertex {
            x,
            y: y + height,
            u,
            v: v + height as u32,
            color,
        };

        let v3 = Vertex {
            x: x + width,
            y: y + height,
            u: u + width as u32,
            v: v + height as u32,
            color,
        };

        let vertices = [v0, v1, v2, v3];

        let vertices1 = vec![vertices[0], vertices[1], vertices[2]];
        let vertices2 = vec![vertices[1], vertices[2], vertices[3]];

        #[cfg(any(feature = "hardware_gpu", feature = "hardware_gpu_web"))]
        {
            self.gpu_commands.push(GPUCommand::RenderPolygon(Polygon {
                vertices: vertices1,
                is_line: false,
                texpage: if self.is_textured {
                    Some(self.texpage)
                } else {
                    None
                },
                clut: (self.clut_x as u32, self.clut_y as u32),
                semitransparent: self.is_semitransparent,
                transparent_mode: self.texpage.semi_transparency as u32,
                textured: self.is_textured,
                is_shaded: self.is_shaded,
                modulate: self.modulate,
                texture_mask_x: self.texture_window_mask_x,
                texture_mask_y: self.texture_window_mask_y,
                texture_offset_x: self.texture_window_offset_x,
                texture_offset_y: self.texture_window_offset_y,
                x1: self.x1,
                x2: self.x2,
                y1: self.y1,
                y2: self.y2,
                force_mask_bit: self.force_mask_bit,
                preserve_masked_pixels: self.preserve_masked_pixels,
            }));
            self.gpu_commands.push(GPUCommand::RenderPolygon(Polygon {
                vertices: vertices2,
                is_line: false,
                texpage: if self.is_textured {
                    Some(self.texpage)
                } else {
                    None
                },
                clut: (self.clut_x as u32, self.clut_y as u32),
                semitransparent: self.is_semitransparent,
                transparent_mode: self.texpage.semi_transparency as u32,
                textured: self.is_textured,
                is_shaded: self.is_shaded,
                modulate: self.modulate,
                texture_mask_x: self.texture_window_mask_x,
                texture_mask_y: self.texture_window_mask_y,
                texture_offset_x: self.texture_window_offset_x,
                texture_offset_y: self.texture_window_offset_y,
                x1: self.x1,
                x2: self.x2,
                y1: self.y1,
                y2: self.y2,
                force_mask_bit: self.force_mask_bit,
                preserve_masked_pixels: self.preserve_masked_pixels,
            }));
        }
        #[cfg(feature = "software_gpu")]
        {
            let mut polygon1 = Polygon {
                vertices: vertices1,
                is_line: false,
                texpage: if self.is_textured {
                    Some(self.texpage)
                } else {
                    None
                },
                clut: (self.clut_x as u32, self.clut_y as u32),
                semitransparent: self.is_semitransparent,
                transparent_mode: self.texpage.semi_transparency as u32,
                textured: self.is_textured,
                is_shaded: self.is_shaded,
                modulate: self.modulate,
                texture_mask_x: self.texture_window_mask_x,
                texture_mask_y: self.texture_window_mask_y,
                texture_offset_x: self.texture_window_offset_x,
                texture_offset_y: self.texture_window_offset_y,
                ..Default::default()
            };

            self.rasterize_triangle(&mut polygon1);

            let mut polygon2 = Polygon {
                vertices: vertices2,
                is_line: false,
                texpage: if self.is_textured {
                    Some(self.texpage)
                } else {
                    None
                },
                clut: (self.clut_x as u32, self.clut_y as u32),
                semitransparent: self.is_semitransparent,
                transparent_mode: self.texpage.semi_transparency as u32,
                textured: self.is_textured,
                is_shaded: self.is_shaded,
                modulate: self.modulate,
                texture_mask_x: self.texture_window_mask_x,
                texture_mask_y: self.texture_window_mask_y,
                texture_offset_x: self.texture_window_offset_x,
                texture_offset_y: self.texture_window_offset_y,
                ..Default::default()
            };

            self.rasterize_triangle(&mut polygon2);
        }

        self.num_vertices = 0;
    }

    fn set_drawing_offset(&mut self, word: u32) {
        self.x_offset = (((word & 0x7ff) as i32) << 21) >> 21;
        self.y_offset = ((((word >> 11) & 0x7ff) as i32) << 21) >> 21;
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
        self.texture_window_mask_x = (word & 0x1f) * 8;
        self.texture_window_mask_y = ((word >> 5) & 0x1f) * 8;
        self.texture_window_offset_x = ((word >> 10) & 0x1f) * 8;
        self.texture_window_offset_y = ((word >> 15) & 0x1f) * 8;
    }

    fn mask_bit(&mut self, word: u32) {
        self.force_mask_bit = word & 1 == 1;
        self.preserve_masked_pixels = (word >> 1) & 1 == 1;
    }

    fn vram_to_cpu_transfer(&mut self) {
        self.transfer_type = Some(TransferType::FromVram);

        self.current_command_buffer.pop_front().unwrap();

        let source = self.current_command_buffer.pop_front().unwrap();
        let dimensions = self.current_command_buffer.pop_front().unwrap();

        let start_x = source & 0x3ff;
        let start_y = (source >> 16) & 0x1ff;

        let width = dimensions & 0x3ff;
        let height = (dimensions >> 16) & 0x1ff;

        #[cfg(any(feature = "hardware_gpu", feature = "hardware_gpu_web"))]
        {
            self.gpu_commands
                .push(GPUCommand::VRAMtoCPU(CPUTransferParams {
                    start_x,
                    start_y,
                    width,
                    height,
                }));
            self.commands_ready = true;
        }
        #[cfg(feature = "software_gpu")]
        {
            self.cpu_transfer_x = start_x;
            self.cpu_transfer_y = start_y;

            self.cpu_transfer_width = width;
            self.cpu_transfer_height = height;
        }

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
        self.transfer_y = (destination >> 16) & 0x3ff;

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
        self.current_command_buffer.pop_front().unwrap();

        let source = self.current_command_buffer.pop_front().unwrap();
        let destination = self.current_command_buffer.pop_front().unwrap();
        let dimensions = self.current_command_buffer.pop_front().unwrap();

        let source_start_x = source & 0x3ff;
        let source_start_y = (source >> 16) & 0x1ff;

        let destination_start_x = destination & 0x3ff;
        let destination_start_y = (destination >> 16) & 0x1ff;

        let width = dimensions & 0x3ff;
        let height = (dimensions >> 16) & 0x1ff;

        #[cfg(any(feature = "hardware_gpu", feature = "hardware_gpu_web"))]
        {
            self.gpu_commands
                .push(GPUCommand::VramToVram(VramToVramTransferParams {
                    source_start_x,
                    source_start_y,
                    destination_start_x,
                    destination_start_y,
                    width,
                    height,
                }));

            self.commands_ready = true;
        }
        #[cfg(feature = "software_gpu")]
        {
            for y in 0..height {
                for x in 0..width {
                    let source_x = x + source_start_x;
                    let dest_x = x + destination_start_x;

                    let source_y = y + source_start_y;
                    let dest_y = y + destination_start_y;

                    let source_vram_address =
                        GPU::get_vram_address(source_x & 0x3ff, source_y & 0x1ff);
                    let destination_vram_address =
                        GPU::get_vram_address(dest_x & 0x3ff, dest_y & 0x1ff);

                    self.vram[destination_vram_address] = self.vram[source_vram_address];
                    self.vram[destination_vram_address + 1] = self.vram[source_vram_address + 1];
                }
            }
        }
    }

    pub fn cross_product(v: &[Vertex]) -> i32 {
        (v[1].x - v[0].x) * (v[2].y - v[0].y) - (v[1].y - v[0].y) * (v[2].x - v[0].x)
    }

    fn fill_vram(&mut self) {
        let color = self.current_command_buffer.pop_front().unwrap();

        let mut r = color & 0xff;
        let mut g = (color >> 8) & 0xff;
        let mut b = (color >> 16) & 0xff;

        r >>= 3;
        g >>= 3;
        b >>= 3;

        let pixel = r as u16 | (g as u16) << 5 | (b as u16) << 10;

        let destination = self.current_command_buffer.pop_front().unwrap();
        let dimensions = self.current_command_buffer.pop_front().unwrap();

        let start_x = destination & 0x3f0;
        let start_y = (destination >> 16) & 0x3ff;

        let w = ((dimensions & 0x3ff) + 0xf) & !0xf;
        let h = (dimensions >> 16) & 0x1ff;

        #[cfg(any(feature = "hardware_gpu", feature = "hardware_gpu_web"))]
        {
            let fill_vram_params = FillVramParams {
                start_x,
                start_y,
                width: w,
                height: h,
                pixel,
            };

            self.gpu_commands
                .push(GPUCommand::FillVRAM(fill_vram_params));
            self.commands_ready = true;
        }
        #[cfg(feature = "software_gpu")]
        {
            for y in start_y..start_y + h {
                for x in start_x..start_x + w {
                    let vram_address = GPU::get_vram_address(x & 0x3ff, y & 0x1ff);

                    unsafe { *(&mut self.vram[vram_address] as *mut u8 as *mut u16) = pixel };
                }
            }
        }
    }

    fn execute_command(&mut self, word: u32) {
        let command = word >> 24;
        let upper = word >> 29;

        match upper {
            1 => self.push_polygon(),
            2 => self.draw_line(),
            3 => self.push_rectangle(),
            4 => self.vram_to_vram_transfer(),
            5 => self.cpu_to_vram_transfer(),
            6 => self.vram_to_cpu_transfer(),
            _ => {
                match command {
                    0x0 => (), // NOP
                    0x2 => self.fill_vram(),
                    0x1 => (),        // TODO: invalidate cache
                    0x3..=0x1e => (), // NOP
                    0xe1 => self.texpage(word),
                    0xe2 => self.texture_window(word),
                    0xe3 => self.set_drawing_area(word, false),
                    0xe4 => self.set_drawing_area(word, true),
                    0xe5 => self.set_drawing_offset(word),
                    0xe6 => self.mask_bit(word),
                    0xe7..=0xff => (), // NOP
                    _ => todo!("command: 0x{:x}", command),
                }
            }
        }
    }

    fn draw_polyline(&mut self) {
        let color0 = if let Some(color0) = self.previous_line_color {
            color0
        } else {
            Self::parse_color(self.current_command_buffer.pop_front().unwrap())
        };

        let vertex0 = if let Some(vertex0) = self.previous_line_vertex {
            vertex0
        } else {
            let word = self.current_command_buffer.pop_front().unwrap();

            let (x, y) = self.parse_position(word);

            Vertex {
                x,
                y,
                u: 0,
                v: 0,
                color: color0,
            }
        };

        let mut color1 = color0;

        if self.is_shaded {
            color1 = Self::parse_color(self.current_command_buffer.pop_front().unwrap());
        }

        let word = self.current_command_buffer.pop_front().unwrap();

        let (x, y) = self.parse_position(word);

        let vertex1 = Vertex {
            x,
            y,
            u: 0,
            v: 0,
            color: color1,
        };

        #[cfg(feature = "software_gpu")]
        {
            let polygon = Polygon {
                vertices: vec![vertex0, vertex1],
                is_line: true,
                is_shaded: self.is_shaded,
                semitransparent: self.is_semitransparent,
                textured: false,
                texpage: None,
                modulate: false,
                transparent_mode: self.texpage.semi_transparency as u32,
                clut: (0, 0),
                ..Default::default()
            };

            self.rasterize_line(&polygon);
        }
        #[cfg(any(feature = "hardware_gpu", feature = "hardware_gpu_web"))]
        {
            self.push_line(vertex0, vertex1);
        }

        self.previous_line_color = Some(color1);
        self.previous_line_vertex = Some(vertex1);
    }

    fn draw_line(&mut self) {
        let color0 = Self::parse_color(self.current_command_buffer.pop_front().unwrap());

        let word = self.current_command_buffer.pop_front().unwrap();

        let (x0, y0) = self.parse_position(word);

        let mut color1 = color0;

        if self.is_shaded {
            color1 = Self::parse_color(self.current_command_buffer.pop_front().unwrap());
        }

        let word = self.current_command_buffer.pop_front().unwrap();

        let (x1, y1) = self.parse_position(word);

        let vertex0 = Vertex {
            x: x0,
            y: y0,
            u: 0,
            v: 0,
            color: color0,
        };
        let vertex1 = Vertex {
            x: x1,
            y: y1,
            u: 0,
            v: 0,
            color: color1,
        };

        #[cfg(feature = "software_gpu")]
        {
            let polygon = Polygon {
                vertices: vec![vertex0, vertex1],
                is_line: true,
                is_shaded: self.is_shaded,
                semitransparent: false,
                textured: false,
                texpage: None,
                modulate: false,
                transparent_mode: self.texpage.semi_transparency as u32,
                clut: (0, 0),
                ..Default::default()
            };

            self.rasterize_line(&polygon);
        }

        #[cfg(any(feature = "hardware_gpu", feature = "hardware_gpu_web"))]
        {
            self.push_line(vertex0, vertex1);
        }
    }

    #[cfg(any(feature = "hardware_gpu", feature = "hardware_gpu_web"))]
    fn push_line(&mut self, vertex0: Vertex, vertex1: Vertex) {
        self.gpu_commands.push(GPUCommand::RenderPolygon(Polygon {
            vertices: vec![vertex0, vertex1],
            is_line: true,
            is_shaded: self.is_shaded,
            semitransparent: self.is_semitransparent,
            textured: false,
            texpage: None,
            modulate: false,
            transparent_mode: self.texpage.semi_transparency as u32,
            clut: (0, 0),
            x1: self.x1,
            x2: self.x2,
            y1: self.y1,
            y2: self.y2,
            force_mask_bit: self.force_mask_bit,
            preserve_masked_pixels: self.preserve_masked_pixels,
            ..Default::default()
        }));

        self.commands_ready = true;
    }

    fn process_polyline(&mut self, word: u32) {
        if (word & 0xf000f000) == 0x50005000 {
            self.is_polyline = false;
            self.words_left = 0;
            self.previous_line_color = None;
            self.previous_line_vertex = None;

            return;
        }

        self.current_command_buffer.push_back(word);
        self.words_left -= 1;

        if self.words_left == 0 {
            self.draw_polyline();
            self.words_left = if self.is_shaded { 2 } else { 1 };
        }
    }

    fn transfer_to_vram(&mut self, halfword: u16) {
        #[cfg(feature = "software_gpu")]
        let curr_x = self.read_x + self.transfer_x;
        #[cfg(feature = "software_gpu")]
        let curr_y = self.read_y + self.transfer_y;

        self.read_x += 1;

        #[cfg(any(feature = "hardware_gpu", feature = "hardware_gpu_web"))]
        self.vram_transfer_halfwords.push(halfword);

        if self.read_x == self.transfer_width {
            self.read_x = 0;

            self.read_y += 1;

            if self.read_y == self.transfer_height {
                self.transfer_type = None;

                #[cfg(any(feature = "hardware_gpu", feature = "hardware_gpu_web"))]
                {
                    self.gpu_commands
                        .push(GPUCommand::CPUtoVram(VRamTransferParams {
                            halfwords: self.vram_transfer_halfwords.drain(..).collect(),
                            start_x: self.transfer_x,
                            start_y: self.transfer_y,
                            width: self.transfer_width,
                            height: self.transfer_height,
                        }));
                    self.commands_ready = true;
                }

                self.read_y = 0;
                self.transfer_width = 0;
                self.transfer_height = 0;
                self.transfer_x = 0;
                self.transfer_y = 0;
            }
        }
        #[cfg(feature = "software_gpu")]
        {
            let vram_address = GPU::get_vram_address(curr_x & 0x3ff, curr_y & 0x1ff);

            unsafe { *(&mut self.vram[vram_address] as *mut u8 as *mut u16) = halfword };
        }
    }

    #[cfg(feature = "software_gpu")]
    fn get_vram_address(x: u32, y: u32) -> usize {
        (2 * (x + 1024 * y)) as usize
    }

    pub fn process_gp1_commands(&mut self, word: u32) {
        let command = word >> 24;
        match command {
            0x0 => self.reset_gpu(word),
            0x1 => self.current_command_buffer.clear(),
            0x2 => self.irq_enabled = false,
            0x3 => self.display_on = word & 1 == 1,
            0x4 => {
                self.dma_direction = match word & 0x3 {
                    0 => DmaDirection::Off,
                    1 => DmaDirection::Fifo,
                    2 => DmaDirection::ToGP0,
                    3 => DmaDirection::ToCPU,
                    _ => unreachable!(),
                }
            }
            0x5 => self.display_area_start(word),
            0x6 => self.display_range_horizontal(word),
            0x7 => self.display_range_vertical(word),
            0x8 => self.display_mode(word),
            0x10..=0x1f => self.read_internal_register(word),
            _ => todo!("gp1 0x{:x}", command),
        }
    }

    fn display_range_horizontal(&mut self, word: u32) {
        self.display_range_x = (word & 0xfff, (word >> 12) & 0xfff);
    }

    fn read_internal_register(&mut self, word: u32) {
        match word & 0x7 {
            0x2 => {
                self.gpuread = self.texture_window_mask_x
                    | (self.texture_window_mask_y) << 5
                    | (self.texture_window_offset_x) << 10
                    | (self.texture_window_offset_y) << 15
            }
            0x3 => self.gpuread = self.x1 | self.y1 << 10,
            0x4 => self.gpuread = self.x2 | self.y2 << 10,
            0x5 => self.gpuread = self.x_offset as u32 | (self.y_offset as u32) << 11,
            _ => (),
        }
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
        let old_display_width = self.display_width;
        let old_display_height = self.display_height;
        self.display_width = if (word >> 6) & 0x1 == 1 {
            368
        } else {
            match word & 0x3 {
                0 => 256,
                1 => 320,
                2 => 512,
                3 => 640,
                _ => unreachable!(),
            }
        };

        self.horizontal_bits1 = word & 0x3;
        self.horizontal_bit2 = (word >> 6) & 0x1;

        self.interlaced = (word >> 5) & 0x1 == 1;

        self.display_height = match (word >> 2) & 0x1 {
            0 => 240,
            1 => 480,
            _ => unreachable!(),
        };

        if !self.interlaced {
            self.display_height = 240;
        }

        if old_display_width != self.display_width || old_display_height != self.display_height {
            self.resolution_changed = true;
        }

        self.video_mode = match (word >> 3) & 0x1 {
            0 => DisplayMode::Ntsc,
            1 => DisplayMode::Pal,
            _ => unreachable!(),
        };

        self.display_depth = match (word >> 4) & 0x1 {
            0 => DisplayDepth::Bit15,
            1 => DisplayDepth::Bit24,
            _ => unreachable!(),
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
        if let Some(transfer_type) = self.transfer_type {
            if transfer_type == TransferType::ToVram {
                self.transfer_to_vram(word as u16);

                if self.transfer_type.is_some() {
                    self.transfer_to_vram((word >> 16) as u16);
                }
                return;
            }
        }

        if self.is_polyline {
            self.process_polyline(word);

            return;
        }

        self.current_command_buffer.push_back(word);

        if self.words_left == 0 {
            self.words_left = self.get_words_left(word);
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
            _ => 0,
        };

        let bit31 = if self.current_line < VBLANK_LINE_START {
            self.even_flag
        } else {
            0
        };

        self.even_flag << 31 |
            self.texpage.x_base |
            self.texpage.y_base1 << 4 |
            (self.texpage.semi_transparency as u32) << 5 |
            (self.texpage.texture_page_colors as u32) << 7 |
            (self.texpage.dither as u32) << 9 |
            (self.texpage.draw_to_display_area as u32) << 10 |
            (self.force_mask_bit as u32) << 11 |
            (self.preserve_masked_pixels as u32) << 12 |
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
            bit31 << 31
    }

    pub fn cap_fps(&mut self) {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("an error occurred")
            .as_millis();

        if self.previous_time != 0 {
            let diff = current_time - self.previous_time;

            // if self.debug_on {
            //     println!("fps = {}", 1000 / diff);
            // }

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
        timers: &mut [Timer],
        cycles_left: usize,
    ) {
        timers[1].in_xblank = true;

        if timers[1]
            .counter_register
            .contains(CounterModeRegister::SYNC_ENABLE)
        {
            timers[1].handle_xblank_sync();
        }

        if self.current_line == NUM_SCANLINES {
            self.current_line = 0;
            scheduler.schedule(
                EventType::HblankStart,
                (HBLANK_START as f64 * (GPU_CYCLES_TO_CPU_CYCLES)) as usize - cycles_left,
            );
        } else {
            scheduler.schedule(
                EventType::Vblank,
                (CYCLES_PER_SCANLINE as f64 * (GPU_CYCLES_TO_CPU_CYCLES)) as usize - cycles_left,
            );
            self.current_line += 1;
        }
    }
}
