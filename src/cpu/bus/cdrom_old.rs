use std::{rc::Rc, cell::Cell, collections::VecDeque, fs::{File, self}, io::{SeekFrom, Read, Seek}};

use crate::{cpu::interrupt::{interrupt_registers::InterruptRegisters, interrupt_register::Interrupt}, spu::{SPU, voices::{POS_ADPCM_TABLE, NEG_ADPCM_TABLE}}};

const CDROM_CYCLES: i32 = 768;
pub const SECTORS_PER_SECOND: u64 = 75;
pub const SECTORS_PER_MINUTE: u64 = 60 * SECTORS_PER_SECOND;
pub const BYTES_PER_SECTOR: u64 = 2352;
pub const LEAD_IN_SECTORS: u64 = 2 * SECTORS_PER_SECOND;

const HEADER_START: usize = 12;
const SUBHEADER_START: usize = 16;

const DATA_OFFSET: usize = 24;
const ADDR_OFFSET: usize = 12;

// per https://psx-spx.consoledev.net/cdromdrive/#25-point-zigzag-interpolation
pub const ZIGZAG_INTERPOLATION_TABLE: [[i32; 29]; 7] = [
  [0, 0, 0, 0, 0, -0x2, 0xa, -0x22, 0x41, -0x54, 0x34, 0x9, -0x10a, 0x400, -0xa78, 0x234c, 0x6794, -0x1780, 0xbcd, -0x623, 0x350, -0x16d, 0x6b, 0xa, -0x10, 0x11, -0x8, 0x3, -0x1],
  [0, 0, 0, -0x2, 0, 0x3, -0x13, 0x3c, -0x4b, 0xa2, -0xe3, 0x132, -0x43, -0x267, 0xc9d, 0x74bb, -0x11b4, 0x9b8, -0x5bf, 0x372, -0x1a8, 0xa6, -0x1b, 0x5, 0x6, -0x8, 0x3, -0x1, 0],
  [0, 0, -0x1, 0x3, -0x2, -0x5, 0x1f, -0x4a, 0xb3, -0x192, 0x2b1, -0x39e, 0x4f8, -0x5a6, 0x7939, -0x5a6, 0x4f8, -0x39e, 0x2b1, -0x192, 0xb3, -0x4a, 0x1f, -0x5, -0x2, 0x3, -0x1, 0, 0],
  [0, -0x1, 0x3, -0x8, 0x6, 0x5, -0x1b, 0xa6, -0x1a8, 0x372, -0x5bf, 0x9b8, -0x11b4, 0x74bb, 0xc9d, -0x267, -0x43, 0x132, -0xe3, 0xa2, -0x4b, 0x3c, -0x13, 0x3, 0, -0x2, 0, 0, 0],
  [-0x1, 0x3, -0x8, 0x11, -0x10, 0xa, 0x6b, -0x16d, 0x350, -0x623, 0xbcd, -0x1780, 0x6794, 0x234c, -0xa78, 0x400, -0x10a, 0x9, 0x34, -0x54, 0x41, -0x22, 0xa, -0x1, 0, 0x1, 0, 0, 0],
  [0x2, -0x8, 0x10, -0x23, 0x2b, 0x1a, -0xeb, 0x27b, -0x548, 0xafa, -0x16fa, 0x53e0, 0x3c07, -0x1249, 0x80e, -0x347, 0x15b, -0x44, -0x17, 0x46, -0x23, 0x11, -0x5, 0, 0, 0, 0, 0, 0],
  [-0x5, 0x11, -0x23, 0x46, -0x17, -0x44, 0x15b, -0x347, 0x80e, -0x1249, 0x3c07, 0x53e0, -0x16fa, 0xafa, -0x548, 0x27b, -0xeb, 0x1a, 0x2b, -0x23, 0x10, -0x8, 0x2, 0, 0, 0, 0, 0, 0],
];


pub struct SubchannelQ {
  pub track: u8,
  pub index: u8,
  pub mm: u8,
  pub ss: u8,
  pub sect: u8,
  pub amm: u8,
  pub ass: u8,
  pub asect: u8
}

impl SubchannelQ {
  pub fn new() -> Self {
    Self {
      track: 0,
      index: 0,
      mm: 0,
      ss: 0,
      sect: 0,
      amm: 0,
      ass: 0,
      asect: 0
    }
  }
}

#[derive(PartialEq)]
pub enum CdSubheaderMode {
  Video,
  Audio,
  Data,
  Error
}


struct CdHeader {
  mm: u8,
  ss: u8,
  sect: u8,
  mode: u8
}

impl CdHeader {
  pub fn new(buf: &mut [u8]) -> Self {
    let offset = HEADER_START;
    Self {
      mm: Cdrom::bcd_to_u8(buf[offset]),
      ss: Cdrom::bcd_to_u8(buf[offset + 1]),
      sect: Cdrom::bcd_to_u8(buf[offset + 2]),
      mode: buf[offset + 3]
    }
  }
}

#[derive(Copy, Clone)]
struct CdSubheader {
  file: u8,
  channel: u8,
  coding_info: u8,
  sub_mode: u8
}

