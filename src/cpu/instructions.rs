use super::{ExceptionType, CPU, RA_REGISTER};
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

    pub fn cop2_command(&self) -> u32 {
        self.0 & 0x3ff_ffff
    }

    pub fn cop_code(&self) -> u32 {
        (self.0 >> 21) & 0x1f
    }
    pub fn bcond(&self) -> u32 {
        (self.0 >> 16) & 0b1
    }
}

impl CPU {
    pub fn decode_opcode(&mut self, instruction: u32) -> usize {
        let op = instruction >> 26;

        if op == 0 {
            let special_op = instruction & 0x3f;
            return self.special_instructions[special_op as usize](self, Instruction(instruction));
        }
        self.instructions[op as usize](self, Instruction(instruction))
    }

    pub fn reserved(&mut self, instruction: Instruction) -> usize {
        panic!("invalid instruction received: 0x{:x}", instruction.0);
    }

    pub fn bcondz(&mut self, instruction: Instruction) -> usize {
        let bits = instruction.rt();
        match (bits & 1, (bits & 0x1e) ) {
            (0, 0x10) => self.bltzal(instruction),
            (1, 0x10) => self.bgezal(instruction),
            (0, _) => self.bltz(instruction),
            (1, _) => self.bgez(instruction),
            _ => unreachable!()
        }
    }

    pub fn bltz(&mut self, instruction: Instruction) -> usize {
        if (self.r[instruction.rs()] as i32) < 0 {
            self.branch_taken = true;
            self.next_pc = ((self.pc as i32) + (instruction.signed_immediate16() << 2)) as u32;
        }

        2
    }

    pub fn bgez(&mut self, instruction: Instruction) -> usize {
        if (self.r[instruction.rs()] as i32) >= 0 {
            self.branch_taken = true;
            self.next_pc = ((self.pc as i32) + (instruction.signed_immediate16() << 2)) as u32;
        }

        2
    }

    pub fn bltzal(&mut self, instruction: Instruction) -> usize {
        let val = self.r[instruction.rs()];
        self.r[RA_REGISTER] = self.next_pc;
        self.ignored_load_delay = Some(RA_REGISTER);

        if (val as i32) < 0 {
            self.branch_taken = true;
            self.next_pc = ((self.pc as i32) + (instruction.signed_immediate16() << 2)) as u32;
        }

        2
    }

    pub fn bgezal(&mut self, instruction: Instruction) -> usize {
        let val = self.r[instruction.rs()];
        self.r[RA_REGISTER] = self.next_pc;
        self.ignored_load_delay = Some(RA_REGISTER);

        if (val as i32) >= 0 {
            self.branch_taken = true;
            self.next_pc = ((self.pc as i32) + (instruction.signed_immediate16() << 2)) as u32;
        }

        2
    }

    pub fn j(&mut self, instruction: Instruction) -> usize {
        self.next_pc = (self.pc & 0xf000_0000) | instruction.immediate26() << 2;

        self.branch_taken = true;

        2
    }

    pub fn jal(&mut self, instruction: Instruction) -> usize {
        self.r[RA_REGISTER] = self.next_pc;

        self.ignored_load_delay = Some(RA_REGISTER);

        self.next_pc = (self.pc & 0xf0000000) | instruction.immediate26() << 2;
        self.branch_taken = true;

        2
    }

    pub fn beq(&mut self, instruction: Instruction) -> usize {
       if self.r[instruction.rs()] == self.r[instruction.rt()] {
            self.next_pc = ((self.pc as i32) + (instruction.signed_immediate16() << 2)) as u32;
            self.branch_taken = true;
        }

        2
    }

    pub fn bne(&mut self, instruction: Instruction) -> usize {
        if self.r[instruction.rs()] != self.r[instruction.rt()] {
            self.next_pc = ((self.pc as i32) + (instruction.signed_immediate16() << 2)) as u32;
            self.branch_taken = true;
        }

        2
    }

    pub fn blez(&mut self, instruction: Instruction) -> usize {
        if self.r[instruction.rs()] as i32 <= 0 {
            self.next_pc = ((self.pc as i32) + (instruction.signed_immediate16() << 2)) as u32;
            self.branch_taken = true;
        }

        2
    }

    pub fn bgtz(&mut self, instruction: Instruction) -> usize {
        if (self.r[instruction.rs()] as i32) > 0 {
           self.next_pc = ((self.pc as i32) + (instruction.signed_immediate16() << 2)) as u32;
           self.branch_taken = true;
        }

        2
    }

