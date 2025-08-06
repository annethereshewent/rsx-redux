use super::CPU;


pub struct Instruction(pub u32);

impl Instruction {
    pub fn immediate16(&self) -> u32 {
        self.0 & 0xffff
    }

    pub fn signed_immediate16(&self) -> i32 {
        (self.0 & 0xffff) as i16 as i32
    }

    pub fn immediate5(&self) -> u32 {
        (self.0 >> 6) & 0x1f
    }

    pub fn rt(&self) -> usize {
        ((self.0 >> 16) & 0x1f) as usize
    }

    pub fn rs(&self) -> usize {
        ((self.0 >> 21) & 0x1f) as usize
    }

    pub fn rd(&self) -> usize {
        ((self.0 >> 11) & 0x1f) as usize
    }

    pub fn immediate26(&self) -> u32 {
        self.0 & 0x3ffffff
    }
}

impl CPU {
    pub fn decode_opcode(&mut self, instruction: u32) {
        let op = instruction >> 26;

        if op == 0 {
            let special_op = instruction & 0x3f;
            self.special_instructions[special_op as usize](self, Instruction(instruction));
        } else {
            self.instructions[op as usize](self, Instruction(instruction));
        }
    }

    pub fn reserved(&mut self, instruction: Instruction) {
        panic!("invalid instruction received: 0x{:x}", instruction.0);
    }

    pub fn bcondz(&mut self, instruction: Instruction) {
        todo!("bcondz");
    }

    pub fn j(&mut self, instruction: Instruction) {
        self.next_pc = (self.pc & 0xf0000000) | instruction.immediate26() << 2;
    }

    pub fn jal(&mut self, instruction: Instruction) {
        todo!("jal");
    }

    pub fn beq(&mut self, instruction: Instruction) {
        todo!("beq");
    }

    pub fn bne(&mut self, instruction: Instruction) {
        if self.r[instruction.rs()] != self.r[instruction.rt()] {
            self.next_pc = ((self.pc as i32) + (instruction.signed_immediate16() << 2)) as u32;
        }
    }

    pub fn blez(&mut self, instruction: Instruction) {
        todo!("blez");
    }

    pub fn bgtz(&mut self, instruction: Instruction) {
        todo!("bgtz");
    }

    pub fn addi(&mut self, instruction: Instruction) {
        let (result, overflow) = (self.r[instruction.rs()] as i32).overflowing_add(instruction.signed_immediate16());

        if overflow {
            todo!("raise checked add exception");
        } else {
            self.r[instruction.rt()] = result as u32;
        }
    }

    pub fn addiu(&mut self, instruction: Instruction) {
        self.r[instruction.rt()] = (self.r[instruction.rs()] as i32 + instruction.signed_immediate16()) as u32;
    }

    pub fn slti(&mut self, instruction: Instruction) {
        todo!("slti");
    }

    pub fn sltiu(&mut self, instruction: Instruction) {
        todo!("sltiu");
    }

    pub fn andi(&mut self, instruction: Instruction) {
        todo!("andi");
    }

    pub fn ori(&mut self, instruction: Instruction) {
        self.r[instruction.rt()] = self.r[instruction.rs()] | instruction.immediate16();
    }

    pub fn xori(&mut self, instruction: Instruction) {
        todo!("xori");
    }

    pub fn lui(&mut self, instruction: Instruction) {
        self.r[instruction.rt()] = instruction.immediate16() << 16;
    }

    pub fn cop0(&mut self, instruction: Instruction) {
        let upper = instruction.0 >> 28;
        let mid = (instruction.0 >> 21) & 0x1f;

        match upper {
            0x4 => match mid {
                4 => self.cop0.mtc0(instruction.rd(), self.r[instruction.rt()]),
                _ => todo!("cop0 instruction: 0x{:x}", instruction.0)
            }
            0xc => todo!("lwc"),
            0xd => todo!("swc"),
            _ => todo!("cop0 instruction: 0x{:x}", instruction.0)
        }
    }

    pub fn cop1(&mut self, instruction: Instruction) {
        todo!("cop1");
    }

    pub fn cop2(&mut self, instruction: Instruction) {
        todo!("cop2");
    }

    pub fn cop3(&mut self, instruction: Instruction) {
        todo!("cop3");
    }

    pub fn lb(&mut self, instruction: Instruction) {
        todo!("lb");
    }

    pub fn lh(&mut self, instruction: Instruction) {
        todo!("lh");
    }

    pub fn lwl(&mut self, instruction: Instruction) {
        todo!("lwl");
    }

