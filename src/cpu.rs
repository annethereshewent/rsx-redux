use std::{
    collections::HashSet,
    fs,
    ops::{Index, IndexMut},
    sync::Arc,
};

use bus::{Bus, scheduler::EventType};
use cop0::{COP0, CauseRegister, StatusRegister};
use gte::Gte;
use instructions::Instruction;
use ringbuf::{SharedRb, storage::Heap, wrap::caching::Caching};

pub mod bus;
pub mod cop0;
pub mod disassembler;
pub mod gte;
pub mod instructions;

pub const RA_REGISTER: usize = 31;

#[derive(Copy, Clone)]
pub struct Registers([u32; 32]);

impl Index<usize> for Registers {
    type Output = u32;
    fn index(&self, idx: usize) -> &Self::Output {
        &self.0[idx]
    }
}

impl IndexMut<usize> for Registers {
    fn index_mut(&mut self, idx: usize) -> &mut Self::Output {
        if idx == 0 {
            // Always hand out a dummy zero reference for $zero.
            // Returning &mut 0 would be UB, so we point into slot 0
            // but also force it back to 0 whenever it's accessed.
            self.0[0] = 0;
        }
        &mut self.0[idx]
    }
}

#[derive(Copy, Clone)]
pub enum ExceptionType {
    Interrupt = 0x0,
    LoadAddressError = 0x4,
    StoreAddressError = 0x5,
    Syscall = 0x8,
    Break = 0x9,
    Overflow = 0xc,
}

pub struct CPU {
    r: Registers,
    delayed_load: Option<(usize, u32)>,
    pc: u32,
    previous_pc: u32,
    next_pc: u32,
    hi: u32,
    lo: u32,
    pub bus: Bus,
    instructions: [fn(&mut CPU, Instruction) -> usize; 0x40],
    special_instructions: [fn(&mut CPU, Instruction) -> usize; 0x40],
    cop0: COP0,
    gte: Gte,
    found: HashSet<u32>,
    pub debug_on: bool,
    ignored_load_delay: Option<usize>,
    branch_taken: bool,
    in_delay_slot: bool,
    output: String,
    exe_file: Option<String>,
    should_transfer_load: bool,
}