impl CdSubheader {
  pub fn new(buf: &mut [u8]) -> Self {
    let offset = SUBHEADER_START;
    Self {
      file: buf[offset],
      channel: buf[offset + 1],
      sub_mode: buf[offset + 2],
      coding_info: buf[offset + 3]
    }
  }

  pub fn realtime(&self) -> bool {
    (self.sub_mode >> 6) & 0b1 == 1
  }

  pub fn sample_rate(&self) -> usize {
    match (self.coding_info >> 2) & 0x3 {
      0 => 37800,
      1 => 18900,
      n => panic!("reserved value for sample rate given: {n}")
    }
  }

  pub fn bits_per_sample(&self) -> usize {
    match (self.coding_info >> 4) & 0x3  {
      0 => 4,
      1 => 8,
      n => panic!("reserved value given for bits per sample: {n}")
    }
  }

  pub fn channels(&self) -> usize {
    match self.coding_info & 0x3 {
      0 => 1,
      1 => 2,
      _ => panic!("invalid value specified for channels: {}", self.coding_info)
    }
  }

  pub fn mode(&self) -> CdSubheaderMode {
    match self.sub_mode & 0xe {
      2 => CdSubheaderMode::Video,
      4 => CdSubheaderMode::Audio,
      0 | 8 => CdSubheaderMode::Data,
      _ => CdSubheaderMode::Error
    }
  }
}

#[derive(PartialEq)]
pub enum SubResponse {
  Disabled,
  GetID,
  GetStat
}

#[derive(PartialEq, Debug)]
pub enum ControllerMode {
  Idle,
  ParamTransfer,
  CommandTransfer,
  CommandExecute,
  ResponseClear,
  ResponseTransfer,
  InterruptTransfer
}

#[derive(PartialEq, Clone, Copy)]
pub enum DriveMode {
  Idle,
  Seek,
  Read,
  Play,
  GetStat
}

#[derive(PartialEq)]
pub enum CdReadMode {
  Data,
  Audio,
  Skip
}


pub struct Cdrom {
  interrupts: Rc<Cell<InterruptRegisters>>,
  pub index: u8,
  interrupt_enable: u8,
  interrupt_flags: u8,
  param_buffer: VecDeque<u8>,
  response_buffer: VecDeque<u8>,
  controller_response_buffer: VecDeque<u8>,
  command: Option<u8>,
  current_command: u8,
  cycles: i32,
  controller_cycles: i32,
  drive_cycles: i32,
  controller_mode: ControllerMode,
  controller_param_buffer: VecDeque<u8>,
  controller_interrupt_flags: u8,
  subresponse: SubResponse,
  subresponse_cycles: i32,
  ss: u8,
  mm: u8,
  sect: u8,
  current_ss: u8,
  current_mm: u8,
  current_sect: u8,
  drive_mode: DriveMode,
  next_drive_mode: DriveMode,
  double_speed: bool,
  processing_seek: bool,
  send_adpcm_sectors: bool,
  report_interrupts: bool,
  xa_filter: bool,
  sector_size: bool,
  game_bytes: Option<Vec<u8>>,
  game_file: Option<File>,
  sector_header: CdHeader,
  sector_subheader: CdSubheader,
  sector_buffer: Vec<u8>,

  drive_interrupt_pending: bool,
  pending_stat: u8,

  data_buffer: Vec<u8>,
  data_buffer_pointer: usize,

  is_playing: bool,
  is_seeking: bool,
  is_reading: bool,

  filter_channel: u8,
  filter_file: u8,

  previous_samples: [[i16; 2]; 2],
  sample_buffer: [Vec<i16>; 2],

  sixstep: usize,
  ringbuf: [[i16; 0x20]; 2],
  subq: SubchannelQ,

  file_pointer: usize,
  pub debug_on: bool
}

impl Cdrom {
  pub fn new(interrupts: Rc<Cell<InterruptRegisters>>, game_file: Option<File>, game_bytes: Option<Vec<u8>>, is_wasm: bool) -> Self {
    Self {
      interrupts,
      index: 0,
      interrupt_enable: 0,
      interrupt_flags: 0,
      param_buffer: VecDeque::with_capacity(16),
      response_buffer: VecDeque::with_capacity(16),
      controller_param_buffer: VecDeque::with_capacity(16),
      controller_response_buffer: VecDeque::with_capacity(16),
      command: None,
      current_command: 0,
      cycles: 0,
      controller_cycles: 0,
      subresponse_cycles: 0,
      drive_cycles: 0,
      controller_mode: ControllerMode::Idle,
      subresponse: SubResponse::Disabled,
      drive_mode: DriveMode::Idle,
      next_drive_mode: DriveMode::Idle,
      controller_interrupt_flags: 0,
      ss: 0,
      mm: 0,
      sect: 0,
      current_ss: 0,
      current_mm: 0,
      current_sect: 0,
      double_speed: false,
      processing_seek: false,
      send_adpcm_sectors: false,
      report_interrupts: false,
      xa_filter: false,
      sector_size: false,
      sector_header: CdHeader {
        mm: 0,
        ss: 0,
        sect: 0,
        mode: 0
      },
      sector_subheader: CdSubheader {
        file: 0,
        channel: 0,
        coding_info: 0,
        sub_mode: 0
      },
      sector_buffer: vec![0; 0x930],
      drive_interrupt_pending: false,
      pending_stat: 0,
      data_buffer: vec![0; 0x930],
      data_buffer_pointer: 0,
      is_playing: false,
      is_seeking: false,
      is_reading: false,
      filter_channel: 0,
      filter_file: 0,
      previous_samples: [[0; 2]; 2],
      sample_buffer: [
        Vec::new(),
        Vec::new()
      ],
      sixstep: 6,
      ringbuf: [[0; 0x20]; 2],
      subq: SubchannelQ::new(),
      file_pointer: 0,
      game_bytes,
      game_file,
      debug_on: false
    }
  }

