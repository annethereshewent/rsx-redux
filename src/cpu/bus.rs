use cdrom::CDRom;
use dma::dma::Dma;
use gpu::GPU;
use mdec::Mdec;
use registers::{delay_register::DelayRegister, interrupt_register::InterruptRegister};
use scheduler::Scheduler;
#[cfg(feature = "new_spu")]
use spu::SPU;
#[cfg(feature = "old_spu")]
use spu_legacy::SPU;
use timer::Timer;

use crate::cpu::bus::peripherals::Peripherals;

pub mod cdrom;
pub mod dma;
pub mod gpu;
pub mod mdec;
pub mod peripherals;
pub mod registers;
pub mod scheduler;
#[cfg(feature = "new_spu")]
pub mod spu;
#[cfg(feature = "old_spu")]
pub mod spu_legacy;
pub mod timer;

pub struct Bus {
    bios: Vec<u8>,
    bios_delay: DelayRegister,
    ram_size: u32,
    com_delay: u32,
    exp1_base_address: u32,
    exp2_base_address: u32,
    exp2_enabled: bool,
    exp1_delay: DelayRegister,
    spu_delay: DelayRegister,
    cdrom_delay: DelayRegister,
    exp3_delay: DelayRegister,
    exp2_delay: DelayRegister,
    cache_config: u32,
    pub(crate) main_ram: Box<[u8]>,
    scratchpad: Box<[u8]>,
    pub spu: SPU,
    exp1_post: u8,
    pub interrupt_mask: InterruptRegister,
    pub interrupt_stat: InterruptRegister,
    pub timers: [Timer; 3],
    pub scheduler: Scheduler,
    pub gpu: GPU,
    pub dma: Dma,
    pub cdrom: CDRom,
    pub mdec: Mdec,
    pub peripherals: Peripherals,
}

impl Bus {
    pub fn new() -> Self {
        let mut scheduler = Scheduler::new();
        Self {
            bios: Vec::new(),
            bios_delay: DelayRegister::new(),
            ram_size: 0,
            com_delay: 0,
            exp1_base_address: 0,
            exp2_base_address: 0,
            exp2_enabled: true,
            exp1_delay: DelayRegister::new(),
            spu_delay: DelayRegister::new(),
            cdrom_delay: DelayRegister::new(),
            exp3_delay: DelayRegister::new(),
            exp2_delay: DelayRegister::new(),
            cache_config: 0,
            main_ram: vec![0; 0x200000].into_boxed_slice(),
            spu: SPU::new(&mut scheduler),
            exp1_post: 0,
            interrupt_mask: InterruptRegister::from_bits_truncate(0),
            interrupt_stat: InterruptRegister::from_bits_truncate(0),
            timers: [Timer::new(0), Timer::new(1), Timer::new(2)],
            gpu: GPU::new(&mut scheduler),
            cdrom: CDRom::new(&mut scheduler),
            scheduler,
            dma: Dma::new(),
            mdec: Mdec::new(),
            scratchpad: vec![0; 0x400].into_boxed_slice(),
            peripherals: Peripherals::new(),
        }
    }

    pub fn load_bios(&mut self, bios: Vec<u8>) {
        self.bios = bios;
    }

    pub fn translate_address(address: u32) -> usize {
        match address >> 28 {
            0x8 | 0xa => (address & 0xfffffff) as usize,
            0xf => address as usize,
            _ => (address & 0x1fffffff) as usize,
        }
    }

    pub fn mem_read32(&mut self, address: u32) -> u32 {
        let address = Self::translate_address(address);

        match address {
            0x00000000..=0x001fffff => unsafe {
                *(&self.main_ram[address] as *const u8 as *const u32)
            },
            0x1f800000..=0x1f8003ff => unsafe {
                *(&self.scratchpad[address - 0x1f800000] as *const u8 as *const u32)
            },
            0x1f801014 => self.spu_delay.read(),
            0x1f801060 => self.ram_size,
            0x1f801070 => self.interrupt_stat.bits(),
            0x1f801074 => self.interrupt_mask.bits(),
            0x1f801080..=0x1f8010f4 => {
                self.scheduler.tick(5);
                self.dma.read_registers(address)
            }
            0x1f801110 => self.timers[1].counter,
            0x1f801810 => {
                self.scheduler.tick(5);
                self.gpu.read_gpu()
            }
            0x1f801814 => {
                self.scheduler.tick(5);
                self.gpu.read_stat()
            }
            0x1f801820..=0x1f801824 => {
                self.scheduler.tick(5);
                self.mdec.read(address)
            }
            0x1fc00000..=0x1fc80000 => {
                if (self.cache_config >> 11) & 1 == 0 {
                    self.scheduler.tick(4);
                }

                unsafe { *(&self.bios[address - 0x1fc00000] as *const u8 as *const u32) }
            }
            _ => todo!("(mem_read32) address: 0x{:x}", address),
        }
    }