    pub fn addi(&mut self, instruction: Instruction) -> usize {
        let (result, overflow) = (self.r[instruction.rs()] as i32).overflowing_add(instruction.signed_immediate16());

        if overflow {
            self.enter_exception(ExceptionType::Overflow);
        } else {
            self.r[instruction.rt()] = result as u32;
            self.ignored_load_delay = Some(instruction.rt());
        }
        self.ignored_load_delay = Some(instruction.rt());

        2
    }

    pub fn addiu(&mut self, instruction: Instruction) -> usize {
        self.r[instruction.rt()] = (self.r[instruction.rs()] as i32 + instruction.signed_immediate16()) as u32;

        self.ignored_load_delay = Some(instruction.rt());

        2
    }

    pub fn slti(&mut self, instruction: Instruction) -> usize {
        self.r[instruction.rt()] = ((self.r[instruction.rs()] as i32) < instruction.signed_immediate16()) as u32;
        self.ignored_load_delay = Some(instruction.rt());

        2
    }

    pub fn sltiu(&mut self, instruction: Instruction) -> usize {
        let extended_immediate = instruction.signed_immediate16() as u32;
        self.r[instruction.rt()] = (self.r[instruction.rs()] < extended_immediate) as u32;
        self.ignored_load_delay = Some(instruction.rt());

        2
    }

    pub fn andi(&mut self, instruction: Instruction) -> usize {
        self.r[instruction.rt()] = self.r[instruction.rs()] & instruction.immediate16();
        self.ignored_load_delay = Some(instruction.rt());

        2
    }

    pub fn ori(&mut self, instruction: Instruction) -> usize {
        self.r[instruction.rt()] = self.r[instruction.rs()] | instruction.immediate16();
        self.ignored_load_delay = Some(instruction.rt());

        2
    }

    pub fn xori(&mut self, instruction: Instruction) -> usize {
        self.r[instruction.rt()] = self.r[instruction.rs()] ^ instruction.immediate16();
        self.ignored_load_delay = Some(instruction.rt());

        2
    }

    pub fn lui(&mut self, instruction: Instruction) -> usize {
        self.r[instruction.rt()] = instruction.immediate16() << 16;
        self.ignored_load_delay = Some(instruction.rt());

        2
    }

    pub fn cop0(&mut self, instruction: Instruction) -> usize {
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

        2
    }

    pub fn cop1(&mut self, _instruction: Instruction) -> usize {
        panic!("cop1 nonexistent in playstation");
    }

    pub fn cop2(&mut self, instruction: Instruction) -> usize {
        let cop_code = instruction.cop_code();

        let mut cycles = 2;

        match cop_code {
            0x0 => {
                let value = self.gte.read_data(instruction.rd());

                self.update_load(instruction.rt(), value);
            }
            0x2 => {
                let value = self.gte.read_control(instruction.rd());

                self.update_load(instruction.rt(), value);
            }
            0x4 => self.gte.write_data(instruction.rd(), self.r[instruction.rt()]),
            0x6 => self.gte.write_control(instruction.rd(), self.r[instruction.rt()]),
            _ => if cop_code & 0x10 == 0x10 { cycles = self.gte.execute_command(instruction) }
        }

        cycles
    }

    pub fn cop3(&mut self, _instruction: Instruction) -> usize {
        panic!("cop3 nonexistent in playstation");
    }

    pub fn lb(&mut self, instruction: Instruction) -> usize {
        let address = (self.r[instruction.rs()] as i32 + instruction.signed_immediate16()) as u32;

        let value = self.bus.mem_read8(address) as i8 as i16 as i32 as u32;

        self.update_load(instruction.rt(), value);

        2
    }

    pub fn lh(&mut self, instruction: Instruction) -> usize {
        let address = (self.r[instruction.rs()] as i32 + instruction.signed_immediate16()) as u32;
        if address & 1 == 0 {
            let value = self.bus.mem_read16(address) as i16 as i32 as u32;
            self.update_load(
                instruction.rt(),
                value
            );
        } else {
            self.cop0.bad_addr = address;
            self.enter_exception(ExceptionType::LoadAddressError);
        }
        2
    }

    pub fn lwl(&mut self, instruction: Instruction) -> usize {
        let address = (self.r[instruction.rs()] as i32 + instruction.signed_immediate16()) as u32;
        let mut result = self.r[instruction.rt()];

        if let Some((register, value)) = self.delayed_load {
            if register == instruction.rt() {

                result = value;
            }
        }

        let aligned_address = address & !3;
        let aligned_word = self.bus.mem_read32(aligned_address);
        result = match address & 0x3 {
            0 => (result & 0xffffff) | (aligned_word << 24),
            1 => (result & 0xffff) | (aligned_word << 16),
            2 => (result & 0xff) | (aligned_word << 8),
            3 => aligned_word,
            _ => unreachable!("can't happen")
        };

        self.update_load(instruction.rt(), result);

        2
    }