  pub fn tick_counter(&mut self, cycles: i32, spu: &mut SPU) {
    self.cycles += cycles;

    if self.cycles >= CDROM_CYCLES {
      if self.debug_on {
        println!("[CDROM]cycles = {}", self.cycles);
      }
      let cd_cycles = self.cycles / CDROM_CYCLES;
      self.cycles %= CDROM_CYCLES;

      if self.debug_on {
        println!("cd cycles = {cd_cycles}");
      }

      self.tick(cd_cycles, spu);
    }
  }

  fn tick(&mut self, cycles: i32, spu: &mut SPU) {
    self.tick_subresponse(cycles);
    self.tick_drive(cycles, spu);
    self.tick_controller(cycles);

    if self.debug_on {
      println!("[CDROM]irqs: {:x}, enable_irq: {:x}", self.interrupt_flags, self.interrupt_enable);
    }

    if (self.interrupt_enable & self.interrupt_flags & 0x1f) != 0 {
      if self.debug_on {
        println!("[CDROM]triggering irq, irqs = 0x{:x}, interrupt_enable = 0x{:x}", self.interrupt_flags, self.interrupt_enable);
      }
      let mut interrupts = self.interrupts.get();
      interrupts.status.set_interrupt(Interrupt::Cdrom);
      self.interrupts.set(interrupts);
    }
  }

  pub fn subresponse_get_id(&mut self) {
    // per https://psx-spx.consoledev.net/cdromdrive/#getid-command-1ah-int3stat-int25-statflagstypeatipscex
    /*
      1st byte: stat  (as usually, but with bit3 same as bit7 in 2nd byte)
      2nd byte: flags (bit7=denied, bit4=audio... or reportedly import, uh?)
        bit7: Licensed (0=Licensed Data CD, 1=Denied Data CD or Audio CD)
        bit6: Missing  (0=Disk Present, 1=Disk Missing)
        bit4: Audio CD (0=Data CD, 1=Audio CD) (always 0 when Modchip installed)
      3rd byte: Disk type (from TOC Point=A0h) (eg. 00h=Audio or Mode1, 20h=Mode2)
      4th byte: Usually 00h (or 8bit ATIP from Point=C0h, if session info exists)
        that 8bit ATIP value is taken form the middle 8bit of the 24bit ATIP value
      5th-8th byte: SCEx region (eg. ASCII "SCEE" = Europe) (0,0,0,0 = Unlicensed)
      */

    if self.interrupt_flags == 0 {
      self.controller_response_buffer.push_back(0x2);
      self.controller_response_buffer.push_back(0x0);
      self.controller_response_buffer.push_back(0x20);
      self.controller_response_buffer.push_back(0x0);
      self.controller_response_buffer.push_back('S' as u8);
      self.controller_response_buffer.push_back('C' as u8);
      self.controller_response_buffer.push_back('E' as u8);
      self.controller_response_buffer.push_back('A' as u8);

      self.controller_mode = ControllerMode::ResponseClear;
      self.controller_interrupt_flags = 0x2;

      self.controller_cycles += 10;

      self.subresponse = SubResponse::Disabled;
    }

    self.subresponse_cycles += 1;
  }

  fn subresponse_get_stat(&mut self) {
    if self.interrupt_flags == 0 {
      self.push_stat();

      self.controller_mode = ControllerMode::ResponseClear;

      self.controller_interrupt_flags = 0x2;

      self.controller_cycles += 10;

      self.subresponse = SubResponse::Disabled;
    }

    self.subresponse_cycles += 1;
  }

  fn tick_subresponse(&mut self, cycles: i32) {
    self.subresponse_cycles -= cycles;

    if self.subresponse_cycles <= 0 {
      match self.subresponse {
        SubResponse::Disabled => self.subresponse_cycles += cycles,
        SubResponse::GetID => self.subresponse_get_id(),
        SubResponse::GetStat => self.subresponse_get_stat()
      }
    }
  }

