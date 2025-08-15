use std::collections::VecDeque;

use memmap2::Mmap;
use registers::HntmaskRegister;

use super::{registers::interrupt_register::InterruptRegister, scheduler::{EventType, Scheduler}};

pub mod registers;

// TODO: use actual numbers instead of these placeholder values lmao
pub const CDROM_CYCLES: usize = 768;
pub const BYTES_PER_SECTOR: usize = 2352; // this one is verified to be a legit number per the CDROM standards
pub const CD_READ_CYCLES: usize = 451584;

#[derive(Debug, PartialEq)]
enum CDMode {
    None,
    Mode1,
    Mode2
}

impl CDMode {
    pub fn from(byte: u8) -> Self {
        match byte {
            1 => CDMode::Mode1,
            2 => CDMode::Mode2,
            _ => CDMode::None
        }
    }
}


#[derive(Debug)]
enum CDReadMode {
    Video,
    Audio,
    Data
}

enum Mode2Form {
    Form1,
    Form2
}

struct CDSubheader {
    file_num: u8,
    channel_num: u8,
    read_mode: CDReadMode,
    form: Mode2Form
}

impl CDSubheader {
    pub fn from_buf(bytes: &[u8]) -> CDSubheader {
        let file_num = bytes[0];
        let channel_num = bytes[1] & 0xf;

        let read_mode = if (bytes[2] >> 1) & 1 == 1 {
            CDReadMode::Video
        } else if (bytes[2] >> 2) & 1 == 1 {
            CDReadMode::Audio
        } else if (bytes[2] >> 3) & 1 == 1 {
            CDReadMode::Data
        } else {
            panic!("unknown mode received")
        };

        let form = if (bytes[2] >> 5) == 0 {
            Mode2Form::Form1
        } else {
            Mode2Form::Form2
        };

        Self {
            file_num,
            channel_num,
            read_mode,
            form
        }
    }

    pub fn new() -> Self {
        Self {
            file_num: 0,
            channel_num: 0,
            read_mode: CDReadMode::Data,
            form: Mode2Form::Form1
        }
    }
}

struct CDHeader {
    mm: u8,
    ss: u8,
    sect: u8,
    mode: CDMode
}

impl CDHeader {
    pub fn new() -> Self {
        Self {
            ss: 0,
            sect: 0,
            mm: 0,
            mode: CDMode::None
        }
    }
    pub fn from_buf(buf: &[u8]) -> CDHeader {
        let mm = CDRom::bcd_to_u8(buf[0]);
        let ss = CDRom::bcd_to_u8(buf[1]);
        let sect = CDRom::bcd_to_u8(buf[2]);
        let mode = CDMode::from(buf[3]);

        Self {
            mm,
            ss,
            sect,
            mode
        }
    }
}

#[derive(Debug)]
struct Msf {
    pub ass: u8,
    pub asect: u8,
    pub amm: u8
}