    pub fn lw(&mut self, instruction: Instruction) -> usize {
        let address = (self.r[instruction.rs()] as i32 + instruction.signed_immediate16()) as u32;

        if address & 0x3 == 0 {
            let value = self.bus.mem_read32(address);
            self.update_load(instruction.rt(), value);
        } else {
            self.cop0.bad_addr = address;
            self.enter_exception(ExceptionType::LoadAddressError);
        }

        2
    }

    pub fn lbu(&mut self, instruction: Instruction) -> usize {
        let address = (self.r[instruction.rs()] as i32 + instruction.signed_immediate16()) as u32;

        let value = self.bus.mem_read8(address);
        self.update_load(instruction.rt(), value);

        2
    }

    pub fn lhu(&mut self, instruction: Instruction) -> usize {
        let address = (self.r[instruction.rs()] as i32 + instruction.signed_immediate16()) as u32;

        if address & 1 == 0 {
            let value = self.bus.mem_read16(address);
            self.update_load(instruction.rt(), value);
        } else {
            self.cop0.bad_addr = address;
            self.enter_exception(ExceptionType::LoadAddressError);
        }

        2
    }

    pub fn lwr(&mut self, instruction: Instruction) -> usize {
        let address = (self.r[instruction.rs()] as i32 + instruction.signed_immediate16()) as u32;
        let mut result = self.r[instruction.rt()];

        if let Some((register, value)) = self.delayed_load {
            if register == instruction.rt() {
                result = value;
            }
        }

        let aligned_address = address & !3;

        let aligned_word = self.bus.mem_read32(aligned_address);
        result = match address & 0x3 {
            0 => aligned_word,
            1 => (result & 0xff000000) | (aligned_word >> 8),
            2 => (result & 0xffff0000) | (aligned_word >> 16),
            3 => (result & 0xffffff00) | (aligned_word >> 24),
            _ => unreachable!("can't happen")
        };

        self.update_load(instruction.rt(), result);

        2
    }

    pub fn sb(&mut self, instruction: Instruction) -> usize {
        let address = (self.r[instruction.rs()] as i32 + instruction.signed_immediate16()) as u32;

        let value = self.r[instruction.rt()] as u8;

        self.store8(address, value);

        2
    }

    pub fn sh(&mut self, instruction: Instruction) -> usize {
        let address = (self.r[instruction.rs()] as i32 + instruction.signed_immediate16()) as u32;

        if address & 1 == 0 {
            self.store16(address, self.r[instruction.rt()] as u16);
        } else {
            self.cop0.bad_addr = address;
            self.enter_exception(ExceptionType::StoreAddressError);
        }

        2
    }

    pub fn swl(&mut self, instruction: Instruction) -> usize {
        let address = (self.r[instruction.rs()] as i32 + instruction.signed_immediate16()) as u32;

        let value = self.r[instruction.rt()];
        let mem_value = self.bus.mem_read32(address & !3);

        let result = match address & 0x3 {
            0 => (mem_value & 0xffffff00) | (value >> 24),
            1 => (mem_value & 0xffff0000) | (value >> 16),
            2 => (mem_value & 0xff000000) | (value >> 8),
            3 => value,
            _ => unreachable!("can't happen")
        };

        self.store32(address & !3, result);

        2
    }

    pub fn sw(&mut self, instruction: Instruction) -> usize {
        let address = (self.r[instruction.rs()] as i32 + instruction.signed_immediate16()) as u32;

        if address & 0x3 == 0 {
            let value = self.r[instruction.rt()];

            self.store32(address, value);
        } else {
            self.cop0.bad_addr = address;
            self.enter_exception(ExceptionType::StoreAddressError);
        }

        2
    }

    pub fn swr(&mut self, instruction: Instruction) -> usize {
        let address = (self.r[instruction.rs()] as i32 + instruction.signed_immediate16()) as u32;

        let value = self.r[instruction.rt()];
        let mem_value = self.bus.mem_read32(address & !3);

        let result = match address & 0x3 {
            0 => value,
            1 => (mem_value & 0xff) | (value << 8),
            2 => (mem_value & 0xffff) | (value << 16),
            3 => (mem_value & 0xffffff) | (value << 24),
            _ => unreachable!("can't happen")
        };

        self.store32(address & !3, result);

        2
    }