  fn seek_drive(&mut self) {
    self.processing_seek = false;

    self.current_mm = self.mm;
    self.current_ss = self.ss;
    self.current_sect = self.sect;

    self.subq.track = 1;
    self.subq.index = 1;

    self.subq.mm = self.mm;
    self.subq.ss = self.ss - 2;
    self.subq.sect = self.sect;

    self.subq.amm = self.mm;
    self.subq.ass = self.ss;
    self.subq.asect = self.sect;

    self.is_playing = false;
    self.is_seeking = false;
    self.is_reading = false;

    match self.next_drive_mode {
      DriveMode::Read => {
        let divisor = if self.double_speed { 150 } else { 75 };

        self.drive_cycles += 44100 / divisor;

        self.is_reading = true;
      }
      DriveMode::Play => {
        let divisor = if self.double_speed { 150 } else { 75 };

        self.drive_cycles += 44100 / divisor;

        self.is_playing = true;
      }
      _ => self.drive_cycles += 10
    }

    self.drive_mode = self.next_drive_mode;
  }

  fn play_drive(&mut self) {
    todo!("play_drive not implemented");
  }

  fn read_drive(&mut self, spu: &mut SPU) {
    if !self.is_reading {
      self.drive_mode = DriveMode::Idle;
      self.drive_cycles += 1;

      if self.debug_on {
        println!("[CDROM]no longer reading!");
      }

      return;
    }
    self.push_stat();

    self.file_pointer = self.get_seek_pointer() as usize;

    let mut buf: Vec<u8> = vec![0; 24];

    if let Some(game_file) = &mut self.game_file {
      game_file.seek(SeekFrom::Start(self.file_pointer as u64)).unwrap();
      game_file.read_exact(&mut buf).unwrap();
    } else if let Some(game_bytes) = &self.game_bytes {
      let end_length = self.file_pointer + 24;

      buf = game_bytes[self.file_pointer..end_length].to_vec();
    }

    let header = CdHeader::new(&mut buf);
    let subheader = CdSubheader::new(&mut buf);

    self.subq.track = 1;
    self.subq.index = 1;

    self.subq.mm = header.mm;
    self.subq.ss = header.ss - 2;
    self.subq.sect = header.sect;

    self.subq.amm = header.mm;
    self.subq.ass = header.ss;
    self.subq.asect = header.sect;

    if header.mm != self.current_mm || header.ss != self.current_ss || header.sect != self.current_sect {
      panic!("mismatched sector info between header and controller");
    }

    if header.mode != 2 {
      panic!("unsupported mode found: {}", header.mode);
    }

    self.sector_header = header;
    self.sector_subheader = subheader;

    self.current_sect += 1;

    if self.current_sect >= 75 {
      self.current_sect = 0;
      self.current_ss += 1;

      if self.current_ss >= 60 {
        self.current_ss = 0;
        self.current_mm += 1;
      }
    }

    // TODO: see if subheader.realtime() is needed here
    let mut mode = if subheader.mode() == CdSubheaderMode::Audio && self.send_adpcm_sectors && subheader.realtime() {
      CdReadMode::Audio
    } else {
      CdReadMode::Data
    };

    if mode == CdReadMode::Audio && self.xa_filter && (subheader.file != self.filter_file || subheader.channel != self.filter_channel) {
      mode = CdReadMode::Skip
    }


    match mode {
      CdReadMode::Audio => self.read_audio(spu),
      CdReadMode::Data => self.read_data(),
      CdReadMode::Skip => ()
    }

    let divisor = if self.double_speed { 150 } else { 75 };
    self.drive_cycles += 44100 / divisor;
  }

  fn read_audio(&mut self, spu: &mut SPU) {
    if self.sector_subheader.bits_per_sample() != 4 {
      todo!("unimplemented bits per sample given")
    }

    let mut buffer: Vec<u8> = vec![0; 0x914];

    if self.debug_on {
      println!("[CDROM]reading audio sector");
    }

    if let Some(game_file) = &mut self.game_file {
      game_file.read_exact(&mut buffer).unwrap();
    } else if let Some(game_bytes) = &self.game_bytes {
      // accomodate for having reading the header data
      self.file_pointer += 24;

      let end_length = self.file_pointer + 0x914;

      buffer = game_bytes[self.file_pointer..end_length].to_vec();
    }

    let channels = self.sector_subheader.channels();

    // per docs, "Each sector consists of 12h 128-byte portions (=900h bytes)
    // (the remaining 14h bytes of the sectors 914h-byte data region are 00h filled)."
    for i in 0..0x12 {
      self.decode_blocks(&buffer[i * 128..], channels);
    }

    let repeat = match self.sector_subheader.sample_rate() {
      18900 => 2,
      37800 => 1,
      _ => unreachable!()
    };

    for channel in 0..channels {
      for _ in 0..repeat {
        for i in 0..self.sample_buffer[channel].len() {
          self.ringbuf[channel][i & 0x1f] = self.sample_buffer[channel][i];

          self.sixstep -= 1;

          if self.sixstep == 0 {
            self.sixstep = 6;

            for j in 0..7 {
              let sample = self.zigzag_interpolate(self.ringbuf[channel], ZIGZAG_INTERPOLATION_TABLE[j], i+1);

              if channels == 1 {
                spu.cd_left_buffer.push_back(sample);
                spu.cd_right_buffer.push_back(sample);
              } else {
                if channel == 0 {
                  spu.cd_left_buffer.push_back(sample)
                } else {
                  spu.cd_right_buffer.push_back(sample);
                }
              }
            }
          }
        }
      }
    }

    self.sample_buffer[0].clear();
    self.sample_buffer[1].clear();
  }