impl CPU {
    pub fn new(
        producer: Caching<Arc<SharedRb<Heap<f32>>>, true, false>,
        exe_file: Option<String>,
    ) -> Self {
        /*
        00h=SPECIAL 08h=ADDI  10h=COP0 18h=N/A   20h=LB   28h=SB   30h=LWC0 38h=SWC0
        01h=BcondZ  09h=ADDIU 11h=COP1 19h=N/A   21h=LH   29h=SH   31h=LWC1 39h=SWC1
        02h=J       0Ah=SLTI  12h=COP2 1Ah=N/A   22h=LWL  2Ah=SWL  32h=LWC2 3Ah=SWC2
        03h=JAL     0Bh=SLTIU 13h=COP3 1Bh=N/A   23h=LW   2Bh=SW   33h=LWC3 3Bh=SWC3
        04h=BEQ     0Ch=ANDI  14h=N/A  1Ch=N/A   24h=LBU  2Ch=N/A  34h=N/A  3Ch=N/A
        05h=BNE     0Dh=ORI   15h=N/A  1Dh=N/A   25h=LHU  2Dh=N/A  35h=N/A  3Dh=N/A
        06h=BLEZ    0Eh=XORI  16h=N/A  1Eh=N/A   26h=LWR  2Eh=SWR  36h=N/A  3Eh=N/A
        07h=BGTZ    0Fh=LUI   17h=N/A  1Fh=N/A   27h=N/A  2Fh=N/A  37h=N/A  3Fh=N/A
        */
        let instructions = [
            CPU::reserved, // 0
            CPU::bcondz,   // 1
            CPU::j,        // 2
            CPU::jal,      // 3
            CPU::beq,      // 4
            CPU::bne,      // 5
            CPU::blez,     // 6
            CPU::bgtz,     // 7
            CPU::addi,     // 8
            CPU::addiu,    // 9
            CPU::slti,     // a
            CPU::sltiu,    // b
            CPU::andi,     // c
            CPU::ori,      // d
            CPU::xori,     // e
            CPU::lui,      // f
            CPU::cop0,     // 10
            CPU::cop1,     // 11
            CPU::cop2,     // 12
            CPU::cop3,     // 13
            CPU::reserved, // 14
            CPU::reserved, // 15
            CPU::reserved, // 16
            CPU::reserved, // 17
            CPU::reserved, // 18
            CPU::reserved, // 19
            CPU::reserved, // 1a
            CPU::reserved, // 1b
            CPU::reserved, // 1c
            CPU::reserved, // 1d
            CPU::reserved, // 1e
            CPU::reserved, // 1f
            CPU::lb,       // 20
            CPU::lh,       // 21,
            CPU::lwl,      // 22
            CPU::lw,       // 23
            CPU::lbu,      // 24
            CPU::lhu,      // 25
            CPU::lwr,      // 26
            CPU::reserved, // 27
            CPU::sb,       // 28
            CPU::sh,       // 29
            CPU::swl,      // 2a
            CPU::sw,       // 2b
            CPU::reserved, // 2c
            CPU::reserved, // 2d
            CPU::swr,      // 2e
            CPU::reserved, // 2f
            CPU::lwc0,     // 30
            CPU::lwc1,     // 31
            CPU::lwc2,     // 32
            CPU::lwc3,     // 33
            CPU::reserved, // 34
            CPU::reserved, // 35
            CPU::reserved, // 36
            CPU::reserved, // 37
            CPU::swc0,     // 38
            CPU::swc1,     // 39
            CPU::swc2,     // 3a
            CPU::swc3,     // 3b
            CPU::reserved, // 3c
            CPU::reserved, // 3d
            CPU::reserved, // 3e
            CPU::reserved, // 3f
        ];

        /*
        00h=SLL   08h=JR      10h=MFHI 18h=MULT  20h=ADD  28h=N/A  30h=N/A  38h=N/A
        01h=N/A   09h=JALR    11h=MTHI 19h=MULTU 21h=ADDU 29h=N/A  31h=N/A  39h=N/A
        02h=SRL   0Ah=N/A     12h=MFLO 1Ah=DIV   22h=SUB  2Ah=SLT  32h=N/A  3Ah=N/A
        03h=SRA   0Bh=N/A     13h=MTLO 1Bh=DIVU  23h=SUBU 2Bh=SLTU 33h=N/A  3Bh=N/A
        04h=SLLV  0Ch=SYSCALL 14h=N/A  1Ch=N/A   24h=AND  2Ch=N/A  34h=N/A  3Ch=N/A
        05h=N/A   0Dh=BREAK   15h=N/A  1Dh=N/A   25h=OR   2Dh=N/A  35h=N/A  3Dh=N/A
        06h=SRLV  0Eh=N/A     16h=N/A  1Eh=N/A   26h=XOR  2Eh=N/A  36h=N/A  3Eh=N/A
        07h=SRAV  0Fh=N/A     17h=N/A  1Fh=N/A   27h=NOR  2Fh=N/A  37h=N/A  3Fh=N/A
        */
        let special_instructions = [
            CPU::sll,      // 0
            CPU::reserved, // 1
            CPU::srl,      // 2
            CPU::sra,      // 3
            CPU::sllv,     // 4
            CPU::reserved, // 5
            CPU::srlv,     // 6
            CPU::srav,     // 7
            CPU::jr,       // 8
            CPU::jalr,     // 9
            CPU::reserved, // a
            CPU::reserved, // b
            CPU::syscall,  // c
            CPU::break_,   // d
            CPU::reserved, // e
            CPU::reserved, // f
            CPU::mfhi,     // 10
            CPU::mthi,     // 11
            CPU::mflo,     // 12
            CPU::mtlo,     // 13
            CPU::reserved, // 14
            CPU::reserved, // 15
            CPU::reserved, // 16
            CPU::reserved, // 17
            CPU::mult,     // 18
            CPU::multu,    // 19
            CPU::div,      // 1a
            CPU::divu,     // 1b
            CPU::reserved, // 1c
            CPU::reserved, // 1d,
            CPU::reserved, // 1e,
            CPU::reserved, // 1f
            CPU::add,      // 20
            CPU::addu,     // 21
            CPU::sub,      // 22
            CPU::subu,     // 23
            CPU::and,      // 24
            CPU::or,       // 25
            CPU::xor,      // 26
            CPU::nor,      // 27
            CPU::reserved, // 28
            CPU::reserved, // 29
            CPU::slt,      // 2a
            CPU::sltu,     // 2b
            CPU::reserved,
            CPU::reserved,
            CPU::reserved,
            CPU::reserved,
            CPU::reserved,
            CPU::reserved,
            CPU::reserved,
            CPU::reserved,
            CPU::reserved,
            CPU::reserved,
            CPU::reserved,
            CPU::reserved,
            CPU::reserved,
            CPU::reserved,
            CPU::reserved,
            CPU::reserved,
            CPU::reserved,
            CPU::reserved,
            CPU::reserved,
            CPU::reserved,
        ];

        Self {
            r: Registers([0; 32]),
            pc: 0xbfc00000,
            previous_pc: 0xbfc00000,
            next_pc: 0xbfc00004,
            hi: 0,
            lo: 0,
            bus: Bus::new(producer),
            instructions,
            special_instructions,
            delayed_load: None,
            cop0: COP0::new(),
            found: HashSet::new(),
            debug_on: false,
            ignored_load_delay: None,
            in_delay_slot: false,
            branch_taken: false,
            output: "".to_string(),
            gte: Gte::new(),
            exe_file,
            should_transfer_load: false,
        }
    }