impl Msf {
    pub fn new() -> Self {
        Self {
            ass: 0,
            amm: 0,
            asect: 0
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum ControllerStatus {
    Idle,
    Busy
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
    controller_status: ControllerStatus,
    command_latch: Option<u8>,
    command: u8,
    is_playing: bool,
    is_seeking: bool,
    is_reading: bool,
    current_msf: Msf,
    msf: Msf,
    next_event: Option<EventType>,
    double_speed: bool,
    send_to_spu: bool,
    sector_size: usize,
    report_interrupts: bool,
    xa_filter: bool,
    #[cfg(not(target_arch = "wasm32"))]
    game_data: Option<Mmap>,
    current_header: CDHeader,
    subheader: CDSubheader,
    sector_buffer: Vec<u8>,
    output_buffer: [u8; 0x930],
    buffer_index: usize,
    reading_buffer: bool,
    pre_seek: bool
}

impl CDRom {
    pub fn new(scheduler: &mut Scheduler) -> Self {
        scheduler.schedule(EventType::CDCheckCommands, 10 * CDROM_CYCLES);
        scheduler.schedule(EventType::CDCheckIrqs, CDROM_CYCLES);
        Self {
            hntmask: HntmaskRegister::from_bits_retain(0),
            bank: 0,
            parameter_fifo: VecDeque::with_capacity(16),
            result_fifo: VecDeque::with_capacity(16),
            irq_latch: 0,
            controller_status: ControllerStatus::Idle,
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
            next_event: None,
            double_speed: false,
            xa_filter: false,
            send_to_spu: false,
            sector_size: 0x800,
            report_interrupts: false,
            game_data: None,
            current_header: CDHeader::new(),
            subheader: CDSubheader::new(),
            sector_buffer: vec![0; 0x930],
            buffer_index: 0,
            output_buffer: [0; 0x930],
            reading_buffer: false,
            pre_seek: false
        }
    }


    pub fn transfer_response(&mut self, scheduler: &mut Scheduler) {
        if self.result_fifo.len() < 16 && self.controller_response_fifo.len() > 0 {
            let value = self.controller_response_fifo.pop_front().unwrap();
            self.result_fifo.push_back(value);

            scheduler.schedule(EventType::CDResponseTransfer, 10 * CDROM_CYCLES);
        } else {
            scheduler.schedule(EventType::CDLatchInterrupts, 10 * CDROM_CYCLES);
        }
    }


    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_game_arm64(&mut self, game: Mmap) {
        self.game_data = Some(game);
    }



    fn read_hintsts(&self) -> u8 {
        self.irqs | 0x7 << 5
    }

    fn read_response(&mut self) -> u8 {
        if self.result_fifo.is_empty() { 0 } else { self.result_fifo.pop_front().unwrap() }
    }

    pub fn read_data_buffer(&mut self) -> u32 {
        (self.read_data_buffer_byte() as u32) |
            (self.read_data_buffer_byte() as u32) << 8 |
            (self.read_data_buffer_byte() as u32) << 16 |
            (self.read_data_buffer_byte() as u32) << 24
    }

    fn read_data_buffer_byte(&mut self) -> u8 {
        if !self.output_buffer_empty() {
            let offset = if self.sector_size == 0x924 {
                0xc
            } else {
                0x18
            };

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
            0x1f801803 => match self.bank {
                0 | 2 => self.hntmask.bits(),
                1 | 3 => self.read_hintsts(),
                _ => todo!("address: 0x{:x}, bank = {}", address, self.bank)
            }
            _ => todo!("address: 0x{:x}, bank = {}", address, self.bank)
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
    pub fn read_hsts(&self) -> u8 {
        (self.bank as u8) |
            (self.parameter_fifo.is_empty() as u8) << 3 |
            ((self.parameter_fifo.len() < 16) as u8) << 4 |
            (!self.result_fifo.is_empty() as u8) << 5 |
            ((self.controller_status != ControllerStatus::Idle) as u8) << 7
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

    pub fn check_commands(&mut self, scheduler: &mut Scheduler) {
        if self.command_latch.is_some() {
            self.controller_status = ControllerStatus::Busy;
            scheduler.schedule(EventType::CDParamTransfer, 10 * CDROM_CYCLES);
        } else {
            scheduler.schedule(EventType::CDCheckCommands, 10 * CDROM_CYCLES);
        }
    }

    pub fn stat(&mut self) {
        let mut val = 1 << 1; // bit 1 is always set to 1, "motor on"

        val |= (self.is_reading as u8) << 5;
        val |= (self.is_seeking as u8) << 6;
        val |= (self.is_playing as u8) << 7;

        self.controller_response_fifo.push_back(val);
    }

    pub fn process_irqs(&mut self, scheduler: &mut Scheduler, interrupt_register: &mut InterruptRegister) {
        if self.irqs & self.hntmask.enable_irq() != 0 {
            interrupt_register.insert(InterruptRegister::CDROM);
        }

        scheduler.schedule(EventType::CDCheckIrqs, CDROM_CYCLES);
    }

    pub fn transfer_command(&mut self, scheduler: &mut Scheduler) {
        self.command = self.command_latch.take().unwrap();

        scheduler.schedule(EventType::CDExecuteCommand, CDROM_CYCLES * 10);
    }

    pub fn transfer_params(&mut self, scheduler: &mut Scheduler) {
        if !self.parameter_fifo.is_empty() {
            let byte = self.parameter_fifo.pop_front().unwrap();

            self.controller_param_fifo.push_back(byte);

            scheduler.schedule(EventType::CDParamTransfer, CDROM_CYCLES * 10);
        } else {
            scheduler.schedule(EventType::CDCommandTransfer, CDROM_CYCLES * 10);
        }
    }

    pub fn transfer_interrupts(&mut self, scheduler: &mut Scheduler) {
        if self.irqs == 0 {
            self.irqs = self.irq_latch;

            self.controller_status = ControllerStatus::Idle;
            scheduler.schedule(EventType::CDCheckCommands, 10 * CDROM_CYCLES);
        } else {
            scheduler.schedule(EventType::CDLatchInterrupts, CDROM_CYCLES);
        }
    }


    pub fn execute_subcommand(&mut self, subcommand: u8) {
        match subcommand {
            0x20 => {
                // get date
                // PSX/PSone date in byte form
                let bytes = [0x99,0x2,0x1,0xC3];

                for byte in bytes {
                    self.controller_response_fifo.push_back(byte);
                }
            }
            _ => todo!("subcommand = 0x{:x}", subcommand)
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

    fn pause(&mut self, scheduler: &mut Scheduler) {
        self.stat();

        self.is_playing = false;
        self.is_seeking = false;
        self.is_reading = false;

        scheduler.schedule(EventType::CDStat, 100 * CDROM_CYCLES);
    }

    pub fn execute_command(&mut self, scheduler: &mut Scheduler) {
        self.controller_response_fifo.clear();

        self.irq_latch = 3;

        match self.command {
            0x1 => self.stat(),
            0x2 => self.set_loc(),
            0x6 => self.cd_read_command(scheduler),
            0x9 => self.pause(scheduler),
            0xe => self.set_mode(),
            0x15 => self.seek(scheduler),
            0x19 => self.commandx19(),
            0x1a => self.get_id(scheduler),
            0x1e => scheduler.schedule(EventType::CDGetTOC, 44100 * CDROM_CYCLES),
            _ => todo!("command byte 0x{:x}", self.command)
        }

        scheduler.schedule(EventType::CDResponseClear, 10 * CDROM_CYCLES);

        self.controller_param_fifo.clear();
    }

    pub fn cd_read_sector(&mut self, scheduler: &mut Scheduler) {
        if !self.is_reading {
            return;
        }

        let pointer = self.get_pointer();

        self.stat();

        // TODO: refactor this to allow for WASM builds to work as well
        if let Some(game_data) = &mut self.game_data {
            self.current_header = CDHeader::from_buf(&game_data[pointer + 0xc..pointer + 0x10]);

            if self.current_header.mode != CDMode::Mode2 {
                todo!("non mode 2 header")
            }
            self.subheader = CDSubheader::from_buf(&game_data[pointer + 0x10..pointer + 0x14]);

            if self.current_header.mm != self.current_msf.amm || self.current_header.ss != self.current_msf.ass || self.current_header.sect != self.current_msf.asect {
                panic!("mismatch between header and current msf");
            }

            match self.subheader.read_mode {
                CDReadMode::Data => self.read_data(),
                CDReadMode::Audio | CDReadMode::Video => todo!("read audio/video cds")
            }

            self.current_msf.asect += 1;

            if self.current_msf.asect >= 75 {
                self.current_msf.asect -= 75;
                self.current_msf.ass += 1;

                if self.current_msf.ass >= 60 {
                    self.current_msf.amm += 1;
                }
            }
        }
        if self.is_reading {
            scheduler.schedule(EventType::CDRead, if self.double_speed { CD_READ_CYCLES / 2 } else { CD_READ_CYCLES });
            }

    }

    fn read_data(&mut self) {
        assert!(self.irqs == 0);

        if let Some(game_data) = &self.game_data {
            let pointer = self.get_pointer();

            self.sector_buffer.copy_from_slice(&game_data[pointer..pointer + 0x930]);

            let mut val = 1 << 1; // bit 1 is always set to 1, "motor on"

            val |= (self.is_reading as u8) << 5;
            val |= (self.is_seeking as u8) << 6;
            val |= (self.is_playing as u8) << 7;

            self.result_fifo.push_back(val);

            self.irqs = 1;
        }
    }

    fn cd_read_command(&mut self, scheduler: &mut Scheduler) {
        if !self.pre_seek {
            // cd has been seeked and is able to be read
            self.is_reading = true;
            self.is_seeking = false;
            self.is_playing = false;

            scheduler.schedule(EventType::CDRead, if self.double_speed { CD_READ_CYCLES / 2 } else { CD_READ_CYCLES } );

        } else {
            self.is_seeking = true;
            self.is_reading = false;
            self.is_playing = false;
            // request a seek
            self.next_event = Some(EventType::CDRead);
            scheduler.schedule(EventType::CDSeek, 25 * CDROM_CYCLES);
        }

        self.stat();
    }

    fn get_pointer(&self) -> usize {
        return ((self.current_msf.amm * 60 + self.current_msf.ass) * 75 + self.current_msf.asect - 150) as usize * BYTES_PER_SECTOR
    }

    fn bcd_to_u8(value: u8) -> u8 {
        (value >> 4) * 10 + value & 0xf
    }

    pub fn cd_stat(&mut self, scheduler: &mut Scheduler) {
        assert!(self.irqs == 0);

        self.stat();

        self.irq_latch = 0x2;

        scheduler.schedule(EventType::CDResponseClear, 10 * CDROM_CYCLES);
    }

    pub fn seek_cd(&mut self, scheduler: &mut Scheduler) {
        let mut msf = Msf::new();

        self.pre_seek = false;

        msf.amm = self.msf.amm;
        msf.ass = self.msf.ass;
        msf.asect = self.msf.asect;

        self.current_msf = msf;

        if let Some(event) = self.next_event.take() {
            match event {
                EventType::CDStat => scheduler.schedule(event, 100 * CDROM_CYCLES),
                EventType::CDRead => scheduler.schedule(event, if self.double_speed { CD_READ_CYCLES / 2 } else { CD_READ_CYCLES }),
                _ => ()
            }
        }
    }

    fn seek(&mut self, scheduler: &mut Scheduler) {
        self.stat();

        self.is_playing = false;
        self.is_reading = false;
        self.is_seeking = true;

        self.next_event = Some(EventType::CDStat);

        scheduler.schedule(EventType::CDSeek, CDROM_CYCLES * 50);
    }

    fn set_loc(&mut self) {
        self.msf.amm = Self::bcd_to_u8(self.controller_param_fifo.pop_front().unwrap());
        self.msf.ass = Self::bcd_to_u8(self.controller_param_fifo.pop_front().unwrap());
        self.msf.asect = Self::bcd_to_u8(self.controller_param_fifo.pop_front().unwrap());

        self.pre_seek = true;
    }

    pub fn get_toc(&mut self, scheduler: &mut Scheduler) {
        self.irq_latch = 2;
        self.stat();

        scheduler.schedule(EventType::CDResponseClear, 10 * CDROM_CYCLES);
    }

    fn get_id(&mut self, scheduler: &mut Scheduler) {
        self.stat();

        scheduler.schedule(EventType::CDGetId, 50 * CDROM_CYCLES)
    }

    pub fn read_id(&mut self, scheduler: &mut Scheduler) {
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

        scheduler.schedule(EventType::CDResponseClear, 10 * CDROM_CYCLES);
    }

    pub fn clear_response(&mut self, scheduler: &mut Scheduler) {
        if !self.result_fifo.is_empty() {
            self.result_fifo.pop_back();

            scheduler.schedule(EventType::CDResponseClear, 10 * CDROM_CYCLES);
        } else {
            scheduler.schedule(EventType::CDResponseTransfer, 10 * CDROM_CYCLES);
        }
    }

    fn write_control(&mut self, value: u8) {
        self.irqs &= !(value & 0x1f);

        self.result_fifo.clear();

        if (value >> 6) & 1 == 1 {
            self.parameter_fifo.clear();
        }
    }

    pub fn write(&mut self, address: usize, value: u8) {
        match address {
            0x1f801803 => match self.bank {
                0 => {
                    if (value >> 7) & 1 == 1 {
                        self.reading_buffer = true;
                    } else {
                        self.buffer_index = 0;
                        self.output_buffer.copy_from_slice(&self.sector_buffer[..0x930]);
                    }
                }
                1 => self.write_control(value),
                _ => todo!("bank = {}", self.bank)
            }
            0x1f801801 => match self.bank {
                0 => {
                    self.command_latch = Some(value);
                }
                _ => todo!("bank = {}", self.bank)
            }
            0x1f801802 => match self.bank {
                0 => self.parameter_fifo.push_back(value),
                1 => self.hntmask.write(value),
                _ => todo!("bank = {}", self.bank)
            }
            _ => todo!("(cdrom) address: 0x{:x}", address)
        }
    }

    pub fn write_bank(&mut self, value: u8) {
        self.bank = (value & 0x3) as usize;
    }
}