  fn zigzag_interpolate(&mut self, buffer: [i16; 32], table: [i32; 29], index: usize) -> i16 {
    // https://psx-spx.consoledev.net/cdromdrive/#25-point-zigzag-interpolation
    let mut sum = 0;
    for i in 1..30 {
      sum += ((buffer[(index - i) & 0x1f] as i32) * table[i - 1]) / 0x8000;
    }

    if sum < -0x8000 {
      sum = -0x8000
    } else if sum > 0x7fff {
      sum = 0x7fff
    }

    sum as i16
  }

  fn decode_blocks(&mut self, buffer: &[u8], channels: usize) {
    for i in 0..8 {
      let channel = if channels > 1 { i & 0b1 } else { 0 };

      self.decode_sample_block(buffer, i, channel)
    }
  }

  fn decode_sample_block(&mut self, buffer: &[u8], block: usize, channel: usize) {
    // per docs, "The separate 128-byte portions consist of a 16-byte header,
    // followed by twentyeight data words (4x28-bytes),"
    //  00h..03h  Copy of below 4 bytes (at 04h..07h)
    let header = buffer[0x4 + block];

    let mut shift = header & 0xf;
    let filter = ((header >> 4) & 0x3) as usize;

    if shift > 12 {
      shift = 9;
    }

    let f0 = POS_ADPCM_TABLE[filter];
    let f1 = NEG_ADPCM_TABLE[filter];

    for i in 0..28 {
      let mut sample = buffer[0x10 + (block/2) + (i * 4)];

      if block & 0b1 == 1 {
        sample >>= 4;
      }

      sample &= 0xf;

      let mut sample = ((sample as u16) << 12) as i16 as i32;
      sample >>= shift;

      let filter = (32 + self.previous_samples[channel][0] as i32 * f0 + self.previous_samples[channel][1] as i32 * f1) / 64;

      sample += filter;

      if sample > 0x7fff {
        sample = 0x7fff;
      } else if sample < -0x8000 {
        sample = -0x8000;
      }

      self.sample_buffer[channel].push(sample as i16);
      self.previous_samples[channel][1] = self.previous_samples[channel][0];
      self.previous_samples[channel][0] = sample as i16;
    }
  }

  fn read_data(&mut self) {
    if let Some(game_file) = &mut self.game_file {
      if self.debug_on {
        println!("[CDROM]reading data sector");
      }
      game_file.seek(SeekFrom::Start(self.file_pointer as u64)).unwrap();
      game_file.read_exact(&mut self.sector_buffer).unwrap();
    } else if let Some(game_bytes) = &self.game_bytes {
      let end_length = self.file_pointer + self.sector_buffer.len();
      self.sector_buffer = game_bytes[self.file_pointer..end_length].to_vec();
    }

    if self.interrupt_flags == 0 {
      if self.debug_on {
        println!("[CDROM]setting interrupt flags to 0x1 in read_data");
      }
      self.interrupt_flags = 0x1;
      self.response_buffer.push_back(self.get_stat());
    } else {
      self.drive_interrupt_pending = true;
      self.pending_stat = self.get_stat();
    }
  }

  fn get_seek_pointer(&self) -> u64 {
    let mut sector = self.current_ss as u64 * SECTORS_PER_SECOND + self.current_mm as u64 * SECTORS_PER_MINUTE + self.current_sect as u64;

    if sector >= LEAD_IN_SECTORS {
      sector -= LEAD_IN_SECTORS;
    }

    sector * BYTES_PER_SECTOR
  }

  fn drive_get_stat(&mut self) {
    if self.interrupt_flags == 0 {
      self.push_stat();

      self.controller_interrupt_flags = 0x2;

      self.controller_mode = ControllerMode::ResponseClear;
      self.controller_cycles += 10;

      self.drive_mode = DriveMode::Idle;
    }

    self.drive_cycles += 1;
  }

  fn tick_drive(&mut self, cycles: i32, spu: &mut SPU) {
    if self.debug_on {
      println!("drive cycles before subtract: {}, cd cycles = {cycles}, after: {}", self.drive_cycles, self.drive_cycles - cycles);
    }
    self.drive_cycles -= cycles;

    if self.drive_cycles <= 0 {
      match self.drive_mode {
        DriveMode::Idle => self.drive_cycles += cycles,
        DriveMode::Seek => self.seek_drive(),
        DriveMode::Play => self.play_drive(),
        DriveMode::Read => self.read_drive(spu),
        DriveMode::GetStat => self.drive_get_stat()
      }
    }
  }

  pub fn bcd_to_u8(value: u8) -> u8 {
    ((value >> 4) * 10) + (value & 0xf)
  }

  pub fn u8_to_bcd(value: u8) -> u8 {
    ((value / 10) << 4) | (value % 10)
  }