    pub fn tick(&mut self, cycles: usize) {
        if self.bus.timers[0].is_active {
            self.bus.timers[0].tick(
                cycles,
                &mut self.bus.scheduler,
                &mut self.bus.interrupt_stat,
            );
        }
        if self.bus.timers[1].is_active {
            self.bus.timers[1].tick(
                cycles,
                &mut self.bus.scheduler,
                &mut self.bus.interrupt_stat,
            );
        }

        self.bus.scheduler.tick(cycles);
    }

    fn handle_interrupts(&mut self) {
        let interrupts = self.bus.interrupt_mask.bits() & self.bus.interrupt_stat.bits();

        if interrupts != 0 {
            self.cop0.cause = CauseRegister::from_bits_retain(self.cop0.cause.bits() | 1 << 10);
        } else {
            self.cop0.cause = CauseRegister::from_bits_retain(self.cop0.cause.bits() & !(1 << 10));
        }
    }

    fn check_irqs(&self) -> bool {
        let mask = ((self.cop0.sr.bits() >> 8) as u8) & ((self.cop0.cause.bits() >> 8) as u8);

        mask != 0 && self.cop0.sr.contains(StatusRegister::IEC)
    }

    pub fn store8(&mut self, address: u32, value: u8) {
        if self.cop0.sr.contains(StatusRegister::ISOLATE_CACHE) {
            // TODO: implement this but for real
            return;
        }

        self.bus.mem_write8(address, value);
    }

    pub fn store16(&mut self, address: u32, value: u16) {
        if self.cop0.sr.contains(StatusRegister::ISOLATE_CACHE) {
            // TODO: implement this but for real
            return;
        }

        self.bus.mem_write16(address, value);
    }

    fn transfer_load(&mut self) {
        if let Some((index, value)) = self.delayed_load {
            if let Some(ignored_load_delay) = self.ignored_load_delay.take() {
                if ignored_load_delay != index {
                    self.r[index] = value;
                }
            } else {
                self.r[index] = value;
            }
        }

        self.ignored_load_delay = None;
        self.delayed_load = None;
    }

    pub fn store32(&mut self, address: u32, value: u32) {
        if self.cop0.sr.contains(StatusRegister::ISOLATE_CACHE) {
            // TODO: implement this but for real
            return;
        }

        self.bus.mem_write32(address, value);
    }

