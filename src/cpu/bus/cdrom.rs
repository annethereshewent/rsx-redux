use std::collections::VecDeque;

use memmap2::Mmap;
use registers::HntmaskRegister;

use crate::cpu::bus::spu_legacy::{
    SPU,
    voices::{NEG_ADPCM_TABLE, POS_ADPCM_TABLE},
};

use super::{
    registers::interrupt_register::InterruptRegister,
    scheduler::{EventType, Scheduler},
};

pub mod registers;

// TODO: use actual numbers instead of these placeholder values lmao
pub const CDROM_CYCLES: usize = 768;
pub const BYTES_PER_SECTOR: usize = 2352;
// this one is verified to be a legit number per the CDROM standards
pub const CD_READ_CYCLES: usize = 451584;

const ZIGZAG_TABLE: [[i32; 29]; 7] = [
    [
        0, 0x0, 0x0, 0x0, 0x0, -0x0002, 0x000A, -0x0022, 0x0041, -0x0054, 0x0034, 0x0009, -0x010A,
        0x0400, -0x0A78, 0x234C, 0x6794, -0x1780, 0x0BCD, -0x0623, 0x0350, -0x016D, 0x006B, 0x000A,
        -0x0010, 0x0011, -0x0008, 0x0003, -0x0001,
    ],
    [
        0, 0x0, 0x0, -0x0002, 0x0, 0x0003, -0x0013, 0x003C, -0x004B, 0x00A2, -0x00E3, 0x0132,
        -0x0043, -0x0267, 0x0C9D, 0x74BB, -0x11B4, 0x09B8, -0x05BF, 0x0372, -0x01A8, 0x00A6,
        -0x001B, 0x0005, 0x0006, -0x0008, 0x0003, -0x0001, 0x0,
    ],
    [
        0, 0x0, -0x0001, 0x0003, -0x0002, -0x0005, 0x001F, -0x004A, 0x00B3, -0x0192, 0x02B1,
        -0x039E, 0x04F8, -0x05A6, 0x7939, -0x05A6, 0x04F8, -0x039E, 0x02B1, -0x0192, 0x00B3,
        -0x004A, 0x001F, -0x0005, -0x0002, 0x0003, -0x0001, 0x0, 0x0,
    ],
    [
        0, -0x0001, 0x0003, -0x0008, 0x0006, 0x0005, -0x001B, 0x00A6, -0x01A8, 0x0372, -0x05BF,
        0x09B8, -0x11B4, 0x74BB, 0x0C9D, -0x0267, -0x0043, 0x0132, -0x00E3, 0x00A2, -0x004B,
        0x003C, -0x0013, 0x0003, 0x0, -0x0002, 0x0, 0x0, 0x0,
    ],
    [
        -0x0001, 0x0003, -0x0008, 0x0011, -0x0010, 0x000A, 0x006B, -0x016D, 0x0350, -0x0623,
        0x0BCD, -0x1780, 0x6794, 0x234C, -0x0A78, 0x0400, -0x010A, 0x0009, 0x0034, -0x0054, 0x0041,
        -0x0022, 0x000A, -0x0001, 0x0, 0x0001, 0x0, 0x0, 0x0,
    ],
    [
        0x0002, -0x0008, 0x0010, -0x0023, 0x002B, 0x001A, -0x00EB, 0x027B, -0x0548, 0x0AFA,
        -0x16FA, 0x53E0, 0x3C07, -0x1249, 0x080E, -0x0347, 0x015B, -0x0044, -0x0017, 0x0046,
        -0x0023, 0x0011, -0x0005, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    ],
    [
        -0x0005, 0x0011, -0x0023, 0x0046, -0x0017, -0x0044, 0x015B, -0x0347, 0x080E, -0x1249,
        0x3C07, 0x53E0, -0x16FA, 0x0AFA, -0x0548, 0x027B, -0x00EB, 0x001A, 0x002B, -0x0023, 0x0010,
        -0x0008, 0x0002, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    ],
];

#[derive(Debug, PartialEq)]
enum CDMode {
    None,
    Mode1,
    Mode2,
}

impl CDMode {
    pub fn from(byte: u8) -> Self {
        match byte {
            1 => CDMode::Mode1,
            2 => CDMode::Mode2,
            _ => CDMode::None,
        }
    }
}

#[derive(Debug, PartialEq)]
enum CDReadMode {
    Video,
    Audio,
    Data,
}