  fn controller_check_commands(&mut self, cycles: i32) {
    if self.debug_on {
      println!("[CDROM]inside controller_check_commands, controller mode = {:?}", self.controller_mode);
    }
    if self.command.is_some() {
      if !self.param_buffer.is_empty() {
        if self.debug_on {
          println!("[CDROM]scheduling param transfer");
        }
        self.controller_mode = ControllerMode::ParamTransfer;
      } else {
        if self.debug_on {
          println!("[CDROM]no params, transferring command");
        }
        self.controller_mode = ControllerMode::CommandTransfer;
      }
    }
    self.controller_cycles += cycles;
  }

  fn controller_param_transfer(&mut self) {
    if self.debug_on {
      println!("[CDROM]inside transfer params");
    }
    if !self.param_buffer.is_empty() {
      let param = self.param_buffer.pop_front().unwrap();

      self.controller_param_buffer.push_back(param);
    } else {
      if self.debug_on {
        println!("[CDROM]transferring command!");
      }
      self.controller_mode = ControllerMode::CommandTransfer;
    }

    self.controller_cycles += 10;
  }

  fn controller_command_transfer(&mut self) {
    if self.debug_on {
      println!("[CDROM]scheduling command execute!");
    }
    self.current_command = self.command.take().unwrap();

    self.controller_mode = ControllerMode::CommandExecute;

    self.controller_cycles += 10;
  }

  fn controller_command_execute(&mut self) {
    let command = self.current_command;

    self.controller_cycles += 10;

    self.controller_response_buffer.clear();

    self.execute(command);

    self.controller_param_buffer.clear();

    self.controller_mode = ControllerMode::ResponseClear;
  }

  fn controller_response_clear(&mut self) {
    if !self.response_buffer.is_empty() {
      self.response_buffer.pop_front();
    } else {
      self.controller_mode = ControllerMode::ResponseTransfer;
    }

    self.controller_cycles += 10;
  }

  fn controller_response_transfer(&mut self) {
    if !self.controller_response_buffer.is_empty() {
      self.response_buffer.push_back(self.controller_response_buffer.pop_front().unwrap());
    } else {
      self.controller_mode = ControllerMode::InterruptTransfer
    }

    self.controller_cycles += 10;
  }

  fn controller_interrupt_transfer(&mut self) {
    if self.interrupt_flags == 0 {
      self.interrupt_flags = self.controller_interrupt_flags;
      if self.debug_on {
        println!("[CDROM]setting interrupt flags to 0x{:x} in controller interrupt transfer", self.interrupt_flags);
        println!("[CDROM](transfer_interrupts) setting controller status to idle");
      }
      self.controller_mode = ControllerMode::Idle;

      self.controller_cycles += 10;
    } else {
      if self.debug_on {
        println!("[CDROM]irqs are *not* 0, checking one more time");
      }
      self.controller_cycles += 1;
    }
  }

  fn tick_controller(&mut self, cycles: i32) {
    if self.debug_on {
      println!("controller cycles before subtract: {}, cd cycles = {cycles}, after: {}", self.controller_cycles, self.controller_cycles - cycles);
    }
    self.controller_cycles -= cycles;

    if self.controller_cycles <= 0 {
      match self.controller_mode {
        ControllerMode::Idle => self.controller_check_commands(cycles),
        ControllerMode::ParamTransfer => self.controller_param_transfer(),
        ControllerMode::CommandTransfer => self.controller_command_transfer(),
        ControllerMode::CommandExecute => self.controller_command_execute(),
        ControllerMode::ResponseClear => self.controller_response_clear(),
        ControllerMode::ResponseTransfer => self.controller_response_transfer(),
        ControllerMode::InterruptTransfer => self.controller_interrupt_transfer()
      }
    }
  }

  fn execute(&mut self, command: u8) {
    if self.debug_on {
      println!("[CDROM]executing command 0x{command:x}");
    }
    if command == 0x1b {
      panic!("oh no!");
    }
    let mut interrupt = 0x3;
    match command {
      0x01 => self.push_stat(),
      0x02 => self.setloc(),
      0x06 => self.readn(),
      0x07 => {
        self.push_stat();

        self.controller_response_buffer.push_back(0x20);

        interrupt = 0x5;
      },
      0x09 => self.pause(),
      0x0a => self.init(),
      0x0b | 0x0c => self.push_stat(),
      0x0d => self.setfilter(),
      0x0e => self.setmode(),
      0x10 => self.getloc_l(),
      0x11 => self.getloc_p(),
      0x13 => {
        self.push_stat();

        self.controller_response_buffer.push_back(1);
        self.controller_response_buffer.push_back(1);
      }
      0x14 => {
        self.push_stat();
        self.controller_response_buffer.push_back(0);
        self.controller_response_buffer.push_back(0);
      }
      0x15 | 0x16 => self.seek(),
      0x19 => {
        let sub_function = self.controller_param_buffer.pop_front().unwrap();
        // per https://psx-spx.consoledev.net/cdromdrive/#19h20h-int3yymmddver
        // 97h,01h,10h,C2h  ;PSX (PU-18) (us/eur)     10 Jan 1997, version vC2 (a)
        if sub_function == 0x20 {
          self.controller_response_buffer.push_back(0x97);
          self.controller_response_buffer.push_back(0x01);
          self.controller_response_buffer.push_back(0x10);
          self.controller_response_buffer.push_back(0xc2);
        } else {
          panic!("unsupported subfunction given: {:x}", sub_function);
        }
      }
      0x1a => {
        self.push_stat();

        self.subresponse = SubResponse::GetID;

        self.subresponse_cycles += 50;
      }
      0x1b => self.reads(),
      0x1e => {
        self.push_stat();

        self.subresponse = SubResponse::GetStat;
        self.subresponse_cycles += 44100;
      }
      _ => todo!("command not implemented yet: {:x}", command)
    }

    self.controller_interrupt_flags = interrupt;
  }