    pub fn step_frame(&mut self) {
        while !self.bus.gpu.frame_finished {
            self.step();
        }

        self.bus.gpu.frame_finished = false;
    }

    pub fn load_exe(&mut self, filename: &str) {
        let bytes = fs::read(filename).unwrap();

        let mut index = 0x10;

        self.pc = unsafe { *(&bytes[index] as *const u8 as *const u32) };
        self.next_pc = self.pc + 4;

        index += 4;

        self.r[28] = unsafe { *(&bytes[index] as *const u8 as *const u32) };

        index += 4;

        let file_dest = unsafe { *(&bytes[index] as *const u8 as *const u32) };

        index += 4;

        // let file_size = util::read_word(&bytes, index);
        let file_size = unsafe { *(&bytes[index] as *const u8 as *const u32) };

        index += 0x10 + 4;

        // let sp_base = util::read_word(&bytes, index);
        let sp_base = unsafe { *(&bytes[index] as *const u8 as *const u32) };

        index += 4;

        if sp_base != 0 {
            // let sp_offset = util::read_word(&bytes, index);
            let sp_offset = unsafe { *(&bytes[index] as *const u8 as *const u32) };

            self.r[29] = sp_base + sp_offset;
            self.r[30] = self.r[29];
        }

        index = 0x800;

        for i in 0..file_size {
            self.bus.main_ram[((file_dest + i) & 0x1f_ffff) as usize] = bytes[index];
            index += 1;
        }
    }

    pub fn step(&mut self) {
        self.r[0] = 0;

        self.handle_interrupts();

        self.should_transfer_load = self.delayed_load.is_some();
        self.ignored_load_delay = None;

        self.in_delay_slot = self.branch_taken;
        self.cop0.cause.set(CauseRegister::BT, self.in_delay_slot);
        self.branch_taken = false;
        self.cop0.cause.remove(CauseRegister::BD);

        if self.pc & 0x3 != 0 {
            self.cop0.bad_addr = self.pc;
            self.enter_exception(ExceptionType::LoadAddressError);

            if self.should_transfer_load {
                self.transfer_load();
            }

            self.should_transfer_load = false;

            return;
        }

        let opcode = self.bus.mem_read32(self.pc);

        if self.check_irqs() {
            self.enter_exception(ExceptionType::Interrupt);

            if (opcode >> 25) == 0x25 {
                self.gte.execute_command(Instruction(opcode));
            }

            if self.should_transfer_load {
                self.transfer_load();
            }

            self.should_transfer_load = false;

            return;
        }

        self.update_tty();

        self.previous_pc = self.pc;

        if self.previous_pc == 0x80030000 {
            if let Some(exe_file) = &self.exe_file {
                let exe_file = exe_file.clone();
                self.load_exe(exe_file.as_str());
            }
        }

        self.pc = self.next_pc;

        if !self.found.contains(&self.previous_pc) && self.debug_on {
            println!(
                "[Opcode: 0x{:x}] [PC: 0x{:x}] {}",
                opcode,
                self.previous_pc,
                self.disassemble(opcode)
            );
            self.found.insert(self.previous_pc);
        }

        self.next_pc += 4;

        let cycles = self.decode_opcode(opcode);

        self.tick(cycles);

        self.handle_events();

        if self.should_transfer_load {
            self.transfer_load();
        }

        self.should_transfer_load = false;
    }