    pub fn lwc0(&mut self, _instruction: Instruction) -> usize {
        todo!("lwc0");
    }

    pub fn lwc1(&mut self, _instruction: Instruction) -> usize {
        todo!("lwc1");
    }

    pub fn lwc2(&mut self, instruction: Instruction) -> usize {
        let address = (self.r[instruction.rs()] as i32 + instruction.signed_immediate16()) as u32;

        let value = self.bus.mem_read32(address);

        self.gte.write_data(instruction.rt(), value);

        2
    }

    pub fn lwc3(&mut self, _instruction: Instruction) -> usize {
        todo!("lwc3");
    }

    pub fn swc0(&mut self, _instruction: Instruction) -> usize {
        todo!("swc0");
    }

    pub fn swc1(&mut self, _instruction: Instruction) -> usize {
        todo!("swc1");
    }

    pub fn swc2(&mut self, instruction: Instruction) -> usize {
        let address = (self.r[instruction.rs()] as i32 + instruction.signed_immediate16()) as u32;

        let value = self.gte.read_data(instruction.rt());

        self.store32(address, value);

        2
    }

    pub fn swc3(&mut self, _instruction: Instruction) -> usize {
        todo!("swc3");
    }

    pub fn sll(&mut self, instruction: Instruction) -> usize {
        self.r[instruction.rd()] = self.r[instruction.rt()] << instruction.immediate5();
        self.ignored_load_delay = Some(instruction.rd());

        2
    }

    pub fn srl(&mut self, instruction: Instruction) -> usize {
        self.r[instruction.rd()] = self.r[instruction.rt()] >> instruction.immediate5();
        self.ignored_load_delay = Some(instruction.rd());

        2
    }

    pub fn sra(&mut self, instruction: Instruction) -> usize {
        let shifted_val = self.r[instruction.rt()] as i32;
        let value = shifted_val >> instruction.immediate5();

        self.r[instruction.rd()] = value as u32;

        self.ignored_load_delay = Some(instruction.rd());

        2
    }

    pub fn sllv(&mut self, instruction: Instruction) -> usize {
        let shift = self.r[instruction.rs()] & 0x1f;
        self.r[instruction.rd()] = self.r[instruction.rt()] << shift;
        self.ignored_load_delay = Some(instruction.rd());

        2
    }

    pub fn srlv(&mut self, instruction: Instruction) -> usize{
        let shift = self.r[instruction.rs()] & 0x1f;
        self.r[instruction.rd()] = self.r[instruction.rt()] >> shift;
        self.ignored_load_delay = Some(instruction.rd());

        2
    }

    pub fn srav(&mut self, instruction: Instruction) -> usize {
        let shifted_val = self.r[instruction.rt()] as i32;

        let shift = self.r[instruction.rs()] & 0x1f;
        let value = shifted_val >> shift;

        self.r[instruction.rd()] = value as u32;

        self.ignored_load_delay = Some(instruction.rd());

        2
    }

    pub fn jr(&mut self, instruction: Instruction) -> usize {
        self.next_pc = self.r[instruction.rs()];

        self.branch_taken = true;

        2
    }

    pub fn jalr(&mut self, instruction: Instruction) -> usize {
        let return_val = self.next_pc;

        self.next_pc = self.r[instruction.rs()];

        self.r[instruction.rd()] = return_val;
        self.ignored_load_delay = Some(instruction.rd());

        self.branch_taken = true;

        2
    }

    pub fn syscall(&mut self, _instruction: Instruction) -> usize {
        self.enter_exception(ExceptionType::Syscall);

        2
    }

    pub fn break_(&mut self, _instruction: Instruction) -> usize {
        self.enter_exception(ExceptionType::Break);

        2
    }

    pub fn mfhi(&mut self, instruction: Instruction) -> usize {
        self.ignored_load_delay = Some(instruction.rd());

        self.r[instruction.rd()] = self.hi;
        self.ignored_load_delay = Some(instruction.rd());

        2
    }

    pub fn mthi(&mut self, instruction: Instruction) -> usize{
        self.hi = self.r[instruction.rs()];

        2
    }

    pub fn mflo(&mut self, instruction: Instruction) -> usize {
        self.ignored_load_delay = Some(instruction.rd());

        self.r[instruction.rd()] = self.lo;
        self.ignored_load_delay = Some(instruction.rd());

        2
    }

    pub fn mtlo(&mut self, instruction: Instruction) -> usize {
        self.lo = self.r[instruction.rs()];

        2
    }