    pub fn lw(&mut self, instruction: Instruction) {
        let address = self.r[instruction.rs()] + instruction.immediate16();

        if self.delayed_register[0].is_none() {
            self.delayed_register[0] = Some(instruction.rt());
            self.delayed_value[0] = Some(self.bus.mem_read32(address));
        } else {
            self.delayed_register[1] = Some(instruction.rt());
            self.delayed_value[1] = Some(self.bus.mem_read32(address));
        }
    }

    pub fn lbu(&mut self, instruction: Instruction) {
        todo!("lbu");
    }

    pub fn lhu(&mut self, instruction: Instruction) {
        todo!("lhu");
    }

    pub fn lwr(&mut self, instruction: Instruction) {
        todo!("lwr");
    }

    pub fn sb(&mut self, instruction: Instruction) {
        todo!("sb");
    }

    pub fn sh(&mut self, instruction: Instruction) {
        todo!("sh");
    }

    pub fn swl(&mut self, instruction: Instruction) {
        todo!("swl");
    }

    pub fn sw(&mut self, instruction: Instruction) {
        let address = self.r[instruction.rs()] + instruction.immediate16();

        self.bus.mem_write32(address, self.r[instruction.rt()]);
    }

    pub fn swr(&mut self, instruction: Instruction) {
        todo!("swr");
    }

    pub fn lwc0(&mut self, instruction: Instruction) {
        todo!("lwc0");
    }

    pub fn lwc1(&mut self, instruction: Instruction) {
        todo!("lwc1");
    }

    pub fn lwc2(&mut self, instruction: Instruction) {
        todo!("lwc2");
    }

    pub fn lwc3(&mut self, instruction: Instruction) {
        todo!("lwc3");
    }

    pub fn swc0(&mut self, instruction: Instruction) {
        todo!("swc0");
    }

    pub fn swc1(&mut self, instruction: Instruction) {
        todo!("swc1");
    }

    pub fn swc2(&mut self, instruction: Instruction) {
        todo!("swc2");
    }

    pub fn swc3(&mut self, instruction: Instruction) {
        todo!("swc3");
    }

    pub fn sll(&mut self, instruction: Instruction) {
        self.r[instruction.rd()] = self.r[instruction.rt()] << instruction.immediate5();
    }

    pub fn srl(&mut self, instruction: Instruction) {
        todo!("srl");
    }

    pub fn sra(&mut self, instruction: Instruction) {
        todo!("sra");
    }

    pub fn sllv(&mut self, instruction: Instruction) {
        todo!("sllv");
    }

    pub fn srlv(&mut self, instruction: Instruction) {
        todo!("srlv");
    }

    pub fn srav(&mut self, instruction: Instruction) {
        todo!("srav");
    }

    pub fn jr(&mut self, instruction: Instruction) {
        todo!("jr");
    }

    pub fn jalr(&mut self, instruction: Instruction) {
        todo!("jalr");
    }

    pub fn syscall(&mut self, instruction: Instruction) {
        todo!("syscall");
    }

    pub fn break_(&mut self, instruction: Instruction) {
        todo!("break");
    }

    pub fn mfhi(&mut self, instruction: Instruction) {
        todo!("mfhi");
    }

    pub fn mthi(&mut self, instruction: Instruction) {
        todo!("mthi");
    }

    pub fn mflo(&mut self, instruction: Instruction) {
        todo!("mflo");
    }

    pub fn mtlo(&mut self, instruction: Instruction) {
        todo!("mtl0");
    }

    pub fn mult(&mut self, instruction: Instruction) {
        todo!("mult");
    }

    pub fn multu(&mut self, instruction: Instruction) {
        todo!("multu");
    }

    pub fn div(&mut self, instruction: Instruction) {
        todo!("div");
    }

    pub fn divu(&mut self, instruction: Instruction) {
        todo!("divu");
    }

    pub fn add(&mut self, instruction: Instruction) {
        todo!("add");
    }

    pub fn addu(&mut self, instruction: Instruction) {
        todo!("addu");
    }

    pub fn sub(&mut self, instruction: Instruction) {
        todo!("sub");
    }

    pub fn subu(&mut self, instruction: Instruction) {
        todo!("subu");
    }

    pub fn and(&mut self, instruction: Instruction) {
        todo!("and");
    }

    pub fn or(&mut self, instruction: Instruction) {
        self.r[instruction.rd()] = self.r[instruction.rs()] | self.r[instruction.rt()];
    }

    pub fn xor(&mut self, instruction: Instruction) {
        todo!("xor");
    }

    pub fn nor(&mut self, instruction: Instruction) {
        todo!("nor");
    }

    pub fn slt(&mut self, instruction: Instruction) {
        todo!("slt");
    }

    pub fn sltu(&mut self, instruction: Instruction) {
        todo!("sltu");
    }
}