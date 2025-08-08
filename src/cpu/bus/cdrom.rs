use std::collections::VecDeque;

use registers::HntmaskRegister;

pub mod registers;

pub struct CDRom {
    hntmask: HntmaskRegister,
    bank: usize,
    parameter_fifo: VecDeque<u8>,
    result_fifo: VecDeque<u8>,
    irqs: u8
}

impl CDRom {
    pub fn new() -> Self {
        Self {
            hntmask: HntmaskRegister::from_bits_retain(0),
            bank: 0,
            parameter_fifo: VecDeque::with_capacity(16),
            result_fifo: VecDeque::with_capacity(16),
            irqs: 0
        }
    }


    pub fn read(&self, address: usize) -> u8 {
        println!("stuck reading!");

        println!("hsts = 0x{:x}", self.read_hsts());
        match address {
            0x1f801800 => self.read_hsts(),
            _ => todo!("address: 0x{:x}", address)
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
        (self.bank as u8) | (!self.parameter_fifo.is_empty() as u8) << 3 | ((self.parameter_fifo.len() < 16) as u8) << 4 | (!self.result_fifo.is_empty() as u8) << 5
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
        let subcommand = self.parameter_fifo.pop_front().unwrap();

        match subcommand {
            0x20 => {
                // get date

                // PSX/PSone date in bye form
                let bytes = [0x99,0x2,0x1,0xC3];

                for byte in bytes {
                    self.result_fifo.push_back(byte);
                }


            }
            _ => todo!("subcommand = 0x{:x}", subcommand)
        }

        self.result_fifo.push_back(0);
        self.result_fifo.push_back(0);

        self.parameter_fifo.drain(..);
    }

    pub fn execute_command(&mut self, command_byte: u8) {
        match command_byte {
            0x19 => self.commandx19(),
            _ => todo!("command byte 0x{:x}", command_byte)
        }
    }

    fn write_control(&mut self, value: u8) {
        self.irqs &= !(value & 0x1f);

        if (value >> 6) & 1 == 1 {
            self.parameter_fifo.clear();
        }
    }

    pub fn write(&mut self, address: usize, value: u8) {
        println!("stuck here!");
        match address {
            0x1f801803 => match self.bank {
                1 => self.write_control(value),
                _ => todo!("bank = {}", self.bank)
            }
            0x1f801801 => match self.bank {
                0 => self.execute_command(value),
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