  fn getloc_p(&mut self) {
    self.controller_response_buffer.push_back(self.subq.track);
    self.controller_response_buffer.push_back(self.subq.index);

    self.controller_response_buffer.push_back(Self::u8_to_bcd(self.subq.mm));
    self.controller_response_buffer.push_back(Self::u8_to_bcd(self.subq.ss));
    self.controller_response_buffer.push_back(Self::u8_to_bcd(self.subq.sect));

    self.controller_response_buffer.push_back(Self::u8_to_bcd(self.subq.amm));
    self.controller_response_buffer.push_back(Self::u8_to_bcd(self.subq.ass));
    self.controller_response_buffer.push_back(Self::u8_to_bcd(self.subq.asect));

  }

  fn getloc_l(&mut self) {
    self.controller_response_buffer.push_back(Self::u8_to_bcd(self.sector_header.mm));
    self.controller_response_buffer.push_back(Self::u8_to_bcd(self.sector_header.ss));
    self.controller_response_buffer.push_back(self.sector_header.sect);

    self.controller_response_buffer.push_back(self.sector_header.mode);
    self.controller_response_buffer.push_back(self.sector_subheader.file);
    self.controller_response_buffer.push_back(self.sector_subheader.channel);
    self.controller_response_buffer.push_back(self.sector_subheader.sub_mode);
    self.controller_response_buffer.push_back(self.sector_subheader.coding_info);
  }

  fn setfilter(&mut self) {
    let file = self.controller_param_buffer.pop_front().unwrap();
    let filter = self.controller_param_buffer.pop_front().unwrap();

    self.filter_file = file;
    self.filter_channel = filter & 0x1f;
  }

  fn init(&mut self) {
    self.push_stat();

    self.double_speed = false;
    self.sector_size = false;

    self.is_playing = false;
    self.is_reading = false;
    self.is_seeking = false;

    self.subresponse = SubResponse::GetStat;
    self.subresponse_cycles += 10;

  }

  fn setmode(&mut self) {
    let param = self.controller_param_buffer.pop_front().unwrap();

    self.double_speed = (param >> 7) & 0b1 == 1;
    self.send_adpcm_sectors = (param >> 6) & 0b1 == 1;
    self.sector_size = (param >> 5) & 0b1 == 1;
    // bit 4 is the ignore bit, but according to no$psx its purpose is unknown.
    // ignoring it for now

    self.xa_filter = (param >> 3) & 0b1 == 1;
    self.report_interrupts = (param >> 2) & 0b1 == 1;

  }

  fn pause(&mut self) {
    self.push_stat();

    if !self.is_playing && !self.is_reading && !self.is_seeking {
      self.subresponse_cycles += 10;
    } else {
      self.subresponse_cycles += if self.double_speed {
        1400
      } else {
        2800
      };
    }

    self.is_playing = false;
    self.is_reading = false;
    self.is_seeking = false;

    self.subresponse = SubResponse::GetStat;
  }

  fn readn(&mut self) {
    self.read_command(true);
  }

  fn reads(&mut self) {
    self.read_command(false);
  }

  fn read_command(&mut self, is_readn: bool) {
    if self.processing_seek {
      self.drive_mode = DriveMode::Seek;
      self.next_drive_mode = DriveMode::Read;

      self.is_seeking = true;
      self.is_reading = false;
      self.is_playing = false;

      self.drive_cycles += if self.double_speed {
        if is_readn { 140 } else { 14 }
      } else {
        if is_readn { 280 } else { 28 }
      };
    } else {
      self.drive_mode = DriveMode::Read;

      self.is_reading = true;
      self.is_playing = false;
      self.is_seeking = false;

      let divisor = if self.double_speed {
        150
      } else {
        75
      };

      self.drive_cycles += 44100 / divisor;
    }

    self.push_stat();
  }

  fn push_stat(&mut self) {
    let stat = self.get_stat();
    self.controller_response_buffer.push_back(stat);
  }

  fn setloc(&mut self) {
    self.push_stat();

    self.mm = Self::bcd_to_u8(self.controller_param_buffer.pop_front().unwrap());
    self.ss = Self::bcd_to_u8(self.controller_param_buffer.pop_front().unwrap());
    self.sect = Self::bcd_to_u8(self.controller_param_buffer.pop_front().unwrap());

    self.processing_seek = true;
  }