    pub fn mult(&mut self, instruction: Instruction) -> usize {
        self.tick(1);

        let result = self.r[instruction.rs()] as i32 as i64 * self.r[instruction.rt()] as i32 as i64;

        self.lo = result as u32;
        self.hi = (result >> 32) as u32;

        2
    }

    pub fn multu(&mut self, instruction: Instruction) -> usize {
        self.tick(1);

        let result = self.r[instruction.rs()] as u64 * self.r[instruction.rt()] as u64;

        self.lo = result as u32;
        self.hi = (result >> 32) as u32;

        2
    }

    pub fn div(&mut self, instruction: Instruction) -> usize {
        self.tick(1);

        let dividend = self.r[instruction.rs()] as i32;
        let divisor = self.r[instruction.rt()] as i32;

        if (dividend as u32) == 0x8000_0000 && divisor == -1 {
            self.lo = dividend as u32;
            self.hi = 0;
        } else if divisor != 0 {
            self.lo = (dividend / divisor) as u32;
            self.hi = (dividend % divisor) as u32;
        } else {
            self.lo = if dividend >= 0 { -1  as i32 as u32 } else { 1 };
            self.hi = dividend as u32;
        }

        2
    }

    pub fn divu(&mut self, instruction: Instruction) -> usize {
        self.tick(1);

        let dividend = self.r[instruction.rs()];
        let divisor = self.r[instruction.rt()];

        if divisor != 0 {
            self.lo = (dividend / divisor) as u32;
            self.hi = (dividend % divisor) as u32;
        } else {
            self.lo = 0xffff_ffff;
            self.hi = dividend;
        }

        2
    }

    pub fn add(&mut self, instruction: Instruction) -> usize{
        if let Some(result) = (self.r[instruction.rs()] as i32).checked_add(self.r[instruction.rt()] as i32) {
            self.r[instruction.rd()] = result as u32;
            self.ignored_load_delay = Some(instruction.rd());

        } else {
            self.enter_exception(ExceptionType::Overflow);
        }

        2
    }

    pub fn addu(&mut self, instruction: Instruction) -> usize {
        self.r[instruction.rd()] = self.r[instruction.rs()] + self.r[instruction.rt()];
        self.ignored_load_delay = Some(instruction.rd());

        2
    }

    pub fn sub(&mut self, instruction: Instruction) -> usize {
        if let Some(result) = (self.r[instruction.rs()] as i32).checked_sub(self.r[instruction.rt()] as i32) {

            self.r[instruction.rd()] = result as u32;

            self.ignored_load_delay = Some(instruction.rd());
        } else {
            self.enter_exception(ExceptionType::Overflow);
        }

        2
    }

    pub fn subu(&mut self, instruction: Instruction) -> usize {
        self.r[instruction.rd()] = self.r[instruction.rs()] - self.r[instruction.rt()];
        self.ignored_load_delay = Some(instruction.rd());

        2
    }

    pub fn and(&mut self, instruction: Instruction) -> usize {
        self.r[instruction.rd()] = self.r[instruction.rs()] & self.r[instruction.rt()];
        self.ignored_load_delay = Some(instruction.rd());

        2
    }

    pub fn or(&mut self, instruction: Instruction) -> usize {
        self.r[instruction.rd()] = self.r[instruction.rs()] | self.r[instruction.rt()];
        self.ignored_load_delay = Some(instruction.rd());

        2
    }

    pub fn xor(&mut self, instruction: Instruction) -> usize {
        self.r[instruction.rd()] = self.r[instruction.rs()] ^ self.r[instruction.rt()];
        self.ignored_load_delay = Some(instruction.rd());

        2
    }

    pub fn nor(&mut self, instruction: Instruction) -> usize {
        self.r[instruction.rd()] = !(self.r[instruction.rs()] | self.r[instruction.rt()]);
        self.ignored_load_delay = Some(instruction.rd());

        2
    }

    pub fn slt(&mut self, instruction: Instruction) -> usize {
        self.r[instruction.rd()] = ((self.r[instruction.rs()] as i32) < (self.r[instruction.rt()] as i32)) as u32;
        self.ignored_load_delay = Some(instruction.rd());

        2
    }

    pub fn sltu(&mut self, instruction: Instruction) -> usize {
        self.r[instruction.rd()] = (self.r[instruction.rs()] < self.r[instruction.rt()]) as u32;
        self.ignored_load_delay = Some(instruction.rd());

        2
    }

    fn update_load(&mut self, index: usize, value: u32) {
        if let Some((pending_index, pending_value)) = self.delayed_load {
            if index != pending_index {
                self.r[pending_index] = pending_value;
            }
        }
        self.should_transfer_load = false;
        self.delayed_load = Some((index, value));
    }
}