use std::collections::HashSet;

use bitflags::bitflags;
use bus::Bus;
use cop0::COP0;
use instructions::Instruction;

pub mod bus;
pub mod instructions;
pub mod disassembler;
pub mod cop0;

pub const RA_REGISTER: usize = 31;

bitflags! {
    pub struct StatusRegister: u32 {
        const IEC = 1 << 0;
        const KUC = 1 << 1;
        const IEP = 1 << 2;
        const KUP = 1 << 3;
        const IEO = 1 << 4;
        const KUO = 1 << 5;
        const ISOLATE_CACHE = 1 << 16;
        const SWC = 1 << 17;
        const PZ = 1 << 18;
        const CM = 1 << 19;
        const PE = 1 << 20;
        const BEV = 1 << 22;
        const COP0_ENABLE = 1 << 28;
        const GTE_ENABLE = 1 << 30;
    }
}

pub struct CPU {
    r: [u32; 32],
    delayed_load: Option<(usize, u32)>,
    pc: u32,
    previous_pc: u32,
    next_pc: u32,
    hi: u32,
    lo: u32,
    pub bus: Bus,
    instructions: [fn(&mut CPU, Instruction); 0x40],
    special_instructions: [fn(&mut CPU, Instruction); 0x40],
    cop0: COP0,
    found: HashSet<u32>,
    debug_on: bool,
    ignored_load_delay: Option<usize>,
    cycles: usize
}

impl CPU {
    pub fn new() -> Self {
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
            CPU::bcondz, // 1
            CPU::j, // 2
            CPU::jal, // 3
            CPU::beq, // 4
            CPU::bne, // 5
            CPU::blez, // 6
            CPU::bgtz, // 7
            CPU::addi, // 8
            CPU::addiu, // 9
            CPU::slti, // a
            CPU::sltiu, // b
            CPU::andi, // c
            CPU::ori, // d
            CPU::xori, // e
            CPU::lui, // f
            CPU::cop0, // 10
            CPU::cop1, // 11
            CPU::cop2, // 12
            CPU::cop3, // 13
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
            CPU::lb, // 20
            CPU::lh, // 21,
            CPU::lwl, // 22
            CPU::lw, // 23
            CPU::lbu, // 24
            CPU::lhu, // 25
            CPU::lwr, // 26
            CPU::reserved, // 27
            CPU::sb, // 28
            CPU::sh, // 29
            CPU::swl, // 2a
            CPU::sw, // 2b
            CPU::reserved, // 2c
            CPU::reserved, // 2d
            CPU::swr, // 2e
            CPU::reserved, // 2f
            CPU::lwc0, // 30
            CPU::lwc1, // 31
            CPU::lwc2, // 32
            CPU::lwc3, // 33
            CPU::reserved, // 34
            CPU::reserved, // 35
            CPU::reserved, // 36
            CPU::reserved, // 37
            CPU::swc0, // 38
            CPU::swc1, // 39
            CPU::swc2, // 3a
            CPU::swc3, // 3b
            CPU::reserved, // 3c
            CPU::reserved, // 3d
            CPU::reserved, // 3e
            CPU::reserved // 3f
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
            CPU::sll, // 0
            CPU::reserved, // 1
            CPU::srl, // 2
            CPU::sra, // 3
            CPU::sllv, // 4
            CPU::reserved, // 5
            CPU::srlv, // 6
            CPU::srav, // 7
            CPU::jr, // 8
            CPU::jalr, // 9
            CPU::reserved, // a
            CPU::reserved, // b
            CPU::syscall, // c
            CPU::break_, // d
            CPU::reserved, // e
            CPU::reserved, // f
            CPU::mfhi, // 10
            CPU::mthi, // 11
            CPU::mflo, // 12
            CPU::mtlo, // 13
            CPU::reserved, // 14
            CPU::reserved, // 15
            CPU::reserved, // 16
            CPU::reserved, // 17
            CPU::mult, // 18
            CPU::multu, // 19
            CPU::div, // 1a
            CPU::divu, // 1b
            CPU::reserved, // 1c
            CPU::reserved, // 1d,
            CPU::reserved, // 1e,
            CPU::reserved, // 1f
            CPU::add, // 20
            CPU::addu, // 21
            CPU::sub, // 22
            CPU::subu, // 23
            CPU::and, // 24
            CPU::or, // 25
            CPU::xor, // 26
            CPU::nor, // 27
            CPU::reserved, // 28
            CPU::reserved, // 29
            CPU::slt, // 2a
            CPU::sltu, // 2b
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
            CPU::reserved
        ];

        Self {
            r: [0; 32],
            pc: 0xbfc00000,
            previous_pc: 0xbfc00000,
            next_pc: 0xbfc00004,
            hi: 0,
            lo: 0,
            bus: Bus::new(),
            instructions,
            special_instructions,
            delayed_load: None,
            cop0: COP0::new(),
            found: HashSet::new(),
            debug_on: false,
            ignored_load_delay: None,
            cycles: 0
        }
    }

    pub fn tick(&mut self, cycles: usize) {
        self.cycles += cycles;
    }

    fn handle_exceptions(&mut self) {
        // TODO
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
            if let Some(ignored_load_delay) = self.ignored_load_delay {
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

    pub fn step(&mut self) {
        self.r[0] = 0;

        self.handle_exceptions();

        let should_transfer = self.delayed_load.is_some();

        let opcode = self.bus.mem_read32(self.pc);

        self.previous_pc = self.pc;

        self.pc = self.next_pc;


        if !self.found.contains(&self.previous_pc) {
            println!("[Opcode: 0x{:x}] [PC: 0x{:x}] {}", opcode, self.previous_pc, self.disassemble(opcode));
            // self.found.insert(self.previous_pc);
        }

        self.next_pc += 4;

        self.decode_opcode(opcode);

        if should_transfer {
            self.transfer_load();
        }
    }
}