    fn handle_events(&mut self) {
        if let Some((event, cycles_left)) = self.bus.scheduler.get_next_event() {
            match event {
                EventType::Vblank => self.bus.gpu.handle_vblank(
                    &mut self.bus.scheduler,
                    &mut self.bus.interrupt_stat,
                    &mut self.bus.timers,
                    cycles_left,
                ),
                EventType::HblankStart => self.bus.gpu.handle_hblank_start(
                    &mut self.bus.scheduler,
                    &mut self.bus.timers,
                    cycles_left,
                ),
                EventType::HblankEnd => self.bus.gpu.handle_hblank(
                    &mut self.bus.scheduler,
                    &mut self.bus.interrupt_stat,
                    &mut self.bus.timers,
                    cycles_left,
                ),
                EventType::DmaFinished(channel) => self
                    .bus
                    .dma
                    .finish_transfer(channel, &mut self.bus.interrupt_stat),
                EventType::CDExecuteCommand => {
                    self.bus.cdrom.execute_command(&mut self.bus.scheduler)
                }
                EventType::CDLatchInterrupts => {
                    self.bus.cdrom.transfer_interrupts(&mut self.bus.scheduler)
                }
                EventType::CDCheckCommands => {
                    self.bus.cdrom.check_commands(&mut self.bus.scheduler)
                }
                EventType::CDCommandTransfer => {
                    self.bus.cdrom.transfer_command(&mut self.bus.scheduler)
                }
                EventType::CDParamTransfer => {
                    self.bus.cdrom.transfer_params(&mut self.bus.scheduler)
                }
                EventType::CDResponseTransfer => {
                    self.bus.cdrom.transfer_response(&mut self.bus.scheduler)
                }
                EventType::CDResponseClear => {
                    self.bus.cdrom.clear_response(&mut self.bus.scheduler)
                }
                EventType::Timer(timer_id) => self.bus.timers[timer_id]
                    .on_overflow_or_target(&mut self.bus.scheduler, &mut self.bus.interrupt_stat),
                EventType::CDCheckIrqs => self
                    .bus
                    .cdrom
                    .process_irqs(&mut self.bus.scheduler, &mut self.bus.interrupt_stat),
                EventType::CDGetId => self.bus.cdrom.read_id(&mut self.bus.scheduler),
                EventType::CDGetTOC => self.bus.cdrom.get_toc(&mut self.bus.scheduler),
                EventType::CDSeek => self.bus.cdrom.seek_cd(&mut self.bus.scheduler),
                EventType::CDStat => self.bus.cdrom.cd_stat(&mut self.bus.scheduler),
                EventType::CDRead => self.bus.cdrom.cd_read_sector(&mut self.bus.scheduler),
                EventType::TickSpu => self
                    .bus
                    .spu
                    .tick(&mut self.bus.interrupt_stat, &mut self.bus.scheduler),
            }
        }
    }

    fn update_tty(&mut self) {
        if self.pc == 0xb0 && self.r[9] == 0x3d {
            let mut buf: Vec<u8> = Vec::new();

            buf.push(self.r[4] as u8);
            buf.push((self.r[4] >> 8) as u8);
            buf.push((self.r[4] >> 16) as u8);
            buf.push((self.r[4] >> 24) as u8);

            self.output += &String::from_utf8(buf).unwrap();

            if self.output.contains("\n") {
                print!("{}", self.output);
                self.output = "".to_string();
            }
        }
    }

    pub fn enter_exception(&mut self, exception_type: ExceptionType) {
        let exception_cause = exception_type as u32;
        self.cop0.cause.write_exception_code(exception_cause);

        let mut sr_bits = self.cop0.sr.bits();
        let mode = sr_bits & 0x3f;
        sr_bits &= !0x3f;
        sr_bits |= (mode << 2) & 0x3f;

        let mut cause_bits = self.cop0.cause.bits();

        cause_bits &= !0x7c;
        cause_bits |= (exception_type as u32) << 2;

        self.cop0.cause = CauseRegister::from_bits_retain(cause_bits);

        self.cop0.sr = StatusRegister::from_bits_retain(sr_bits);

        self.cop0.epc = match exception_type {
            ExceptionType::Interrupt => self.pc,
            _ => self.previous_pc,
        };

        if self.in_delay_slot {
            self.cop0.epc -= 4;
            self.cop0.cause = CauseRegister::from_bits_retain(self.cop0.cause.bits() | 1 << 31);

            self.cop0.tar = self.pc;
        } else {
            self.cop0.cause = CauseRegister::from_bits_retain(self.cop0.cause.bits() & !(1 << 31));
        }

        self.pc = if self.cop0.sr.contains(StatusRegister::BEV) {
            0xbfc00180
        } else {
            0x80000080
        };
        self.next_pc = self.pc + 4;
    }
}
