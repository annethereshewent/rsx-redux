use super::{cop0::CauseRegister, ExceptionType, CPU, RA_REGISTER};


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
        match instruction.rt() {
            0x0 => self.bltz(instruction),
            0x1 => self.bgez(instruction),
            0x10 => self.bltzal(instruction),
            0x11 => self.bgezal(instruction),
            _ => panic!("invalid option given for BcondZ: 0x{:x}", instruction.rt())
        }
    }

    pub fn bltz(&mut self, instruction: Instruction) {
        if (self.r[instruction.rs()] as i32) < 0 {
            self.next_pc = ((self.pc as i32) + (instruction.signed_immediate16() << 2)) as u32;
            self.branch_taken = true;
            self.cop0.cause.insert(CauseRegister::BD);
        }
    }

    pub fn bgez(&mut self, instruction: Instruction) {
        if (self.r[instruction.rs()] as i32) >= 0 {
            self.next_pc = ((self.pc as i32) + (instruction.signed_immediate16() << 2)) as u32;
            self.branch_taken = true;
            self.cop0.cause.insert(CauseRegister::BD);
        }
    }

    pub fn bltzal(&mut self, instruction: Instruction) {
        todo!("bltzal");
    }

    pub fn bgezal(&mut self, instruction: Instruction) {
        todo!("bgezal");
    }

    pub fn j(&mut self, instruction: Instruction) {
        self.next_pc = (self.pc & 0xf0000000) | instruction.immediate26() << 2;
        self.branch_taken = true;
        self.cop0.cause.insert(CauseRegister::BD);
    }

    pub fn jal(&mut self, instruction: Instruction) {
        self.r[RA_REGISTER] = self.next_pc;

        self.ignored_load_delay = Some(RA_REGISTER);

        self.next_pc = (self.pc & 0xf0000000) | instruction.immediate26() << 2;
        self.branch_taken = true;
        self.cop0.cause.insert(CauseRegister::BD);
    }

    pub fn beq(&mut self, instruction: Instruction) {
       if self.r[instruction.rs()] == self.r[instruction.rt()] {
            self.next_pc = ((self.pc as i32) + (instruction.signed_immediate16() << 2)) as u32;
            self.branch_taken = true;
            self.cop0.cause.insert(CauseRegister::BD);
        }
    }

    pub fn bne(&mut self, instruction: Instruction) {
        if self.r[instruction.rs()] != self.r[instruction.rt()] {
            self.next_pc = ((self.pc as i32) + (instruction.signed_immediate16() << 2)) as u32;
            self.branch_taken = true;
            self.cop0.cause.insert(CauseRegister::BD);
        }
    }

    pub fn blez(&mut self, instruction: Instruction) {
        if self.r[instruction.rs()] as i32 <= 0 {
            self.next_pc = ((self.pc as i32) + (instruction.signed_immediate16() << 2)) as u32;
            self.branch_taken = true;
            self.cop0.cause.insert(CauseRegister::BD);
        }
    }

    pub fn bgtz(&mut self, instruction: Instruction) {
        if self.r[instruction.rs()] > 0 {
           self.next_pc = ((self.pc as i32) + (instruction.signed_immediate16() << 2)) as u32;
           self.branch_taken = true;
           self.cop0.cause.insert(CauseRegister::BD);
        }
    }

    pub fn addi(&mut self, instruction: Instruction) {
        let (result, overflow) = (self.r[instruction.rs()] as i32).overflowing_add(instruction.signed_immediate16());

        if overflow {
            todo!("raise checked add exception");
        } else {
            self.r[instruction.rt()] = result as u32;
            self.ignored_load_delay = Some(instruction.rt());
        }
        self.ignored_load_delay = Some(instruction.rt());
    }

    pub fn addiu(&mut self, instruction: Instruction) {
        self.r[instruction.rt()] = (self.r[instruction.rs()] as i32 + instruction.signed_immediate16()) as u32;
        self.ignored_load_delay = Some(instruction.rt());
    }

    pub fn slti(&mut self, instruction: Instruction) {
        self.r[instruction.rt()] = ((self.r[instruction.rs()] as i32) < instruction.signed_immediate16()) as u32;
        self.ignored_load_delay = Some(instruction.rt());
    }

    pub fn sltiu(&mut self, instruction: Instruction) {
        let extended_immediate = instruction.signed_immediate16() as u32;
        self.r[instruction.rt()] = (self.r[instruction.rs()] < extended_immediate) as u32;
        self.ignored_load_delay = Some(instruction.rt());
    }

    pub fn andi(&mut self, instruction: Instruction) {
        self.r[instruction.rt()] = self.r[instruction.rs()] & instruction.immediate16();
        self.ignored_load_delay = Some(instruction.rt());
    }

    pub fn ori(&mut self, instruction: Instruction) {
        self.r[instruction.rt()] = self.r[instruction.rs()] | instruction.immediate16();
        self.ignored_load_delay = Some(instruction.rt());
    }

    pub fn xori(&mut self, instruction: Instruction) {
        todo!("xori");
    }

    pub fn lui(&mut self, instruction: Instruction) {
        self.r[instruction.rt()] = instruction.immediate16() << 16;
        self.ignored_load_delay = Some(instruction.rt());
    }

    pub fn cop0(&mut self, instruction: Instruction) {
        let upper = instruction.0 >> 28;
        let mid = (instruction.0 >> 21) & 0x1f;

        match upper {
            0x4 => match mid {
                0 => {
                    let value = self.cop0.mfc0(instruction.rd());

                    self.update_load(instruction.rt(), value);
                }
                4 => self.cop0.mtc0(instruction.rd(), self.r[instruction.rt()]),
                0x10 => self.cop0.rfe(),
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
        let address = (self.r[instruction.rs()] as i32 + instruction.signed_immediate16()) as u32;

        self.update_load(
            instruction.rt(),
            self.bus.mem_read8(address) as i8 as i16 as i32 as u32
        );
    }

    pub fn lh(&mut self, instruction: Instruction) {
        let address = (self.r[instruction.rs()] as i32 + instruction.signed_immediate16()) as u32;

        self.update_load(
            instruction.rt(),
            self.bus.mem_read16(address) as i16 as i32 as u32
        );
    }

    pub fn lwl(&mut self, instruction: Instruction) {
        todo!("lwl");
    }

    pub fn lw(&mut self, instruction: Instruction) {
        let address = (self.r[instruction.rs()] as i32 + instruction.signed_immediate16()) as u32;

        self.update_load(instruction.rt(), self.bus.mem_read32(address));
    }

    pub fn lbu(&mut self, instruction: Instruction) {
        let address = (self.r[instruction.rs()] as i32 + instruction.signed_immediate16()) as u32;

        self.update_load(instruction.rt(), self.bus.mem_read8(address));
    }

    pub fn lhu(&mut self, instruction: Instruction) {
        let address = (self.r[instruction.rs()] as i32 + instruction.signed_immediate16()) as u32;

        self.update_load(instruction.rt(), self.bus.mem_read16(address));
    }

    pub fn lwr(&mut self, instruction: Instruction) {
        todo!("lwr");
    }

    pub fn sb(&mut self, instruction: Instruction) {
        let address = (self.r[instruction.rs()] as i32 + instruction.signed_immediate16()) as u32;

        self.store8(address, self.r[instruction.rt()] as u8);
    }

    pub fn sh(&mut self, instruction: Instruction) {
        let address = (self.r[instruction.rs()] as i32 + instruction.signed_immediate16()) as u32;

        self.store16(address, self.r[instruction.rt()] as u16);
    }

    pub fn swl(&mut self, instruction: Instruction) {
        todo!("swl");
    }

    pub fn sw(&mut self, instruction: Instruction) {
        let address = (self.r[instruction.rs()] as i32 + instruction.signed_immediate16()) as u32;

        self.store32(address, self.r[instruction.rt()]);
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
        self.ignored_load_delay = Some(instruction.rd());
    }

    pub fn srl(&mut self, instruction: Instruction) {
        self.r[instruction.rd()] = self.r[instruction.rt()] >> instruction.immediate5();
        self.ignored_load_delay = Some(instruction.rd());
    }

    pub fn sra(&mut self, instruction: Instruction) {
        let shifted_val = self.r[instruction.rt()] as i32;
        self.r[instruction.rd()] = (shifted_val >> instruction.immediate5()) as u32;

        self.ignored_load_delay = Some(instruction.rd());
    }

    pub fn sllv(&mut self, instruction: Instruction) {
        let shift = self.r[instruction.rs()] & 0x1f;
        self.r[instruction.rd()] = self.r[instruction.rt()] << shift;
    }

    pub fn srlv(&mut self, instruction: Instruction) {
        todo!("srlv");
    }

    pub fn srav(&mut self, instruction: Instruction) {
        todo!("srav");
    }

    pub fn jr(&mut self, instruction: Instruction) {
        self.next_pc = self.r[instruction.rs()];

        self.branch_taken = true;
        self.cop0.cause.insert(CauseRegister::BD);
    }

    pub fn jalr(&mut self, instruction: Instruction) {
        let return_val = self.next_pc;

        self.next_pc = self.r[instruction.rs()];

        self.r[instruction.rd()] = return_val;
        self.ignored_load_delay = Some(instruction.rd());

        self.branch_taken = true;
        self.cop0.cause.insert(CauseRegister::BD);
    }

    pub fn syscall(&mut self, _instruction: Instruction) {
        self.enter_exception(ExceptionType::Syscall);
    }

    pub fn break_(&mut self, instruction: Instruction) {
        todo!("break");
    }

    pub fn mfhi(&mut self, instruction: Instruction) {
        self.ignored_load_delay = Some(instruction.rd());

        self.r[instruction.rd()] = self.hi;
    }

    pub fn mthi(&mut self, instruction: Instruction) {
        self.hi = self.r[instruction.rs()];
    }

    pub fn mflo(&mut self, instruction: Instruction) {
        self.ignored_load_delay = Some(instruction.rd());

        self.r[instruction.rd()] = self.lo;
    }

    pub fn mtlo(&mut self, instruction: Instruction) {
        self.lo = self.r[instruction.rs()];
    }

    pub fn mult(&mut self, instruction: Instruction) {
        todo!("mult");
    }

    pub fn multu(&mut self, instruction: Instruction) {
        todo!("multu");
    }

    pub fn div(&mut self, instruction: Instruction) {
        self.tick(1);

        let divisor = self.r[instruction.rs()] as i32;
        let dividend = self.r[instruction.rt()] as i32;

        if divisor != 0 {
            self.lo = (dividend / divisor) as u32;
            self.hi = (dividend % divisor) as u32;
        }
    }

    pub fn divu(&mut self, instruction: Instruction) {
        self.tick(1);

        let dividend = self.r[instruction.rs()];
        let divisor = self.r[instruction.rt()];

        if divisor != 0 {
            self.lo = dividend / divisor;
            self.hi = dividend % divisor;
        }
    }

    pub fn add(&mut self, instruction: Instruction) {
        let (result, overflow) = self.r[instruction.rs()].overflowing_add(self.r[instruction.rt()]);

        if overflow {
            todo!("raise checked add exception");
        } else {
            self.r[instruction.rd()] = result as u32;
            self.ignored_load_delay = Some(instruction.rd());
        }
    }

    pub fn addu(&mut self, instruction: Instruction) {
        self.r[instruction.rd()] = self.r[instruction.rs()] + self.r[instruction.rt()];
        self.ignored_load_delay = Some(instruction.rd());
    }

    pub fn sub(&mut self, instruction: Instruction) {
        todo!("sub");
    }

    pub fn subu(&mut self, instruction: Instruction) {
        self.r[instruction.rd()] = self.r[instruction.rs()] - self.r[instruction.rt()];
        self.ignored_load_delay = Some(instruction.rd());
    }

    pub fn and(&mut self, instruction: Instruction) {
        self.r[instruction.rd()] = self.r[instruction.rs()] & self.r[instruction.rt()];
        self.ignored_load_delay = Some(instruction.rd());
    }

    pub fn or(&mut self, instruction: Instruction) {
        self.r[instruction.rd()] = self.r[instruction.rs()] | self.r[instruction.rt()];
        self.ignored_load_delay = Some(instruction.rd());
    }

    pub fn xor(&mut self, instruction: Instruction) {
        todo!("xor");
    }

    pub fn nor(&mut self, instruction: Instruction) {
        self.r[instruction.rd()] = !(self.r[instruction.rs()] | self.r[instruction.rt()]);
    }

    pub fn slt(&mut self, instruction: Instruction) {
        self.r[instruction.rd()] = ((self.r[instruction.rs()] as i32) < (self.r[instruction.rt()] as i32)) as u32;
        self.ignored_load_delay = Some(instruction.rd());
    }

    pub fn sltu(&mut self, instruction: Instruction) {
        self.r[instruction.rd()] = (self.r[instruction.rs()] < self.r[instruction.rt()]) as u32;
        self.ignored_load_delay = Some(instruction.rd());
    }

    fn update_load(&mut self, index: usize, value: u32) {
        if let Some((pending_index, pending_value)) = self.delayed_load {
            if index != pending_index {
                self.r[pending_index] = pending_value;
            }
        }

        self.delayed_load = Some((index, value));
    }
}