enum Mode2Form {
    Form1,
    Form2,
}

#[derive(Copy, Clone, PartialEq, Debug)]
enum SpeakerOutput {
    Mono,
    Stereo,
}

#[derive(PartialEq)]
enum BitsPerSample {
    FourBits,
    EightBits,
}

struct CodingInfo {
    speaker_output: SpeakerOutput,
    sample_rate: usize,
    bits_per_sample: BitsPerSample,
    _emphasis: bool,
}

impl CodingInfo {
    pub fn new(byte: u8) -> Self {
        Self {
            speaker_output: match byte & 0x3 {
                0 => SpeakerOutput::Mono,
                1 => SpeakerOutput::Stereo,
                _ => panic!("speaker output is invalid"),
            },
            sample_rate: match (byte >> 2) & 0x3 {
                0 => 37800,
                1 => 18900,
                _ => panic!("coding_info sample rate is invalid"),
            },
            bits_per_sample: match (byte >> 4) & 0x3 {
                0 => BitsPerSample::FourBits,
                1 => BitsPerSample::EightBits,
                _ => panic!("bits per sample is invalid"),
            },
            _emphasis: (byte >> 6) & 1 == 1,
        }
    }
}

struct CDSubheader {
    file_num: u8,
    channel_num: u8,
    read_mode: CDReadMode,
    _form: Mode2Form,
    realtime: bool,
    coding_info: CodingInfo,
}

impl CDSubheader {
    pub fn from_buf(bytes: &[u8]) -> CDSubheader {
        let file_num = bytes[0];
        let channel_num = bytes[1] & 0xf;

        let read_mode = if (bytes[2] >> 1) & 1 == 1 {
            CDReadMode::Video
        } else if (bytes[2] >> 2) & 1 == 1 {
            CDReadMode::Audio
        } else if ((bytes[2] >> 3) & 1 == 1) || bytes[2] == 0 {
            CDReadMode::Data
        } else {
            panic!("unknown mode received")
        };

        let form = if (bytes[2] >> 5) == 0 {
            Mode2Form::Form1
        } else {
            Mode2Form::Form2
        };

        let realtime = (bytes[2] >> 6) & 1 == 1;

        Self {
            file_num,
            channel_num,
            read_mode,
            _form: form,
            coding_info: CodingInfo::new(bytes[3]),
            realtime,
        }
    }

    pub fn new() -> Self {
        Self {
            file_num: 0,
            channel_num: 0,
            read_mode: CDReadMode::Data,
            _form: Mode2Form::Form1,
            coding_info: CodingInfo::new(0),
            realtime: false,
        }
    }
}

#[derive(Debug)]
struct CDHeader {
    mm: u8,
    ss: u8,
    sect: u8,
    mode: CDMode,
}

impl CDHeader {
    pub fn new() -> Self {
        Self {
            ss: 0,
            sect: 0,
            mm: 0,
            mode: CDMode::None,
        }
    }
    pub fn from_buf(buf: &[u8]) -> CDHeader {
        let mm = CDRom::bcd_to_u8(buf[0]);
        let ss = CDRom::bcd_to_u8(buf[1]);
        let sect = CDRom::bcd_to_u8(buf[2]);
        let mode = CDMode::from(buf[3]);

        Self { mm, ss, sect, mode }
    }
}

#[derive(Debug)]
struct Msf {
    pub ass: u8,
    pub asect: u8,
    pub amm: u8,
}

impl Msf {
    pub fn new() -> Self {
        Self {
            ass: 0,
            amm: 0,
            asect: 0,
        }
    }
}

#[derive(Copy, Clone, PartialEq)]
enum ControllerMode {
    Idle,
    ExecuteCommand,
    LatchInterrupts,
    TransferCommand,
    TransferParams,
    ClearResponseFifo,
    TransferResponse,
}

#[derive(Copy, Clone, PartialEq)]
enum DriveMode {
    Idle,
    Seek,
    Stat,
    Play,
    Read,
}

#[derive(Copy, Clone, PartialEq)]
enum SubresponseMode {
    Disabled,
    GetId,
    GetStat,
}

