use super::{instructions::Instruction, CPU};

impl CPU {
    pub fn disassemble(&self, instruction: u32) -> String {
        let instr = Instruction(instruction);

        let upper = instruction >> 26;

        let command = match upper {
            0x0 => match instruction & 0x3f {
                0x0 => "SLL",
                0x2 => "SRL",
                0x3 => "SRA",
                0x4 => "SLLV",
                0x6 => "SRLV",
                0x7 => "SRAV",
                0x8 => "JR",
                0x9 => "JALR",
                0xc => "SYSCALL",
                0xd => "BREAK",
                0x10 => "MFHI",
                0x11 => "MTHI",
                0x12 => "MFLO",
                0x13 => "MTLO",
                0x18 => "MULT",
                0x19 => "MULTU",
                0x1a => "DIV",
                0x1b => "DIVU",
                0x20 => "ADD",
                0x21 => "ADDU",
                0x22 => "SUB",
                0x23 => "SUBU",
                0x24 => "AND",
                0x25 => "OR",
                0x26 => "XOR",
                0x27 => "NOR",
                0x2a => "SLT",
                0x2b => "SLTU",
                _ => panic!("unknown instruction received: 0x{:x}", instruction & 0x3f)
            }
            0x1 => match instr.rt() {
                0x0 => "BLTZ",
                0x1 => "BGEZ",
                0x10 => "BLTZAL",
                0x11 => "BGEZAL",
                _ => panic!("unknown value for BcondZ given: 0x{:x}", instr.rt())
            },
            0x2 => "J",
            0x3 => "JAL",
            0x4 => "BEQ",
            0x5 => "BNE",
            0x6 => "BLEZ",
            0x7 => "BGTZ",
            0x8 => "ADDI",
            0x9 => "ADDIU",
            0xa => "SLTI",
            0xb => "SLTIU",
            0xc => "ANDI",
            0xd => "ORI",
            0xe => "XORI",
            0xf => "LUI",
            0x10 => "COP0",
            0x11 => "COP1",
            0x12 => "COP2",
            0x13 => "COP3",
            0x20 => "LB",
            0x21 => "LH",
            0x22 => "LWL",
            0x23 => "LW",
            0x24 => "LBU",
            0x25 => "LHU",
            0x26 => "LWR",
            0x28 => "SB",
            0x29 => "SH",
            0x2a => "SWL",
            0x2b => "SW",
            0x2e => "SWR",
            0x30 => "LWC0",
            0x31 => "LWC1",
            0x32 => "LWC2",
            0x33 => "LWC3",
            0x38 => "SWC0",
            0x39 => "SWC1",
            0x3a => "SWC2",
            0x3b => "SWC3",
            _ => panic!("invalid value given to disassembler: 0x{:x}", upper)
        };

        // see https://psx-spx.consoledev.net/cpuspecifications/#cpu-opcode-encoding
        if upper == 0 {
            if instruction & 0b111100 == 0b100 {
                return format!("{command} r{}, r{}, r{}", instr.rd(), instr.rt(), instr.rs());
            }
            if instruction & 0b111111 <= 0b11 {
                return format!("{command} r{}, r{}, 0x{:x}", instr.rd(), instr.rt(), instr.immediate5());
            }
            if instruction & 0b111111 == 0b1000 {
                return format!("{command} 0x{:x} (r{})", self.r[instr.rs()], instr.rs());
            }
            if instruction & 0b111111 == 0b1001 {
                return format!("{command} r{}, r{}", instr.rd(), instr.rs());
            }
            if instruction & 0b111110 == 0b1100 {
                return command.to_string();
            }
            if instruction & 0b111101 == 0b10000 {
                return format!("{command} r{}", instr.rd());
            }
            if instruction & 0b111101 == 0b10001 {
                return format!("{command} r{}", instr.rs());
            }
            if instruction & 0b111100 == 0b11000 {
                return format!("{command} r{}, r{}", instr.rs(), instr.rt());
            }
            if instruction & 0b110000 == 0b100000 {
                return format!("{command} r{}, r{}, r{}", instr.rd(), instr.rs(), instr.rt());
            }
        }

        if upper == 1 {
            return format!("{command} r{}, 0x{:x}", instr.rs(), instr.immediate16());
        }

        if upper & 0b111110 == 0b10 {
            return format!("{command} 0x{:x}", (instr.immediate26() << 2) | (self.pc & 0xf0000000));
        }

        if upper & 0b111110 == 0b100 {
            let destination = ((self.pc as i32) + (instr.signed_immediate16() << 2)) as u32;
            return format!("{command} r{}, r{}, 0x{:x}", instr.rs(), instr.rt(), destination);
        }

        if upper & 0b111110 == 0b110 {
            let destination = ((self.pc as i32) + (instr.signed_immediate16() << 2)) as u32;
            return format!("{command} r{}, 0x{:x}", instr.rs(), destination);
        }

        if upper & 0b111000 == 0b1000 {
            return format!("{command} r{}, r{}, 0x{:x}", instr.rt(), instr.rs(), instr.immediate16());
        }

        if upper & 0b111111 == 0b1111 {
            return format!("{command} r{}, 0x{:x}", instr.rt(), instr.immediate16());
        }

        if upper & 0b110000 == 0b100000 {
            return format!("{command} r{}, [r{} + 0x{:x}]", instr.rt(), instr.rs(), instr.immediate16());
        }

        let upper_cop0 = instruction >> 28;
        let mid = (instruction >> 21) & 0x1f;

        match upper_cop0 {
            0x4 => match mid {
                0x0 => return format!("MFC0 r{}, r{}", instr.rt(), instr.rd()),
                0x4 => return format!("MTC0 r{}, r{}", instr.rt(), instr.rd()),
                0x10 => return "RFE".to_string(),
                _ => todo!("cop0 disassembly: 0b{:032b}, upper = 0b{:b} middle = 0b{:b}", instruction, upper_cop0, mid)
            }
            _ => todo!("cop0 disassembly: 0b{:032b}", instruction)
        }
    }
}