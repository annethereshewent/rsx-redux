use super::CPU;

impl CPU {
    pub fn decode_opcode(&mut self, instruction: u32) {
        let op = instruction >> 26;

        if op == 0 {
            let special_op = instruction & 0x3f;
            self.special_instructions[special_op as usize](self, instruction);
        } else {
            self.instructions[op as usize](self, instruction);
        }
    }

    pub fn reserved(&mut self, instruction: u32) {
        panic!("invalid instruction received: 0x{:x}", instruction);
    }

    pub fn bcondz(&mut self, instruction: u32) {
        todo!("bcondz");
    }

    pub fn j(&mut self, instruction: u32) {
        todo!("j");
    }

    pub fn jal(&mut self, instruction: u32) {
        todo!("jal");
    }

    pub fn beq(&mut self, instruction: u32) {
        todo!("beq");
    }

    pub fn bne(&mut self, instruction: u32) {
        todo!("bne");
    }

    pub fn blez(&mut self, instruction: u32) {
        todo!("blez");
    }

    pub fn bgtz(&mut self, instruction: u32) {
        todo!("bgtz");
    }

    pub fn addi(&mut self, instruction: u32) {
        todo!("addi");
    }

    pub fn addiu(&mut self, instruction: u32) {
        todo!("addiu");
    }

    pub fn slti(&mut self, instruction: u32) {
        todo!("slti");
    }

    pub fn sltiu(&mut self, instruction: u32) {
        todo!("sltiu");
    }

    pub fn andi(&mut self, instruction: u32) {
        todo!("andi");
    }

    pub fn ori(&mut self, instruction: u32) {
        todo!("ori");
    }

    pub fn xori(&mut self, instruction: u32) {
        todo!("xori");
    }

    pub fn lui(&mut self, instruction: u32) {
        todo!("lui");
    }

    pub fn cop0(&mut self, instruction: u32) {
        todo!("cop0");
    }

    pub fn cop1(&mut self, instruction: u32) {
        todo!("cop1");
    }

    pub fn cop2(&mut self, instruction: u32) {
        todo!("cop2");
    }

    pub fn cop3(&mut self, instruction: u32) {
        todo!("cop3");
    }

    pub fn lb(&mut self, instruction: u32) {
        todo!("lb");
    }

    pub fn lh(&mut self, instruction: u32) {
        todo!("lh");
    }

    pub fn lwl(&mut self, instruction: u32) {
        todo!("lwl");
    }

    pub fn lw(&mut self, instruction: u32) {
        todo!("lw");
    }

    pub fn lbu(&mut self, instruction: u32) {
        todo!("lbu");
    }

    pub fn lhu(&mut self, instruction: u32) {
        todo!("lhu");
    }

    pub fn lwr(&mut self, instruction: u32) {
        todo!("lwr");
    }

    pub fn sb(&mut self, instruction: u32) {
        todo!("sb");
    }

    pub fn sh(&mut self, instruction: u32) {
        todo!("sh");
    }

    pub fn swl(&mut self, instruction: u32) {
        todo!("swl");
    }

    pub fn sw(&mut self, instruction: u32) {
        todo!("sw");
    }

    pub fn swr(&mut self, instruction: u32) {
        todo!("swr");
    }

    pub fn lwc0(&mut self, instruction: u32) {
        todo!("lwc0");
    }

    pub fn lwc1(&mut self, instruction: u32) {
        todo!("lwc1");
    }

    pub fn lwc2(&mut self, instruction: u32) {
        todo!("lwc2");
    }

    pub fn lwc3(&mut self, instruction: u32) {
        todo!("lwc3");
    }

    pub fn swc0(&mut self, instruction: u32) {
        todo!("swc0");
    }

    pub fn swc1(&mut self, instruction: u32) {
        todo!("swc1");
    }

    pub fn swc2(&mut self, instruction: u32) {
        todo!("swc2");
    }

    pub fn swc3(&mut self, instruction: u32) {
        todo!("swc3");
    }

    pub fn sll(&mut self, instruction: u32) {
        todo!("sll");
    }

    pub fn srl(&mut self, instruction: u32) {
        todo!("srl");
    }

    pub fn sra(&mut self, instruction: u32) {
        todo!("sra");
    }

    pub fn sllv(&mut self, instruction: u32) {
        todo!("sllv");
    }

    pub fn srlv(&mut self, instruction: u32) {
        todo!("srlv");
    }

    pub fn srav(&mut self, instruction: u32) {
        todo!("srav");
    }

    pub fn jr(&mut self, instruction: u32) {
        todo!("jr");
    }

    pub fn jalr(&mut self, instruction: u32) {
        todo!("jalr");
    }

    pub fn syscall(&mut self, instruction: u32) {
        todo!("syscall");
    }

    pub fn break_(&mut self, instruction: u32) {
        todo!("break");
    }

    pub fn mfhi(&mut self, instruction: u32) {
        todo!("mfhi");
    }

    pub fn mthi(&mut self, instruction: u32) {
        todo!("mthi");
    }

    pub fn mflo(&mut self, instruction: u32) {
        todo!("mflo");
    }

    pub fn mtlo(&mut self, instruction: u32) {
        todo!("mtl0");
    }

    pub fn mult(&mut self, instruction: u32) {
        todo!("mult");
    }

    pub fn multu(&mut self, instruction: u32) {
        todo!("multu");
    }

    pub fn div(&mut self, instruction: u32) {
        todo!("div");
    }

    pub fn divu(&mut self, instruction: u32) {
        todo!("divu");
    }

    pub fn add(&mut self, instruction: u32) {
        todo!("add");
    }

    pub fn addu(&mut self, instruction: u32) {
        todo!("addu");
    }

    pub fn sub(&mut self, instruction: u32) {
        todo!("sub");
    }

    pub fn subu(&mut self, instruction: u32) {
        todo!("subu");
    }

    pub fn and(&mut self, instruction: u32) {
        todo!("and");
    }

    pub fn or(&mut self, instruction: u32) {
        todo!("or");
    }

    pub fn xor(&mut self, instruction: u32) {
        todo!("xor");
    }

    pub fn nor(&mut self, instruction: u32) {
        todo!("nor");
    }

    pub fn slt(&mut self, instruction: u32) {
        todo!("slt");
    }

    pub fn sltu(&mut self, instruction: u32) {
        todo!("sltu");
    }
}