pub struct CDRom {
    hntmask: HntmaskRegister,
    bank: usize,
    parameter_fifo: VecDeque<u8>,
    controller_param_fifo: VecDeque<u8>,
    controller_response_fifo: VecDeque<u8>,
    result_fifo: VecDeque<u8>,
    irq_latch: u8,
    irqs: u8,
    controller_mode: ControllerMode,
    drive_mode: DriveMode,
    subresponse_mode: SubresponseMode,
    command_latch: Option<u8>,
    command: u8,
    is_playing: bool,
    is_seeking: bool,
    is_reading: bool,
    current_msf: Msf,
    msf: Msf,
    next_mode: Option<DriveMode>,
    double_speed: bool,
    send_to_spu: bool,
    sector_size: usize,
    report_interrupts: bool,
    xa_filter: bool,
    #[cfg(not(target_arch = "wasm32"))]
    game_data: Option<Mmap>,
    current_header: CDHeader,
    subheader: CDSubheader,
    output_buffer: [u8; 0x930],
    buffer_index: usize,
    pre_seek: bool,
    pending_stat: Option<u8>,
    sample_buffer: Vec<i16>,
    old_samples: [i16; 2],
    older_samples: [i16; 2],
    sixstep: usize,
    p: usize,
    filter_file: u8,
    filter_channel: u8,
    drive_cycles: usize,
    controller_cycles: usize,
    subresponse_cycles: usize,
    pub debug_on: bool,
}

impl CDRom {
    pub fn new(scheduler: &mut Scheduler) -> Self {
        scheduler.schedule(EventType::TickCDRom, CDROM_CYCLES);

        Self {
            hntmask: HntmaskRegister::from_bits_retain(0),
            bank: 0,
            parameter_fifo: VecDeque::with_capacity(16),
            result_fifo: VecDeque::with_capacity(16),
            irq_latch: 0,
            controller_mode: ControllerMode::Idle,
            drive_mode: DriveMode::Idle,
            subresponse_mode: SubresponseMode::Disabled,
            irqs: 0,
            controller_param_fifo: VecDeque::with_capacity(16),
            command: 0,
            command_latch: None,
            controller_response_fifo: VecDeque::with_capacity(16),
            is_playing: false,
            is_reading: false,
            is_seeking: false,
            current_msf: Msf::new(),
            msf: Msf::new(),
            next_mode: None,
            double_speed: false,
            xa_filter: false,
            send_to_spu: false,
            sector_size: 0x800,
            report_interrupts: false,
            game_data: None,
            current_header: CDHeader::new(),
            subheader: CDSubheader::new(),
            buffer_index: 0,
            output_buffer: [0; 0x930],
            pre_seek: false,
            pending_stat: None,
            sample_buffer: Vec::new(),
            old_samples: [0; 2],
            older_samples: [0; 2],
            sixstep: 6,
            p: 0,
            filter_file: 0,
            filter_channel: 0,
            drive_cycles: 1,
            controller_cycles: 1,
            subresponse_cycles: 1,
            debug_on: false,
        }
    }

