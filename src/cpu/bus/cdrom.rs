use std::collections::VecDeque;

use registers::HntmaskRegister;

use super::{registers::interrupt_register::InterruptRegister, scheduler::{EventType, Scheduler}};

pub mod registers;

// TODO: use actual numbers instead of these placeholder values.
pub const CDROM_CYCLES: usize = 768;

#[derive(Copy, Clone)]
pub enum CDStatus {
    Idle,
    Seek,
    Read,
    Play,
    GetStat
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
    status: CDStatus,
    controller_status: ControllerStatus,
    command_latch: Option<u8>,
    command: u8,
    is_playing: bool,
    is_seeking: bool,
    is_reading: bool,
    amm: u8,
    ass: u8,
    asect: u8,
    current_amm: u8,
    current_ass: u8,
    current_asect: u8,
    next_event: Option<EventType>
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
            status: CDStatus::Idle,
            controller_status: ControllerStatus::Idle,
            irqs: 0,
            controller_param_fifo: VecDeque::with_capacity(16),
            command: 0,
            command_latch: None,
            controller_response_fifo: VecDeque::with_capacity(16),
            is_playing: false,
            is_reading: false,
            is_seeking: false,
            amm: 0,
            ass: 0,
            asect: 0,
            current_amm: 0,
            current_asect: 0,
            current_ass: 0,
            next_event: None
        }
    }


    pub fn transfer_response(&mut self, scheduler: &mut Scheduler, interrupt_register: &mut InterruptRegister) {
        if self.result_fifo.len() < 16 && self.controller_response_fifo.len() > 0 {
            let value = self.controller_response_fifo.pop_front().unwrap();
            self.result_fifo.push_back(value);

            scheduler.schedule(EventType::CDResponseTransfer, 10 * CDROM_CYCLES);
        } else {
            scheduler.schedule(EventType::CDLatchInterrupts, 10 * CDROM_CYCLES);
        }
    }
    fn read_hintsts(&self) -> u8 {
        self.irqs | 0x7 << 5
    }

    fn read_response(&mut self) -> u8 {
        if self.result_fifo.is_empty() { 0 } else { self.result_fifo.pop_front().unwrap() }
    }

    pub fn read(&mut self, address: usize) -> u8 {
        match address {
            0x1f801800 => self.read_hsts(),
            0x1f801801 => self.read_response(),
            0x1f801803 => match self.bank {
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

    pub fn check_commands(&mut self, scheduler: &mut Scheduler, interrupt_register: &mut InterruptRegister) {
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

    pub fn transfer_command(&mut self, scheduler: &mut Scheduler, interrupt_register: &mut InterruptRegister) {
        self.command = self.command_latch.take().unwrap();

        scheduler.schedule(EventType::CDExecuteCommand, CDROM_CYCLES * 10);
    }

    pub fn transfer_params(&mut self, scheduler: &mut Scheduler, interrupt_register: &mut InterruptRegister) {
        if !self.parameter_fifo.is_empty() {
            let byte = self.parameter_fifo.pop_front().unwrap();

            self.controller_param_fifo.push_back(byte);

            scheduler.schedule(EventType::CDParamTransfer, CDROM_CYCLES * 10);
        } else {
            scheduler.schedule(EventType::CDCommandTransfer, CDROM_CYCLES * 10);
        }
    }

    pub fn transfer_interrupts(&mut self, scheduler: &mut Scheduler, interrupt_register: &mut InterruptRegister) {
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

    pub fn execute_command(&mut self, scheduler: &mut Scheduler) {
        self.controller_response_fifo.clear();

        self.irq_latch = 3;

        match self.command {
            0x1 => self.stat(),
            0x2 => self.set_loc(),
            0x15 => self.seek(scheduler),
            0x19 => self.commandx19(),
            0x1a => self.get_id(scheduler),
            0x1e => scheduler.schedule(EventType::CDGetTOC, 44100 * CDROM_CYCLES),
            _ => todo!("command byte 0x{:x}", self.command)
        }

        scheduler.schedule(EventType::CDResponseClear, 10 * CDROM_CYCLES);

        self.controller_param_fifo.clear();
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
        self.current_amm = self.amm;
        self.current_ass = self.ass;
        self.current_asect = self.asect;

        if let Some(event) = self.next_event.take() {
            match event {
                EventType::CDStat => scheduler.schedule(event, 10 * CDROM_CYCLES),
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
        self.amm = Self::bcd_to_u8(self.controller_param_fifo.pop_front().unwrap());
        self.ass = Self::bcd_to_u8(self.controller_param_fifo.pop_front().unwrap());
        self.asect = Self::bcd_to_u8(self.controller_param_fifo.pop_front().unwrap());
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

        for byte in bytes {
            self.controller_response_fifo.push_back(*byte);
        }

        scheduler.schedule(EventType::CDResponseClear, 10 * CDROM_CYCLES);
    }

    pub fn clear_response(&mut self, scheduler: &mut Scheduler, interrupt_register: &mut InterruptRegister) {
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