    pub fn mem_read16(&mut self, address: u32) -> u32 {
        let address = Self::translate_address(address);

        match address {
            0x00000000..=0x001fffff => unsafe {
                *(&self.main_ram[address] as *const u8 as *const u16) as u32
            },
            0x1f800000..=0x1f8003ff => unsafe {
                *(&self.scratchpad[address - 0x1f800000] as *const u8 as *const u16) as u32
            },
            0x1f801044 => self.peripherals.read_stat() as u32,
            0x1f80104a => self.peripherals.read_ctrl() as u32,
            0x1f801070 => {
                self.scheduler.tick(5);
                self.interrupt_stat.bits() & 0xffff
            }
            0x1f801072 => {
                self.scheduler.tick(5);
                (self.interrupt_stat.bits() >> 16) & 0xffff
            }
            0x1f801074 => {
                self.scheduler.tick(5);
                self.interrupt_mask.bits() & 0xffff
            }
            0x1f801076 => {
                self.scheduler.tick(5);
                self.interrupt_mask.bits() >> 16
            }
            0x1f801120 => self.timers[2].counter,
            0x1f801c00..=0x1f801e7f => {
                self.scheduler.tick(5);
                self.spu.read16(address) as u32
            }
            _ => todo!("(mem_read16) address: 0x{:x}", address),
        }
    }

    pub fn mem_read8(&mut self, address: u32) -> u32 {
        let address = Self::translate_address(address);

        match address {
            0x00000000..=0x001fffff => self.main_ram[address] as u32,
            0x1f800000..=0x1f8003ff => self.scratchpad[address - 0x1f800000] as u32,
            0x1f801040 => self.peripherals.read_byte() as u32,
            0x1f801800..=0x1f801803 => {
                self.scheduler.tick(5);
                self.cdrom.read(address) as u32
            }
            0x1f000000..=0x1f02ffff => 0, // expansion 1 I/O, not needed
            0x1fc00000..=0x1fc80000 => {
                if (self.cache_config >> 1) & 1 == 0 {
                    self.scheduler.tick(4);
                }
                self.bios[address - 0x1fc00000] as u32
            }
            _ => todo!("(mem_read8) address 0x{:x}", address),
        }
    }

    pub fn mem_write32(&mut self, address: u32, value: u32) {
        let address = Self::translate_address(address);

        if (0x1f801000..=0x1f802000).contains(&address) {
            self.scheduler.tick(5);
        }

        match address {
            0x00000000..=0x001fffff => unsafe {
                *(&mut self.main_ram[address] as *mut u8 as *mut u32) = value
            },
            0x1f800000..=0x1f8003ff => unsafe {
                *(&mut self.scratchpad[address - 0x1f800000] as *mut u8 as *mut u32) = value
            },
            0x1f801000 => self.exp1_base_address = value & 0xffffff | (0x1f << 24), // TODO: implement
            0x1f801004 => {
                self.exp2_base_address = value & 0xffffff | (0x1f << 24);
                self.exp2_enabled = self.exp2_base_address == 0x1f802000;
            }
            0x1f801008 => self.exp1_delay.write(value), // TODO
            0x1f80100c => self.exp3_delay.write(value), // TODO
            0x1f801010 => self.bios_delay.write(value), // TODO
            0x1f801014 => self.spu_delay.write(value),  // TODO
            0x1f801018 => self.cdrom_delay.write(value), // TODO
            0x1f80101c => self.exp2_delay.write(value),
            0x1f801020 => self.com_delay = value & 0xffff, // TODO: actually implement
            0x1f801060 => self.ram_size = value,           // TODO: actually implement lmao
            0x1f801070 => {
                let new_stat = self.interrupt_stat.bits() & value;
                self.interrupt_stat = InterruptRegister::from_bits_retain(new_stat);
            }
            0x1f801074 => self.interrupt_mask = InterruptRegister::from_bits_truncate(value),
            0x1f801080..=0x1f8010f4 => self.dma.write_registers(
                address,
                value,
                &mut self.scheduler,
                &mut self.main_ram,
                &mut self.gpu,
                &mut self.spu,
                &mut self.cdrom,
                &mut self.mdec,
                &mut self.interrupt_stat,
            ),
            0x1f801114 => self.timers[1].write_counter_register(value as u16),
            0x1f801118 => self.timers[1].counter_target = value as u16,
            0x1f801810 => self.gpu.process_gp0_commands(value),
            0x1f801814 => self.gpu.process_gp1_commands(value),
            0x1f801820..=0x1f801824 => self.mdec.write(address, value),
            0xfffe0130 => {
                self.cache_config = value;
                self.cache_config &= !((1 << 6) | (1 << 10));
            }
            _ => todo!("(mem_write32) address: 0x{:x}", address),
        }
    }