    fn transfer_response(&mut self) {
        if self.result_fifo.len() < 16 && !self.controller_response_fifo.is_empty() {
            let value = self.controller_response_fifo.pop_front().unwrap();
            self.result_fifo.push_back(value);

            self.controller_mode = ControllerMode::TransferResponse;
        } else {
            self.controller_mode = ControllerMode::LatchInterrupts;
        }

        self.controller_cycles += 10;
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_game_desktop(&mut self, game: Mmap) {
        self.game_data = Some(game);
    }

    fn read_hintsts(&self) -> u8 {
        self.irqs | 0x7 << 5
    }

    fn read_response(&mut self) -> u8 {
        if self.result_fifo.is_empty() {
            0
        } else {
            self.result_fifo.pop_front().unwrap()
        }
    }

    pub fn read_data_buffer(&mut self) -> u32 {
        (self.read_data_buffer_byte() as u32)
            | (self.read_data_buffer_byte() as u32) << 8
            | (self.read_data_buffer_byte() as u32) << 16
            | (self.read_data_buffer_byte() as u32) << 24
    }

    fn read_data_buffer_byte(&mut self) -> u8 {
        if !self.output_buffer_empty() {
            let offset = if self.sector_size == 0x924 { 0xc } else { 0x18 };

            let value = self.output_buffer[self.buffer_index + offset];

            self.buffer_index += 1;

            return value;
        }

        panic!("data buffer is empty, shouldnt happen");
    }

    fn output_buffer_empty(&self) -> bool {
        self.buffer_index >= self.sector_size
    }

    pub fn read(&mut self, address: usize) -> u8 {
        match address {
            0x1f801800 => self.read_hsts(),
            0x1f801801 => self.read_response(),
            0x1f801802 => self.read_data_buffer_byte(),
            0x1f801803 => match self.bank {
                0 | 2 => self.hntmask.bits(),
                1 | 3 => self.read_hintsts(),
                _ => todo!("address: 0x{:x}, bank = {}", address, self.bank),
            },
            _ => todo!("address: 0x{:x}, bank = {}", address, self.bank),
        }
    }
    /*
    0-1 RA       Current register bank (R/W)
    2   ADPBUSY  ADPCM busy            (R, 1=playing XA-ADPCM)
    3   PRMEMPT  Parameter empty       (R, 1=parameter FIFO empty)
    4   PRMWRDY  Parameter write ready (R, 1=parameter FIFO not full)
    5   RSLRRDY  Result read ready     (R, 1=result FIFO not empty)
    6   DRQSTS   Data request          (R, 1=one or more RDDATA reads or WRDATA writes pending)
    7   BUSYSTS  Busy status           (R, 1=HC05 busy acknowledging command)
    */
    fn read_hsts(&self) -> u8 {
        (self.bank as u8)
            | (self.parameter_fifo.is_empty() as u8) << 3
            | ((self.parameter_fifo.len() < 16) as u8) << 4
            | (!self.result_fifo.is_empty() as u8) << 5
            | (!self.output_buffer_empty() as u8) << 6
            | ((self.controller_mode != ControllerMode::Idle) as u8) << 7
    }

    /*
    7  Play          Playing CD-DA         ;\only ONE of these bits can be set
    6  Seek          Seeking               ; at a time (ie. Read/Play won't get
    5  Read          Reading data sectors  ;/set until after Seek completion)
    4  ShellOpen     Once shell open (0=Closed, 1=Is/was Open)
    3  IdError       (0=Okay, 1=GetID denied) (also set when Setmode.Bit4=1)
    2  SeekError     (0=Okay, 1=Seek error)     (followed by Error Byte)
    1  Spindle Motor (0=Motor off, or in spin-up phase, 1=Motor on)
    0  Error         Invalid Command/parameters (followed by Error Byte)
     */
    fn commandx19(&mut self) {
        let subcommand = self.controller_param_fifo.pop_front().unwrap();

        self.execute_subcommand(subcommand);
    }

    fn check_commands(&mut self) {
        if self.command_latch.is_some() {
            if !self.parameter_fifo.is_empty() {
                self.controller_mode = ControllerMode::TransferParams;
            } else {
                self.controller_mode = ControllerMode::TransferCommand;
            }
        }

        self.controller_cycles += 1;
    }

    fn stat(&mut self) {
        let mut val = 1 << 1; // bit 1 is always set to 1, "motor on"

        val |= (self.is_reading as u8) << 5;
        val |= (self.is_seeking as u8) << 6;
        val |= (self.is_playing as u8) << 7;

        self.controller_response_fifo.push_back(val);
    }

    pub fn tick(
        &mut self,
        scheduler: &mut Scheduler,
        spu: &mut SPU,
        interrupt_register: &mut InterruptRegister,
        cycles_left: usize,
    ) {
        self.tick_subresponse();
        self.tick_drive(spu, interrupt_register);
        self.tick_controller(interrupt_register);

        self.process_irqs(interrupt_register);

        scheduler.schedule(EventType::TickCDRom, CDROM_CYCLES - cycles_left);
    }

    fn tick_subresponse(&mut self) {
        if self.subresponse_mode != SubresponseMode::Disabled {
            self.subresponse_cycles -= 1;
        }

        if self.subresponse_cycles == 0 {
            match self.subresponse_mode {
                SubresponseMode::Disabled => (),
                SubresponseMode::GetId => self.read_id(),
                SubresponseMode::GetStat => self.sub_stat(),
            }
        }
    }

    fn sub_stat(&mut self) {
        if self.irqs == 0 {
            self.irq_latch = 2;
            self.stat();

            self.controller_cycles += 10;
            self.controller_mode = ControllerMode::ClearResponseFifo;
            self.subresponse_mode = SubresponseMode::Disabled;
        }
    }

    fn tick_drive(&mut self, spu: &mut SPU, interrupt_register: &mut InterruptRegister) {
        if self.drive_mode != DriveMode::Idle {
            self.drive_cycles -= 1;
        }

        if self.drive_cycles == 0 {
            match self.drive_mode {
                DriveMode::Idle => (),
                DriveMode::Seek => self.seek_cd(),
                DriveMode::Stat => self.cd_stat(),
                DriveMode::Read => self.cd_read_sector(spu, interrupt_register),
                DriveMode::Play => todo!("play drive"),
            }
        }
    }

    fn tick_controller(&mut self, interrupt_register: &mut InterruptRegister) {
        match self.controller_mode {
            ControllerMode::Idle => self.check_commands(),
            ControllerMode::ExecuteCommand => self.execute_command(),
            ControllerMode::LatchInterrupts => self.transfer_interrupts(interrupt_register),
            ControllerMode::TransferCommand => self.transfer_command(),
            ControllerMode::TransferParams => self.transfer_params(),
            ControllerMode::ClearResponseFifo => self.clear_response(),
            ControllerMode::TransferResponse => self.transfer_response(),
        }
    }

    fn process_irqs(&mut self, interrupt_register: &mut InterruptRegister) {
        if self.irqs & self.hntmask.enable_irq() != 0 {
            interrupt_register.insert(InterruptRegister::CDROM);
        }
    }

    fn transfer_command(&mut self) {
        self.command = self.command_latch.take().unwrap();

        self.controller_mode = ControllerMode::ExecuteCommand;

        self.controller_cycles += 10;
    }

    fn transfer_params(&mut self) {
        if !self.parameter_fifo.is_empty() {
            let byte = self.parameter_fifo.pop_front().unwrap();

            self.controller_param_fifo.push_back(byte);
        } else {
            self.controller_mode = ControllerMode::TransferCommand
        }

        self.controller_cycles += 10;
    }

    fn transfer_interrupts(&mut self, interrupt_register: &mut InterruptRegister) {
        if self.irqs == 0 {
            self.irqs = self.irq_latch;
            self.process_irqs(interrupt_register);

            self.controller_mode = ControllerMode::Idle;
            self.controller_cycles += 10;
        } else {
            self.controller_cycles += 1;
        }
    }

    fn execute_subcommand(&mut self, subcommand: u8) {
        match subcommand {
            0x20 => {
                // get date
                // PSX/PSone date in byte form
                let bytes = [0x99, 0x2, 0x1, 0xC3];

                for byte in bytes {
                    self.controller_response_fifo.push_back(byte);
                }
            }
            _ => todo!("subcommand = 0x{:x}", subcommand),
        }
    }

    fn set_mode(&mut self) {
        self.stat();

        let byte = self.controller_param_fifo.pop_front().unwrap();

        self.double_speed = (byte >> 7) & 1 == 1;
        self.send_to_spu = (byte >> 6) & 1 == 1;
        self.sector_size = if (byte >> 5) & 1 == 1 { 0x924 } else { 0x800 };

        // ignore bit is bit 4, but purpose is unknown, so ignoring it for now.

        self.xa_filter = (byte >> 3) & 1 == 1;
        self.report_interrupts = (byte >> 2) & 1 == 1;

        // bits 0 and 1 are audio related so no need to worry about them
    }

    fn pause(&mut self) {
        self.stat();

        if !self.is_playing && !self.is_reading && !self.is_seeking {
            self.subresponse_cycles += 10;
        } else {
            self.subresponse_cycles += if self.double_speed { 1400 } else { 2800 };
        }

        self.is_playing = false;
        self.is_seeking = false;
        self.is_reading = false;

        self.subresponse_mode = SubresponseMode::GetStat;
    }

    fn init(&mut self) {
        self.stat();

        self.double_speed = false;
        self.sector_size = 0x800;

        self.is_playing = false;
        self.is_seeking = false;
        self.is_reading = false;

        self.subresponse_mode = SubresponseMode::GetStat;
        self.subresponse_cycles += 10;
    }

    fn execute_command(&mut self) {
        self.controller_response_fifo.clear();

        self.irq_latch = 3;

        self.controller_response_fifo.clear();

        match self.command {
            0x1 => self.stat(),
            0x2 => self.set_loc(),
            0x6 | 0x1b => self.cd_read_command(),
            0x9 => self.pause(),
            0xa => self.init(),
            0xb | 0xc => self.stat(),
            0xe => self.set_mode(),
            0x15 => self.seek(),
            0x19 => self.commandx19(),
            0x1a => self.get_id(),
            0x1e => self.toc(),
            _ => todo!("command byte 0x{:x}", self.command),
        }

        self.controller_mode = ControllerMode::ClearResponseFifo;
        self.controller_cycles += 10;

        self.controller_param_fifo.clear();
    }

    fn toc(&mut self) {
        self.stat();

        self.subresponse_mode = SubresponseMode::GetStat;
        self.subresponse_cycles += 44100;
    }

    fn cd_read_sector(&mut self, spu: &mut SPU, interrupt_register: &mut InterruptRegister) {
        if !self.is_reading {
            self.drive_mode = DriveMode::Idle;
            self.drive_cycles += 1;

            return;
        }

        let pointer = self.get_pointer();

        self.stat();

        // TODO: refactor this to allow for WASM builds to work as well
        if let Some(game_data) = &mut self.game_data {
            if self.debug_on {
                println!("pointer: {pointer}");
                println!("game_data length: {}", game_data.len());
            }
            self.current_header = CDHeader::from_buf(&game_data[pointer + 0xc..pointer + 0x10]);

            if self.current_header.mode != CDMode::Mode2 {
                todo!("non mode 2 header")
            }
            self.subheader = CDSubheader::from_buf(&game_data[pointer + 0x10..pointer + 0x14]);

            if self.current_header.mm != self.current_msf.amm
                || self.current_header.ss != self.current_msf.ass
                || self.current_header.sect != self.current_msf.asect
            {
                panic!("mismatch between header and current msf");
            }

            if self.subheader.read_mode == CDReadMode::Audio
                && (!self.send_to_spu || !self.subheader.realtime)
            {
                self.subheader.read_mode = CDReadMode::Data;
            }

            match self.subheader.read_mode {
                CDReadMode::Data => self.read_data(interrupt_register),
                CDReadMode::Audio => {
                    if !self.xa_filter
                        || (self.filter_file == self.subheader.file_num
                            && self.subheader.channel_num == self.filter_channel)
                    {
                        self.read_audio(spu)
                    }
                }
                CDReadMode::Video => todo!("read video cds"),
            }

            self.current_msf.asect += 1;

            if self.current_msf.asect >= 75 {
                self.current_msf.asect -= 75;
                self.current_msf.ass += 1;

                if self.current_msf.ass >= 60 {
                    self.current_msf.amm += 1;
                    self.current_msf.ass = 0;

                    if self.current_msf.amm == 74 {
                        self.current_msf.amm = 0;
                    }
                }
            }
        }
        if self.is_reading {
            self.drive_cycles += self.get_drive_cycles();
        }
    }

    fn get_drive_cycles(&self) -> usize {
        let divisor = if self.double_speed { 150 } else { 75 };

        44100 / divisor
    }

    fn read_audio(&mut self, spu: &mut SPU) {
        if self.subheader.coding_info.bits_per_sample == BitsPerSample::EightBits {
            todo!("8 bit audio not supported");
        }

        if let Some(game_data) = &self.game_data {
            let pointer = self.get_pointer() + 24;

            let mut audio_sector = [0; 0x914];

            audio_sector.copy_from_slice(&game_data[pointer..pointer + 0x914]);

            for i in 0..0x12 {
                let section_index = i * 128;
                let section = &audio_sector[section_index..section_index + 128];

                self.decode_section(section);
            }

            let samples: Vec<i16> = self.sample_buffer.drain(..).collect();

            if self.subheader.coding_info.sample_rate == 37800 {
                self.resample_37800_sample(&samples, spu);
            } else {
                self.resample_18900_sample(&samples, spu);
            }
        }
    }

    fn resample_37800_sample(&mut self, samples: &[i16], spu: &mut SPU) {
        let mut sixstep = self.sixstep;
        let mut p = self.p;

        let is_stereo = self.subheader.coding_info.speaker_output == SpeakerOutput::Stereo;

        let mut ringbuffer = [vec![0; samples.len()], vec![0; samples.len()]];

        let mut sample_index = 0;

        while sample_index < samples.len() {
            ringbuffer[0][p] = samples[sample_index];
            sample_index += 1;
            if is_stereo {
                ringbuffer[1][p] = samples[sample_index];
                sample_index += 1;
            }

            sixstep -= 1;
            p = (p + 1) & 0x1f;

            if sixstep == 0 {
                sixstep = 6;

                for i in 0..7 {
                    let left = self.zigzag_interpolate(p, i, &ringbuffer[0]);
                    let right = if is_stereo {
                        self.zigzag_interpolate(p, i, &ringbuffer[1])
                    } else {
                        left
                    };

                    spu.cd_left_samples.push_back(left);
                    spu.cd_right_samples.push_back(right);
                }
            }
        }

        self.sixstep = sixstep;
        self.p = p;
    }

    fn zigzag_interpolate(&mut self, p: usize, table: usize, ringbuffer: &[i16]) -> i16 {
        let mut sum: i32 = 0;

        for i in 0..29 {
            sum += (ringbuffer[(p - i) & 0x1f] as i32 * ZIGZAG_TABLE[table][i]) / 0x8000;
        }

        sum.clamp(-0x8000, 0x7fff) as i16
    }

    fn resample_18900_sample(&mut self, _samples: &[i16], _spu: &mut SPU) {
        todo!("0x18900 resample");
    }

    fn decode_section(&mut self, section: &[u8]) {
        let headers = &section[4..12];

        let block_start = 16;
        for i in 0..28 {
            let index = block_start + i * 4;
            let word = unsafe { *(&section[index] as *const u8 as *const u32) };

            self.decode_xa_word(word, headers);
        }
    }

    fn decode_xa_word(&mut self, word: u32, headers: &[u8]) {
        for i in 0..8 {
            let channel_index =
                if self.subheader.coding_info.speaker_output == SpeakerOutput::Stereo {
                    i & 1
                } else {
                    0
                };
            let header = headers[i];

            let mut shift = header & 0xf;
            let filter = (header >> 4) & 0x3;

            if shift > 12 {
                shift = 9;
            }

            let f0 = POS_ADPCM_TABLE[filter as usize];
            let f1 = NEG_ADPCM_TABLE[filter as usize];

            let nibble = ((word >> (i * 4)) & 0xf) as i32;

            let mut sample = (((nibble << 12) as i16) >> shift) as i32;

            sample += (self.old_samples[channel_index] as i32 * f0
                + self.older_samples[channel_index] as i32 * f1
                + 32)
                / 64;

            sample = sample.clamp(-0x8000, 0x7fff);

            self.sample_buffer.push(sample as i16);

            self.older_samples[channel_index] = self.old_samples[channel_index];
            self.old_samples[channel_index] = sample as i16;
        }
    }

    fn read_data(&mut self, interrupt_register: &mut InterruptRegister) {
        if let Some(game_data) = &self.game_data {
            let pointer = self.get_pointer();

            self.output_buffer
                .copy_from_slice(&game_data[pointer..pointer + 0x930]);

            let mut val = 1 << 1; // bit 1 is always set to 1, "motor on"

            val |= (self.is_reading as u8) << 5;
            val |= (self.is_seeking as u8) << 6;
            val |= (self.is_playing as u8) << 7;

            if self.irqs == 0 {
                self.irqs = 1;
                self.process_irqs(interrupt_register);
                self.result_fifo.push_back(val);
            } else {
                self.pending_stat = Some(val);
            }
        }
    }

    fn cd_read_command(&mut self) {
        if !self.pre_seek {
            // cd has been seeked and is able to be read
            self.is_reading = true;
            self.is_seeking = false;
            self.is_playing = false;

            self.drive_cycles += self.get_drive_cycles();
            self.drive_mode = DriveMode::Read;
        } else {
            self.is_seeking = true;
            self.is_reading = false;
            self.is_playing = false;
            // request a seek
            self.next_mode = Some(DriveMode::Read);

            let cycles = if self.double_speed { 14 } else { 28 };

            self.drive_cycles += cycles;
            self.drive_mode = DriveMode::Seek;
        }

        self.stat();
    }

    fn get_pointer(&self) -> usize {
        let mm = self.current_msf.amm as usize;
        let ss = self.current_msf.ass as usize;
        let sect = self.current_msf.asect as usize;

        ((mm * 60 + ss) * 75 + sect - 150) * BYTES_PER_SECTOR
    }

    fn bcd_to_u8(value: u8) -> u8 {
        ((value >> 4) * 10) + (value & 0xf)
    }

    fn cd_stat(&mut self) {
        assert!(self.irqs == 0);

        self.stat();

        self.irq_latch = 0x2;

        self.controller_cycles += 10;
        self.controller_mode = ControllerMode::ClearResponseFifo;

        self.drive_mode = DriveMode::Idle;

        self.subresponse_mode = SubresponseMode::Disabled;
    }

    fn seek_cd(&mut self) {
        let mut msf = Msf::new();

        self.pre_seek = false;

        msf.amm = self.msf.amm;
        msf.ass = self.msf.ass;
        msf.asect = self.msf.asect;

        self.current_msf = msf;

        if let Some(mode) = self.next_mode.take() {
            self.drive_mode = mode;
            match mode {
                DriveMode::Play => {
                    self.is_playing = true;
                    self.is_seeking = false;
                    self.is_reading = false;
                    self.drive_cycles += self.get_drive_cycles();
                }
                DriveMode::Read => {
                    self.is_reading = true;
                    self.is_seeking = false;
                    self.is_playing = false;
                    self.drive_cycles += self.get_drive_cycles();
                }
                _ => self.drive_cycles += 10,
            }
        }
    }

    fn seek(&mut self) {
        self.stat();

        self.is_playing = false;
        self.is_reading = false;
        self.is_seeking = true;

        self.next_mode = Some(DriveMode::Stat);

        let cycles = if self.double_speed { 14 } else { 28 };

        self.drive_cycles += cycles;
        self.drive_mode = DriveMode::Seek;
    }

    fn set_loc(&mut self) {
        self.stat();

        self.msf.amm = Self::bcd_to_u8(self.controller_param_fifo.pop_front().unwrap());
        self.msf.ass = Self::bcd_to_u8(self.controller_param_fifo.pop_front().unwrap());
        self.msf.asect = Self::bcd_to_u8(self.controller_param_fifo.pop_front().unwrap());

        self.pre_seek = true;
    }

    fn get_id(&mut self) {
        self.stat();

        self.subresponse_mode = SubresponseMode::GetId;
        self.subresponse_cycles += 50;
    }

    fn read_id(&mut self) {
        assert!(self.irqs == 0);

        self.irq_latch = 0x2;

        let bytes = "SCEA".as_bytes();

        self.controller_response_fifo.push_back(0x2);
        self.controller_response_fifo.push_back(0x0);
        self.controller_response_fifo.push_back(0x20);
        self.controller_response_fifo.push_back(0x0);
        for byte in bytes {
            self.controller_response_fifo.push_back(*byte);
        }

        self.controller_mode = ControllerMode::ClearResponseFifo;

        self.subresponse_mode = SubresponseMode::Disabled;
        self.controller_cycles += 10;
    }

    fn clear_response(&mut self) {
        if !self.result_fifo.is_empty() {
            self.result_fifo.pop_back();
        } else {
            self.controller_mode = ControllerMode::TransferResponse;
        }

        self.controller_cycles += 10;
    }

    fn write_control(&mut self, value: u8, interrupt_register: &mut InterruptRegister) {
        self.irqs &= !(value & 0x1f);

        if (value >> 6) & 1 == 1 {
            self.parameter_fifo.clear();
        }
        self.result_fifo.clear();
        if self.irqs == 0 {
            if let Some(stat) = self.pending_stat.take() {
                self.result_fifo.push_back(stat);
                self.irqs = 0x1;
                self.process_irqs(interrupt_register);
            }
        }
    }

    pub fn write(&mut self, address: usize, value: u8, interrupt_register: &mut InterruptRegister) {
        match address {
            0x1f801801 => match self.bank {
                0 => self.command_latch = Some(value),
                2 | 3 => (), // TODO: SPU CD Audio stuff
                _ => todo!("bank = {}", self.bank),
            },
            0x1f801802 => match self.bank {
                0 => self.parameter_fifo.push_back(value),
                1 => {
                    self.hntmask.write(value);
                    self.process_irqs(interrupt_register);
                }
                2 | 3 => (), // TODO: SPU CD Audio stuff
                _ => todo!("bank = {}", self.bank),
            },
            0x1f801803 => match self.bank {
                0 => {
                    if (value >> 7) & 1 == 0 {
                        self.buffer_index = 0x930;
                    } else if self.output_buffer_empty() {
                        self.buffer_index = 0;
                    }
                }
                1 => self.write_control(value, interrupt_register),
                2 | 3 => (), // TODO: SPU CD Audio stuff
                _ => todo!("bank = {}", self.bank),
            },
            _ => todo!("(cdrom) address: 0x{:x}", address),
        }
    }

    pub fn write_bank(&mut self, value: u8) {
        self.bank = (value & 0x3) as usize;
    }
}