  fn seek(&mut self) {
    self.push_stat();

    self.drive_mode = DriveMode::Seek;
    self.next_drive_mode = DriveMode::GetStat;

    self.drive_cycles += if self.double_speed {
      14
    } else {
      28
    };

    self.is_seeking = true;
    self.is_playing = false;
    self.is_reading = false;
  }

  fn read_data_buffer(&mut self) -> u8 {
    let offset = if self.sector_size {
      ADDR_OFFSET
    } else {
      DATA_OFFSET
    };

    if self.data_buffer_empty() {
      panic!("data buffer is empty");
    }

    let val = self.data_buffer[offset + self.data_buffer_pointer];

    self.data_buffer_pointer += 1;

    val
  }

  fn data_buffer_empty(&self) -> bool {
    let max = if self.sector_size {
      0x924
    } else {
      0x800
    };

    self.data_buffer_pointer >= max
  }

  fn get_stat(&self) -> u8 {
    // bit 1 is for the "motor on" status, should always be 1 in our case
    let mut val = 0b10;
    val |= (self.is_playing as u8) << 7;
    val |= (self.is_seeking as u8) << 6;
    val |= (self.is_reading as u8) << 5;

    val
  }

  pub fn read_dma(&mut self) -> u32 {
    let byte0 = self.read_data_buffer() as u32;
    let byte1 = self.read_data_buffer() as u32;
    let byte2 = self.read_data_buffer() as u32;
    let byte3 = self.read_data_buffer() as u32;

    byte0 | (byte1 << 8) | (byte2 << 16) | (byte3 << 24)
  }

  pub fn read(&mut self, address: u32) -> u8 {
    match address & 0x3 {
      0 => {
        let mut value = self.index & 0x3;

        value |= ((self.controller_mode != ControllerMode::Idle) as u8) << 7;
        value |= (!self.data_buffer_empty() as u8) << 6;
        value |= (!self.response_buffer.is_empty() as u8) << 5;
        value |= ((self.param_buffer.len() < 16) as u8) << 4;
        value |= (self.param_buffer.is_empty() as u8) << 3;

        value
      },
      1 => if self.response_buffer.is_empty() { 0 } else { self.response_buffer.pop_front().unwrap() },
      2 => self.read_data_buffer(),
      3 => {
        match self.index {
          0 => (0b111 << 5) | self.interrupt_enable,
          1 => (0b111 << 5) | self.interrupt_flags,
          _ => todo!("offset 3 with index {} not implemented", self.index)
        }
      }
      _ => todo!("not implemented yet: {} (index = {})", address & 0x3, self.index)
    }
  }

  pub fn write(&mut self, address: u32, value: u8) {
    if address != 0x1f801800 {
      if self.debug_on {
        println!(
          "address = 0x{address:x} bank = {}, value = 0x{value:x}",
          self.index
        );
      }
    }

    match address & 0x3 {
      0 => self.index = value & 0x3,
      1 => {
        match self.index {
          0 => {
            if value == 0x1b {
              self.debug_on = true;
            }
            self.command = Some(value);
          }
          3 => (), // cd audio to spu, unimplemented
          _ => panic!("offset 1 with index {} not implemented", self.index)
        }
      }
      2 => {
        match self.index {
          0 => self.param_buffer.push_back(value),
          1 => self.interrupt_enable = value & 0x1f,
          2 => (), // left cd audio to spu, not implemented
          3 => (), // right cd audio to spu, not implemented
          _ => panic!("offset 2 with index {} not implemented yet", {self.index})
        }
      }
      3 => {
        match self.index {
          0 => {
            if (value >> 7) & 0b1 == 0 {
              if self.debug_on {
                println!("[CDROM]disabling data transfer");
              }
              self.data_buffer_pointer = 0x930;
            } else if self.data_buffer_empty() {
              if self.debug_on {
                println!("[CDROM]restarting data transfer");
              }
              self.data_buffer_pointer = 0;
              self.data_buffer[..0x930].clone_from_slice(&self.sector_buffer[..0x930]);
            }
          }
          1 => {
            // writing 1 to these bits clears them
            self.interrupt_flags &= !(value & 0x1f);
            if self.debug_on {
              println!("[CDROM]inside write_control, irqs = 0x{:x}", self.interrupt_flags);
            }

            if self.interrupt_flags == 0 && self.drive_interrupt_pending {
              if self.debug_on {
                println!("[CDROM]interrupt is pending, setting interrupt flags to 0x1");
              }
              self.interrupt_flags = 1;
              self.drive_interrupt_pending = false;
              self.response_buffer.push_back(self.pending_stat);
            }

            self.response_buffer.clear();

            if (value >> 6) & 0b1 == 1 {
              self.param_buffer.clear();
            }
          }
          2 | 3 => (), // more CD audio stuff
          _ => panic!("offset 3 with index {} not implemented yet", self.index)
        }
      }
      _ => todo!("not implemented yet: {:X} with index {}", address, self.index)
    }
  }
}