    pub fn mem_write16(&mut self, address: u32, value: u16) {
        let address = Self::translate_address(address);

        if (0x1f801000..=0x1f802000).contains(&address) {
            self.scheduler.tick(5);
        }

        match address {
            0x00000000..=0x001fffff => unsafe {
                *(&mut self.main_ram[address] as *mut u8 as *mut u16) = value
            },
            0x1f800000..=0x1f8003ff => unsafe {
                *(&mut self.scratchpad[address - 0x1f800000] as *mut u8 as *mut u16) = value
            },
            0x1f801048 => self.peripherals.write_mode(value),
            0x1f80104a => self.peripherals.write_ctrl(value, &mut self.scheduler),
            0x1f80104e => self.peripherals.write_reload_rate(value),
            0x1f801070 => {
                let new_stat = self.interrupt_stat.bits() & value as u32;
                self.interrupt_stat = InterruptRegister::from_bits_retain(new_stat);
            }
            0x1f801074 => {
                self.interrupt_mask = InterruptRegister::from_bits_retain(
                    (self.interrupt_mask.bits() & 0xffff0000) | value as u32,
                )
            }
            0x1f801076 => {
                self.interrupt_mask = InterruptRegister::from_bits_retain(
                    (self.interrupt_mask.bits() & 0xffff) | (value as u32) << 16,
                )
            }
            0x1f801100 => self.timers[0].write_counter(value as u32),
            0x1f801104 => self.timers[0].write_counter_register(value),
            0x1f801108 => self.timers[0].counter_target = value,
            0x1f801110 => self.timers[1].write_counter(value as u32),
            0x1f801114 => self.timers[1].write_counter_register(value),
            0x1f801118 => self.timers[1].counter_target = value,
            0x1f801120 => self.timers[2].write_counter(value as u32),
            0x1f801124 => self.timers[2].write_counter_register(value),
            0x1f801128 => self.timers[2].counter_target = value,
            #[cfg(feature = "old_spu")]
            0x1f801c00..=0x1f801e7f => self.spu.write16(address, value),
            #[cfg(feature = "new_spu")]
            0x1f801c00..=0x1f801e7f => self.spu.write16(address, value, &mut self.interrupt_stat),
            _ => todo!("(mem_write16) address: 0x{:x}", address),
        }
    }

    pub fn mem_write8(&mut self, address: u32, value: u8) {
        let address = Self::translate_address(address);

        match address {
            0x00000000..=0x001fffff => self.main_ram[address] = value,
            0x1f800000..=0x1f8003ff => self.scratchpad[address - 0x1f800000] = value,
            0x1f801040 => self.peripherals.write_byte(value, &mut self.scheduler),
            0x1f801800 => {
                self.scheduler.tick(5);
                self.cdrom.write_bank(value);
            }
            0x1f801801..=0x1f801803 => {
                self.scheduler.tick(5);
                self.cdrom.write(address, value);
            }
            0x1f802041 => {
                self.scheduler.tick(5);
                self.exp1_post = value;
            }
            _ => todo!("(mem_write8) address: 0x{:x}", address),
        }
    }
}
