use std::{cmp, collections::HashMap};

use super::Instruction;


// see https://psx-spx.consoledev.net/geometrytransformationenginegte/#gte-division-inaccuracy
const UNR_TABLE: [u8; 0x101] = [
    0xFF, 0xFD, 0xFB, 0xF9, 0xF7, 0xF5, 0xF3, 0xF1, 0xEF, 0xEE, 0xEC, 0xEA, 0xE8, 0xE6, 0xE4, 0xE3,
    0xE1, 0xDF, 0xDD, 0xDC, 0xDA, 0xD8, 0xD6, 0xD5, 0xD3, 0xD1, 0xD0, 0xCE, 0xCD, 0xCB, 0xC9, 0xC8,
    0xC6, 0xC5, 0xC3, 0xC1, 0xC0, 0xBE, 0xBD, 0xBB, 0xBA, 0xB8, 0xB7, 0xB5, 0xB4, 0xB2, 0xB1, 0xB0,
    0xAE, 0xAD, 0xAB, 0xAA, 0xA9, 0xA7, 0xA6, 0xA4, 0xA3, 0xA2, 0xA0, 0x9F, 0x9E, 0x9C, 0x9B, 0x9A,
    0x99, 0x97, 0x96, 0x95, 0x94, 0x92, 0x91, 0x90, 0x8F, 0x8D, 0x8C, 0x8B, 0x8A, 0x89, 0x87, 0x86,
    0x85, 0x84, 0x83, 0x82, 0x81, 0x7F, 0x7E, 0x7D, 0x7C, 0x7B, 0x7A, 0x79, 0x78, 0x77, 0x75, 0x74,
    0x73, 0x72, 0x71, 0x70, 0x6F, 0x6E, 0x6D, 0x6C, 0x6B, 0x6A, 0x69, 0x68, 0x67, 0x66, 0x65, 0x64,
    0x63, 0x62, 0x61, 0x60, 0x5F, 0x5E, 0x5D, 0x5D, 0x5C, 0x5B, 0x5A, 0x59, 0x58, 0x57, 0x56, 0x55,
    0x54, 0x53, 0x53, 0x52, 0x51, 0x50, 0x4F, 0x4E, 0x4D, 0x4D, 0x4C, 0x4B, 0x4A, 0x49, 0x48, 0x48,
    0x47, 0x46, 0x45, 0x44, 0x43, 0x43, 0x42, 0x41, 0x40, 0x3F, 0x3F, 0x3E, 0x3D, 0x3C, 0x3C, 0x3B,
    0x3A, 0x39, 0x39, 0x38, 0x37, 0x36, 0x36, 0x35, 0x34, 0x33, 0x33, 0x32, 0x31, 0x31, 0x30, 0x2F,
    0x2E, 0x2E, 0x2D, 0x2C, 0x2C, 0x2B, 0x2A, 0x2A, 0x29, 0x28, 0x28, 0x27, 0x26, 0x26, 0x25, 0x24,
    0x24, 0x23, 0x22, 0x22, 0x21, 0x20, 0x20, 0x1F, 0x1E, 0x1E, 0x1D, 0x1D, 0x1C, 0x1B, 0x1B, 0x1A,
    0x19, 0x19, 0x18, 0x18, 0x17, 0x16, 0x16, 0x15, 0x15, 0x14, 0x14, 0x13, 0x12, 0x12, 0x11, 0x11,
    0x10, 0x0F, 0x0F, 0x0E, 0x0E, 0x0D, 0x0D, 0x0C, 0x0C, 0x0B, 0x0A, 0x0A, 0x09, 0x09, 0x08, 0x08,
    0x07, 0x07, 0x06, 0x06, 0x05, 0x05, 0x04, 0x04, 0x03, 0x03, 0x02, 0x02, 0x01, 0x01, 0x00, 0x00,
    0x00
];

#[derive(Clone, Copy, Debug)]
struct Rgb {
    r: u8,
    g: u8,
    b: u8,
    c: u8
}

pub struct Gte {
    pub debug_on: bool,
    executed_commands: HashMap<u32, bool>,
    zsf3: i16,
    zsf4: i16,
    h: u16,
    dqa: i16,
    dqb: i32,
    ofx: i32,
    ofy: i32,
    fc: (i32, i32, i32),
    bk: (i32, i32, i32),
    tr: (i32, i32, i32),
    color: [[i16; 3]; 3],
    light: [[i16; 3]; 3],
    rotation: [[i16; 3]; 3],
    v: [(i16, i16, i16); 3],
    rgbc: Rgb,
    otz: u16,
    ir: [i16; 4],
    flags: u32,
    sf: usize,
    mx: usize,
    sv: usize,
    cv: usize,
    lm: bool,
    sxy_fifo: [(i16, i16); 3],
    sz_fifo: [u16; 4],
    rgb_fifo: [Rgb; 3],
    res1: u32,
    mac: [i32; 4],
    lzcs: i32,
    lzcr: i32
}

impl Gte {
    pub fn new() -> Self {
        Self {
            zsf3: 0,
            zsf4: 0,
            h: 0,
            dqa: 0,
            dqb: 0,
            ofx: 0,
            ofy: 0,
            fc: (0, 0, 0),
            bk: (0, 0, 0),
            color: [[0; 3]; 3],
            light: [[0; 3]; 3],
            rotation: [[0; 3]; 3],
            tr: (0, 0, 0),
            v: [(0,0,0); 3],
            rgbc: Rgb {
                r: 0,
                g: 0,
                b: 0,
                c: 0
            },
            otz: 0,
            ir: [0; 4],
            flags: 0,
            sf: 0,
            mx: 0,
            sv: 0,
            cv: 0,
            lm: false,
            sxy_fifo: [(0, 0); 3],
            sz_fifo: [0; 4],
            rgb_fifo: [Rgb { r: 0, g: 0, b: 0, c: 0 }; 3],
            res1: 0,
            mac: [0; 4],
            lzcs: 0,
            lzcr: 0,
            debug_on: false,
            executed_commands: HashMap::new()
        }
    }

    pub fn execute_command(&mut self, instr: Instruction) {
        let command = instr.cop2_command();

        let op_code = command & 0x3f;

        self.sf = if (command >> 19) & 0b1 == 1 {
            12
        } else {
            0
        };

        self.mx = ((command >> 17) & 0x3) as usize;
        self.sv = ((command >> 15) & 0x3) as usize;
        self.cv = ((command >> 13) & 0x3) as usize;

        self.lm = (command >> 10) & 0b1 == 1;

        self.flags = 0;

        if self.debug_on {
            if !self.executed_commands.contains_key(&op_code) {
                println!("executed {:X}", op_code);
                self.executed_commands.insert(op_code, true);
            }

        }

        match op_code {
            0x01 => self.rtps(),
            0x06 => self.nclip(),
            0x0c => self.op(),
            0x10 => self.dpcs(),
            0x11 => self.intpl(),
            0x12 => self.mvmva(),
            0x13 => self.ncds(),
            0x14 => self.cdp(),
            0x16 => self.ncdt(),
            0x1b => self.nccs(),
            0x1c => self.cc(),
            0x1e => self.ncs(),
            0x20 => self.nct(),
            0x28 => self.sqr(),
            0x29 => self.dpcl(),
            0x2a => self.dpct(),
            0x2d => self.avsz3(),
            0x2e => self.avsz4(),
            0x30 => self.rtpt(),
            0x3d => self.gpf(),
            0x3e => self.gpl(),
            0x3f => self.ncct(),
            _ => panic!("unimplemented op code for gte: {:x}", op_code)
        }

        if (self.flags & 0x7f87e000) != 0 {
            self.flags |= 1 << 31;
        }
    }

    fn cdp(&mut self) {
        let bk_x = (self.bk.0 as i64) << 12;
        let bk_y = (self.bk.1 as i64) << 12;
        let bk_z = (self.bk.2 as i64) << 12;

        let c11 = self.color[0][0] as i64;
        let c12 = self.color[0][1] as i64;
        let c13 = self.color[0][2] as i64;

        let c21 = self.color[1][0] as i64;
        let c22 = self.color[1][1] as i64;
        let c23 = self.color[1][2] as i64;

        let c31 = self.color[2][0] as i64;
        let c32 = self.color[2][1] as i64;
        let c33 = self.color[2][2] as i64;

        let ir1 = self.ir[1] as i64;
        let ir2 = self.ir[2] as i64;
        let ir3 = self.ir[3] as i64;

        let mut mac1 = self.set_mac_flags(bk_x + c11 * ir1, 1);
        let mut mac2 = self.set_mac_flags(bk_y + c21 * ir1, 2);
        let mut mac3 = self.set_mac_flags(bk_z + c31 * ir1, 3);

        mac1 = self.set_mac_flags(mac1 + c12 * ir2, 1);
        mac2 = self.set_mac_flags(mac2 + c22 * ir2, 2);
        mac3 = self.set_mac_flags(mac3 + c32 * ir2, 3);

        mac1 = self.set_mac_flags(mac1 + c13 * ir3, 1);
        mac2 = self.set_mac_flags(mac2 + c23 * ir3, 2);
        mac3 = self.set_mac_flags(mac3 + c33 * ir3, 3);

        self.mac[1] = (mac1 >> self.sf) as i32;
        self.mac[2] = (mac2 >> self.sf) as i32;
        self.mac[3] = (mac3 >> self.sf) as i32;

        for i in 1..4 {
            self.ir[i] = self.set_ir_flags(self.mac[i], i, self.lm);
        }

        let r = (self.rgbc.r as i64) << 4;
        let g = (self.rgbc.g as i64) << 4;
        let b = (self.rgbc.b as i64) << 4;
        let c = self.rgbc.c;

        let fc_x = (self.fc.0 as i64) << 12;
        let fc_y = (self.fc.1 as i64) << 12;
        let fc_z = (self.fc.2 as i64) << 12;

        let ir0 = self.ir[0] as i64;
        let ir1 = self.ir[1] as i64;
        let ir2 = self.ir[2] as i64;
        let ir3 = self.ir[3] as i64;

        self.mac[1] = (self.set_mac_flags(fc_x - r * ir1, 1) >> self.sf) as i32;
        self.mac[2] = (self.set_mac_flags(fc_y - g * ir2, 2) >> self.sf) as i32;
        self.mac[3] = (self.set_mac_flags(fc_z - b * ir3, 3) >> self.sf) as i32;

        let temp1 = self.set_ir_flags(self.mac[1], 1, false) as i64;
        let temp2 = self.set_ir_flags(self.mac[2], 2, false) as i64;
        let temp3 = self.set_ir_flags(self.mac[3], 3, false) as i64;

        self.mac[1] = (self.set_mac_flags(r * ir1 + ir0 * temp1, 1) >> self.sf) as i32;
        self.mac[2] = (self.set_mac_flags(g * ir2 + ir0 * temp2, 1) >> self.sf) as i32;
        self.mac[3] = (self.set_mac_flags(b * ir3 + ir0 * temp3, 1) >> self.sf) as i32;

        let r = self.set_color_fifo_flags(self.mac[1] >> 4, 1);
        let g = self.set_color_fifo_flags(self.mac[2] >> 4, 2);
        let b = self.set_color_fifo_flags(self.mac[3] >> 4, 3);

        self.push_rgb(r, g, b, c);

        for i in 1..4 {
            self.ir[i] = self.set_ir_flags(self.mac[i], i, self.lm);
        }
    }

    fn cc(&mut self) {
        let bk_x = (self.bk.0 as i64) << 12;
        let bk_y = (self.bk.1 as i64) << 12;
        let bk_z = (self.bk.2 as i64) << 12;

        let c11 = self.color[0][0] as i64;
        let c12 = self.color[0][1] as i64;
        let c13 = self.color[0][2] as i64;

        let c21 = self.color[1][0] as i64;
        let c22 = self.color[1][1] as i64;
        let c23 = self.color[1][2] as i64;

        let c31 = self.color[2][0] as i64;
        let c32 = self.color[2][1] as i64;
        let c33 = self.color[2][2] as i64;

        let ir1 = self.ir[1] as i64;
        let ir2 = self.ir[2] as i64;
        let ir3 = self.ir[3] as i64;

        let mut mac1 = self.set_mac_flags(bk_x + c11 * ir1, 1);
        let mut mac2 = self.set_mac_flags(bk_y + c21 * ir1, 2);
        let mut mac3 = self.set_mac_flags(bk_z + c31 * ir1, 3);

        mac1 = self.set_mac_flags(mac1 + c12 * ir2, 1);
        mac2 = self.set_mac_flags(mac2 + c22 * ir2, 2);
        mac3 = self.set_mac_flags(mac3 + c32 * ir2, 3);

        mac1 = self.set_mac_flags(mac1 + c13 * ir3, 1);
        mac2 = self.set_mac_flags(mac2 + c23 * ir3, 2);
        mac3 = self.set_mac_flags(mac3 + c33 * ir3, 3);

        self.mac[1] = (mac1 >> self.sf) as i32;
        self.mac[2] = (mac2 >> self.sf) as i32;
        self.mac[3] = (mac3 >> self.sf) as i32;

        for i in 1..4 {
            self.ir[i] = self.set_ir_flags(self.mac[i], i, self.lm);
        }

        let r = (self.rgbc.r as i64) << 4;
        let g = (self.rgbc.g as i64) << 4;
        let b = (self.rgbc.b as i64) << 4;
        let c = self.rgbc.c;

        self.mac[1] = (self.set_mac_flags(r * self.ir[1] as i64, 1) >> self.sf) as i32;
        self.mac[2] = (self.set_mac_flags(g * self.ir[2] as i64, 2) >> self.sf) as i32;
        self.mac[3] = (self.set_mac_flags(b * self.ir[3] as i64, 3) >> self.sf) as i32;

        let r = self.set_color_fifo_flags(self.mac[1] >> 4, 1);
        let g = self.set_color_fifo_flags(self.mac[2] >> 4, 2);
        let b = self.set_color_fifo_flags(self.mac[3] >> 4, 3);

        self.push_rgb(r, g, b, c);

        for i in 1..4 {
            self.ir[i] = self.set_ir_flags(self.mac[i], i, self.lm);
        }
    }

    fn dpcl(&mut self) {
        let r = (self.rgbc.r as i64) << 4;
        let g = (self.rgbc.g as i64) << 4;
        let b = (self.rgbc.b as i64) << 4;
        let c = self.rgbc.c;

        let ir1 = self.ir[1] as i64;
        let ir2 = self.ir[2] as i64;
        let ir3 = self.ir[3] as i64;

        let fc_x = (self.fc.0 as i64) << 12;
        let fc_y = (self.fc.1 as i64) << 12;
        let fc_z = (self.fc.2 as i64) << 12;

        self.mac[1] = (self.set_mac_flags(fc_x - r * ir1, 1) >> self.sf) as i32;
        self.mac[2] = (self.set_mac_flags(fc_y - g * ir2, 2) >> self.sf) as i32;
        self.mac[3] = (self.set_mac_flags(fc_z - b * ir3, 3) >> self.sf) as i32;

        let temp1 = self.set_ir_flags(self.mac[1], 1, false) as i64;
        let temp2 = self.set_ir_flags(self.mac[2], 2, false) as i64;
        let temp3 = self.set_ir_flags(self.mac[3], 3, false) as i64;

        let ir0 = self.ir[0] as i64;

        self.mac[1] = (self.set_mac_flags(r * ir1 + ir0 * temp1, 1) >> self.sf) as i32;
        self.mac[2] = (self.set_mac_flags(g * ir2 + ir0 * temp2, 2) >> self.sf) as i32;
        self.mac[3] = (self.set_mac_flags(b * ir3 + ir0 * temp3, 3) >> self.sf) as i32;

        let r = self.set_color_fifo_flags(self.mac[1] >> 4, 1);
        let g = self.set_color_fifo_flags(self.mac[2] >> 4, 2);
        let b = self.set_color_fifo_flags(self.mac[3] >> 4, 3);

        self.push_rgb(r, g, b, c);

        for i in 1..4 {
            self.ir[i] = self.set_ir_flags(self.mac[i], i, self.lm);
        }

    }

    fn ncs(&mut self) {
        self.nc(0);
    }

    fn nct(&mut self) {
        self.nc(0);
        self.nc(1);
        self.nc(2);
    }

    fn nc(&mut self, index: usize) {
        let light_m11 = self.light[0][0] as i64;
        let light_m12 = self.light[0][1] as i64;
        let light_m13 = self.light[0][2] as i64;

        let light_m21 = self.light[1][0] as i64;
        let light_m22 = self.light[1][1] as i64;
        let light_m23 = self.light[1][2] as i64;

        let light_m31 = self.light[2][0] as i64;
        let light_m32 = self.light[2][1] as i64;
        let light_m33 = self.light[2][2] as i64;

        let vx = self.v[index].0 as i64;
        let vy = self.v[index].1 as i64;
        let vz = self.v[index].2 as i64;

        let mut mac1 = self.set_mac_flags(vx * light_m11, 1);
        let mut mac2 = self.set_mac_flags(vx * light_m21, 2);
        let mut mac3 = self.set_mac_flags(vx * light_m31, 3);

        mac1 = self.set_mac_flags(mac1 + vy * light_m12, 1);
        mac2 = self.set_mac_flags(mac2 + vy * light_m22, 2);
        mac3 = self.set_mac_flags(mac3 + vy * light_m32, 3);

        mac1 = self.set_mac_flags(mac1 + vz * light_m13, 1);
        mac2 = self.set_mac_flags(mac2 + vz * light_m23, 2);
        mac3 = self.set_mac_flags(mac3 + vz * light_m33, 3);

        self.mac[1] = (mac1 >> self.sf) as i32;
        self.mac[2] = (mac2 >> self.sf) as i32;
        self.mac[3] = (mac3 >> self.sf) as i32;

        for i in 1..4 {
            self.ir[i] = self.set_ir_flags(self.mac[i], i, self.lm);
        }

        let bk_x = (self.bk.0 as i64) << 12;
        let bk_y = (self.bk.1 as i64) << 12;
        let bk_z = (self.bk.2 as i64) << 12;

        let color_m11 = self.color[0][0] as i64;
        let color_m12 = self.color[0][1] as i64;
        let color_m13 = self.color[0][2] as i64;

        let color_m21 = self.color[1][0] as i64;
        let color_m22 = self.color[1][1] as i64;
        let color_m23 = self.color[1][2] as i64;

        let color_m31 = self.color[2][0] as i64;
        let color_m32 = self.color[2][1] as i64;
        let color_m33 = self.color[2][2] as i64;

        let ir1 = self.ir[1] as i64;
        let ir2 = self.ir[2] as i64;
        let ir3 = self.ir[3] as i64;

        let mut mac1 = self.set_mac_flags(bk_x + color_m11 * ir1, 1);
        let mut mac2 = self.set_mac_flags(bk_y + color_m21 * ir1, 2);
        let mut mac3 = self.set_mac_flags(bk_z + color_m31 * ir1, 3);

        mac1 = self.set_mac_flags(mac1 + color_m12 * ir2, 1);
        mac2 = self.set_mac_flags(mac2 + color_m22 * ir2, 2);
        mac3 = self.set_mac_flags(mac3 + color_m32 * ir2, 3);

        mac1 = self.set_mac_flags(mac1 + color_m13 * ir3, 1);
        mac2 = self.set_mac_flags(mac2 + color_m23 * ir3, 2);
        mac3 = self.set_mac_flags(mac3 + color_m33 * ir3, 3);

        self.mac[1] = (mac1 >> self.sf) as i32;
        self.mac[2] = (mac2 >> self.sf) as i32;
        self.mac[3] = (mac3 >> self.sf) as i32;

        for i in 1..4 {
            self.ir[i] = self.set_ir_flags(self.mac[i], i, self.lm);
        }

        let r = self.set_color_fifo_flags(self.mac[1] >> 4, 1);
        let g = self.set_color_fifo_flags(self.mac[2] >> 4, 2);
        let b = self.set_color_fifo_flags(self.mac[3] >> 4, 3);
        let c = self.rgbc.c;

        self.push_rgb(r, g, b, c);

    }

    fn op(&mut self) {
        let ir1 = self.ir[1] as i64;
        let ir2 = self.ir[2] as i64;
        let ir3 = self.ir[3] as i64;

        let d1 = self.rotation[0][0] as i64;
        let d2 = self.rotation[1][1] as i64;
        let d3 = self.rotation[2][2] as i64;

        let mac1 = self.set_mac_flags(ir3 * d2 - ir2 * d3, 1);
        let mac2 = self.set_mac_flags(ir1 * d3 - ir3 * d1, 2);
        let mac3 = self.set_mac_flags(ir2 * d1 - ir1 * d2, 3);

        self.mac[1] = (mac1 >> self.sf) as i32;
        self.mac[2] = (mac2 >> self.sf) as i32;
        self.mac[3] = (mac3 >> self.sf) as i32;

        self.ir[1] = self.set_ir_flags(self.mac[1], 1, self.lm);
        self.ir[2] = self.set_ir_flags(self.mac[2], 2, self.lm);
        self.ir[3] = self.set_ir_flags(self.mac[3], 3, self.lm);
    }

    fn dpcs(&mut self) {
        self.dpc(false);
    }

    fn dpct(&mut self) {
        self.dpc(true);
        self.dpc(true);
        self.dpc(true);
    }

    fn dpc(&mut self, is_triple: bool) {
        let rgbc = if is_triple {
            self.rgb_fifo[0]
        } else {
            self.rgbc
        };

        let r = (rgbc.r as i64) << 16;
        let g = (rgbc.g as i64) << 16;
        let b = (rgbc.b as i64) << 16;
        let c = self.rgbc.c;

        let fc_x = (self.fc.0 as i64) << 12;
        let fc_y = (self.fc.1 as i64) << 12;
        let fc_z = (self.fc.2 as i64) << 12;

        self.mac[1] = (self.set_mac_flags(fc_x - r, 1) >> self.sf) as i32;
        self.mac[2] = (self.set_mac_flags(fc_y - g, 2) >> self.sf) as i32;
        self.mac[3] = (self.set_mac_flags(fc_z - b, 3) >> self.sf) as i32;

        for i in 1..4 {
            self.ir[i] = self.set_ir_flags(self.mac[i], i, false);
        }

        let ir0 = self.ir[0] as i64;
        let ir1 = self.ir[1] as i64;
        let ir2 = self.ir[2] as i64;
        let ir3 = self.ir[3] as i64;

        self.mac[1] = (self.set_mac_flags(r + ir1 * ir0, 1) >> self.sf) as i32;
        self.mac[2] = (self.set_mac_flags(g + ir2 * ir0, 2) >> self.sf) as i32;
        self.mac[3] = (self.set_mac_flags(b + ir3 * ir0, 3) >> self.sf) as i32;

        let r = self.set_color_fifo_flags(self.mac[1] >> 4, 1);
        let g = self.set_color_fifo_flags(self.mac[2] >> 4, 2);
        let b = self.set_color_fifo_flags(self.mac[3] >> 4, 3);

        self.push_rgb(r, g, b, c);

        for i in 1..4 {
            self.ir[i] = self.set_ir_flags(self.mac[i], i, self.lm);
        }
    }

    fn sqr(&mut self) {
        let ir1 = self.ir[1] as i64;
        let ir2 = self.ir[2] as i64;
        let ir3 = self.ir[3] as i64;

        self.mac[1] = (self.set_mac_flags(ir1 * ir1, 1) >> self.sf) as i32;
        self.mac[2] = (self.set_mac_flags(ir2 * ir2, 2) >> self.sf) as i32;
        self.mac[3] = (self.set_mac_flags(ir3 * ir3, 3) >> self.sf) as i32;

        self.ir[1] = self.set_ir_flags(self.mac[1], 1, self.lm);
        self.ir[2] = self.set_ir_flags(self.mac[2], 2, self.lm);
        self.ir[3] = self.set_ir_flags(self.mac[3], 3, self.lm);
    }

    fn nccs(&mut self) {
        self.ncc(0);
    }

    fn ncct(&mut self) {
        self.ncc(0);
        self.ncc(1);
        self.ncc(2);
    }

    fn ncc(&mut self, index: usize) {
        let light_m11 = self.light[0][0] as i64;
        let light_m12 = self.light[0][1] as i64;
        let light_m13 = self.light[0][2] as i64;

        let light_m21 = self.light[1][0] as i64;
        let light_m22 = self.light[1][1] as i64;
        let light_m23 = self.light[1][2] as i64;

        let light_m31 = self.light[2][0] as i64;
        let light_m32 = self.light[2][1] as i64;
        let light_m33 = self.light[2][2] as i64;

        let vx = self.v[index].0 as i64;
        let vy = self.v[index].1 as i64;
        let vz = self.v[index].2 as i64;

        let mut mac1 = self.set_mac_flags(light_m11 * vx, 1);
        let mut mac2 = self.set_mac_flags(light_m21 * vx, 2);
        let mut mac3 = self.set_mac_flags(light_m31 * vx, 3);

        mac1 = self.set_mac_flags(mac1 + light_m12 * vy, 1);
        mac2 = self.set_mac_flags(mac2 + light_m22 * vy, 2);
        mac3 = self.set_mac_flags(mac3 + light_m32 * vy, 3);

        mac1 = self.set_mac_flags(mac1 + light_m13 * vz, 1);
        mac2 = self.set_mac_flags(mac2 + light_m23 * vz, 2);
        mac3 = self.set_mac_flags(mac3 + light_m33 * vz, 3);

        self.mac[1] = (mac1 >> self.sf) as i32;
        self.mac[2] = (mac2 >> self.sf) as i32;
        self.mac[3] = (mac3 >> self.sf) as i32;

        for i in 1..4 {
            self.ir[i] = self.set_ir_flags(self.mac[i], i, self.lm);
        }

        let color_m11 = self.color[0][0] as i64;
        let color_m12 = self.color[0][1] as i64;
        let color_m13 = self.color[0][2] as i64;

        let color_m21 = self.color[1][0] as i64;
        let color_m22 = self.color[1][1] as i64;
        let color_m23 = self.color[1][2] as i64;

        let color_m31 = self.color[2][0] as i64;
        let color_m32 = self.color[2][1] as i64;
        let color_m33 = self.color[2][2] as i64;

        let bk_x = self.bk.0 as i64;
        let bk_y = self.bk.1 as i64;
        let bk_z = self.bk.2 as i64;

        let ir1 = self.ir[1] as i64;
        let ir2 = self.ir[2] as i64;
        let ir3 = self.ir[3] as i64;

        mac1 = self.set_mac_flags(bk_x * 0x1000 + color_m11 * ir1, 1);
        mac2 = self.set_mac_flags(bk_y * 0x1000 + color_m21 * ir1, 2);
        mac3 = self.set_mac_flags(bk_z * 0x1000 + color_m31 * ir1, 3);

        mac1 = self.set_mac_flags(mac1 + color_m12 * ir2, 1);
        mac2 = self.set_mac_flags(mac2 + color_m22 * ir2, 2);
        mac3 = self.set_mac_flags(mac3 + color_m32 * ir2, 3);

        mac1 = self.set_mac_flags(mac1 + color_m13 * ir3, 1);
        mac2 = self.set_mac_flags(mac2 + color_m23 * ir3, 2);
        mac3 = self.set_mac_flags(mac3 + color_m33 * ir3, 3);

        self.mac[1] = (mac1 >> self.sf) as i32;
        self.mac[2] = (mac2 >> self.sf) as i32;
        self.mac[3] = (mac3 >> self.sf) as i32;

        for i in 1..4 {
            self.ir[i] = self.set_ir_flags(self.mac[i], i, self.lm);
        }

        let r = self.rgbc.r as i64;
        let g = self.rgbc.g as i64;
        let b = self.rgbc.b as i64;

        let ir1 = self.ir[1] as i64;
        let ir2 = self.ir[2] as i64;
        let ir3 = self.ir[3] as i64;

        self.mac[1] = (self.set_mac_flags((r * ir1) << 4, 1) >> self.sf) as i32;
        self.mac[2] = (self.set_mac_flags((g * ir2) << 4, 2) >> self.sf) as i32;
        self.mac[3] = (self.set_mac_flags((b * ir3) << 4, 3) >> self.sf) as i32;

        for i in 1..4 {
            self.ir[i] = self.set_ir_flags(self.mac[i], i, self.lm);
        }

        let r = self.set_color_fifo_flags(self.mac[1] >> 4, 1);
        let g = self.set_color_fifo_flags(self.mac[2] >> 4, 2);
        let b = self.set_color_fifo_flags(self.mac[3] >> 4, 3);
        let c = self.rgbc.c;

        self.push_rgb(r, g, b, c);
    }

    fn gpf(&mut self) {
        self.interpolate(0, 0, 0);
    }

    fn intpl(&mut self) {
        let mac1 = (self.ir[1] as i64) << 12;
        let mac2 = (self.ir[2] as i64) << 12;
        let mac3 = (self.ir[3] as i64) << 12;

        let fc_x = (self.fc.0 as i64) << 12;
        let fc_y = (self.fc.1 as i64) << 12;
        let fc_z = (self.fc.2 as i64) << 12;

        let ir0 = self.ir[0] as i64;

        self.mac[1] = (self.set_mac_flags(fc_x - mac1, 1) >> self.sf) as i32;
        self.mac[2] = (self.set_mac_flags(fc_y - mac2, 2) >> self.sf) as i32;
        self.mac[3] = (self.set_mac_flags(fc_z - mac3, 3) >> self.sf) as i32;

        for i in 1..4 {
            self.ir[i] = self.set_ir_flags(self.mac[i], i, false);
        }

        let ir1 = self.ir[1] as i64;
        let ir2 = self.ir[2] as i64;
        let ir3 = self.ir[3] as i64;

        self.mac[1] = (self.set_mac_flags(mac1 + ir1 * ir0, 1) >> self.sf) as i32;
        self.mac[2] = (self.set_mac_flags(mac2 + ir2 * ir0, 2) >> self.sf) as i32;
        self.mac[3] = (self.set_mac_flags(mac3 + ir3 * ir0, 3) >> self.sf) as i32;


        let r = self.set_color_fifo_flags(self.mac[1] >> 4, 1);
        let g = self.set_color_fifo_flags(self.mac[2] >> 4, 2);
        let b = self.set_color_fifo_flags(self.mac[3] >> 4, 3);
        let c = self.rgbc.c;

        self.push_rgb(r, g, b, c);

        for i in 1..4 {
            self.ir[i] = self.set_ir_flags(self.mac[i], i, self.lm);
        }

    }

    fn gpl(&mut self) {
        let mac1 = (self.mac[1] as i64) << self.sf;
        let mac2 = (self.mac[2] as i64) << self.sf;
        let mac3 = (self.mac[3] as i64) << self.sf;

        self.interpolate(mac1, mac2, mac3);
    }

    fn interpolate(&mut self, mac1: i64, mac2: i64, mac3: i64) {
        let ir1 = self.ir[1] as i64;
        let ir2 = self.ir[2] as i64;
        let ir3 = self.ir[3] as i64;

        let ir0 = self.ir[0] as i64;

        let temp1 = self.set_mac_flags(ir1 * ir0 + mac1, 1);
        let temp2 = self.set_mac_flags(ir2 * ir0 + mac2, 2);
        let temp3 = self.set_mac_flags(ir3 * ir0 + mac3, 3);

        self.mac[1] = (temp1 >> self.sf) as i32;
        self.mac[2] = (temp2 >> self.sf) as i32;
        self.mac[3] = (temp3 >> self.sf) as i32;

        let r = self.set_color_fifo_flags(self.mac[1] >> 4, 1);
        let g = self.set_color_fifo_flags(self.mac[2] >> 4, 2);
        let b = self.set_color_fifo_flags(self.mac[3] >> 4, 3);
        let c = self.rgbc.c;

        self.push_rgb(r, g, b, c);

        self.ir[1] = self.set_ir_flags(self.mac[1], 1, self.lm);
        self.ir[2] = self.set_ir_flags(self.mac[2], 2, self.lm);
        self.ir[3] = self.set_ir_flags(self.mac[3], 3, self.lm);
    }

    fn avsz3(&mut self) {
        let value = self.zsf3 as i64 * (self.sz_fifo[1] as i64 + self.sz_fifo[2] as i64 + self.sz_fifo[3] as i64);

        self.set_mac0_flags(value);

        self.mac[0] = value as i32;

        let otz = value >> 12;

        self.otz = self.set_sz3_or_otz_flags(otz);
    }

    fn avsz4(&mut self) {
        let value = self.zsf4 as i64 * (self.sz_fifo[0] as i64 + self.sz_fifo[1] as i64 + self.sz_fifo[2] as i64 + self.sz_fifo[3] as i64);

        self.set_mac0_flags(value);

        self.mac[0] = value as i32;

        let otz = value >> 12;

        self.otz = self.set_sz3_or_otz_flags(otz);
    }

    fn nclip(&mut self) {
        let value = (self.sxy_fifo[0].0 as i64 * self.sxy_fifo[1].1 as i64)
            + (self.sxy_fifo[1].0 as i64 * self.sxy_fifo[2].1 as i64)
            + (self.sxy_fifo[2].0 as i64 * self.sxy_fifo[0].1 as i64)
            - (self.sxy_fifo[0].0 as i64 * self.sxy_fifo[2].1 as i64)
            - (self.sxy_fifo[1].0 as i64 * self.sxy_fifo[0].1 as i64)
            - (self.sxy_fifo[2].0 as i64 * self.sxy_fifo[1].1 as i64);

        self.set_mac0_flags(value);

        self.mac[0] = value as i32;
    }

    fn ncds(&mut self) {
        self.ncd(0);
    }

    fn ncdt(&mut self) {
        self.ncd(0);
        self.ncd(1);
        self.ncd(2);
    }

    fn mvmva(&mut self) {
        let mx = match self.mx {
            0 => self.rotation,
            1 => self.light,
            2 => self.color,
            3 => [
                [-((self.rgbc.r as i16)    << 4), (self.rgbc.r as i16) << 4, self.ir[0] as i16],
                [self.rotation[0][2], self.rotation[0][2], self.rotation[0][2]],
                [self.rotation[1][1], self.rotation[1][1], self.rotation[1][1]]
            ],
            _ => unreachable!("can't happen")
        };

        let vx = match self.sv {
            0 => self.v[0],
            1 => self.v[1],
            2 => self.v[2],
            3 => (self.ir[1], self.ir[2], self.ir[3]),
            _ => unreachable!("can't happen")
        };

        let tx = match self.cv {
            0 => self.tr,
            1 => self.bk,
            2 => self.fc,
            3 => (0, 0, 0),
            _ => unreachable!("can't happen")
        };

        let mx_m11 = mx[0][0] as i64;
        let mx_m12 = mx[0][1] as i64;
        let mx_m13 = mx[0][2] as i64;

        let mx_m21 = mx[1][0] as i64;
        let mx_m22 = mx[1][1] as i64;
        let mx_m23 = mx[1][2] as i64;

        let mx_m31 = mx[2][0] as i64;
        let mx_m32 = mx[2][1] as i64;
        let mx_m33 = mx[2][2] as i64;

        let vx_x = vx.0 as i64;
        let vx_y = vx.1 as i64;
        let vx_z = vx.2 as i64;

        let tx_x = (tx.0 as i64) << 12;
        let tx_y = (tx.1 as i64) << 12;
        let tx_z = (tx.2 as i64) << 12;


        let mut mac1 = self.set_mac_flags(tx_x + mx_m11 * vx_x, 1);
        let mut mac2 = self.set_mac_flags(tx_y + mx_m21 * vx_x, 2);
        let mut mac3 = self.set_mac_flags(tx_z + mx_m31 * vx_x, 3);

        if self.cv == 2 {
            self.set_ir_flags((mac1 >> self.sf) as i32, 1, false);
            self.set_ir_flags((mac2 >> self.sf) as i32, 2, false);
            self.set_ir_flags((mac3 >> self.sf) as i32, 3, false);

            mac1 = 0;
            mac2 = 0;
            mac3 = 0;
        }

        mac1 = self.set_mac_flags(mac1 + mx_m12 * vx_y, 1);
        mac2 = self.set_mac_flags(mac2 + mx_m22 * vx_y, 2);
        mac3 = self.set_mac_flags(mac3 + mx_m32 * vx_y, 3);

        mac1 = self.set_mac_flags(mac1 + mx_m13 * vx_z, 1);
        mac2 = self.set_mac_flags(mac2 + mx_m23 * vx_z, 2);
        mac3 = self.set_mac_flags(mac3 + mx_m33 * vx_z, 3);

        self.mac[1] = (mac1 >> self.sf) as i32;
        self.mac[2] = (mac2 >> self.sf) as i32;
        self.mac[3] = (mac3 >> self.sf) as i32;

        self.ir[1] = self.set_ir_flags(self.mac[1], 1, self.lm);
        self.ir[2] = self.set_ir_flags(self.mac[2], 2, self.lm);
        self.ir[3] = self.set_ir_flags(self.mac[3], 3, self.lm);
    }

    fn ncd(&mut self, index: usize) {
        let l11 = self.light[0][0] as i64;
        let l12 = self.light[0][1] as i64;
        let l13 = self.light[0][2] as i64;

        let l21 = self.light[1][0] as i64;
        let l22 = self.light[1][1] as i64;
        let l23 = self.light[1][2] as i64;

        let l31 = self.light[2][0] as i64;
        let l32 = self.light[2][1] as i64;
        let l33 = self.light[2][2] as i64;

        let vx = self.v[index].0 as i64;
        let vy = self.v[index].1 as i64;
        let vz = self.v[index].2 as i64;

        let mut mac1 = self.set_mac_flags(l11 * vx, 1);
        let mut mac2 = self.set_mac_flags(l21 * vx, 2);
        let mut mac3 = self.set_mac_flags(l31 * vx, 3);

        mac1 = self.set_mac_flags(mac1 + l12 * vy, 1);
        mac2 = self.set_mac_flags(mac2 + l22 * vy, 2);
        mac3 = self.set_mac_flags(mac3 + l32 * vy, 3);

        mac1 = self.set_mac_flags(mac1 + l13 * vz, 1);
        mac2 = self.set_mac_flags(mac2 + l23 * vz, 2);
        mac3 = self.set_mac_flags(mac3 + l33 * vz, 3);

        self.mac[1] = (mac1 >> self.sf) as i32;
        self.mac[2] = (mac2 >> self.sf) as i32;
        self.mac[3] = (mac3 >> self.sf) as i32;

        self.ir[1] = self.set_ir_flags(self.mac[1], 1, self.lm);
        self.ir[2] = self.set_ir_flags(self.mac[2], 2, self.lm);
        self.ir[3] = self.set_ir_flags(self.mac[3], 3, self.lm);

        let rbk = (self.bk.0 as i64) << 12;
        let gbk = (self.bk.1 as i64) << 12;
        let bbk = (self.bk.2 as i64) << 12;

        let c11 = self.color[0][0] as i64;
        let c12 = self.color[0][1] as i64;
        let c13 = self.color[0][2] as i64;

        let c21 = self.color[1][0] as i64;
        let c22 = self.color[1][1] as i64;
        let c23 = self.color[1][2] as i64;

        let c31 = self.color[2][0] as i64;
        let c32 = self.color[2][1] as i64;
        let c33 = self.color[2][2] as i64;

        let ir1 = self.ir[1] as i64;
        let ir2 = self.ir[2] as i64;
        let ir3 = self.ir[3] as i64;

        let mut mac1 = self.set_mac_flags(rbk + c11 * ir1, 1);
        let mut mac2 = self.set_mac_flags(gbk + c21 * ir1, 2);
        let mut mac3 = self.set_mac_flags(bbk + c31 * ir1, 3);

        mac1 = self.set_mac_flags(mac1 + c12 * ir2, 1);
        mac2 = self.set_mac_flags(mac2 + c22 * ir2, 2);
        mac3 = self.set_mac_flags(mac3 + c32 * ir2, 3);

        mac1 = self.set_mac_flags(mac1 + c13 * ir3, 1);
        mac2 = self.set_mac_flags(mac2 + c23 * ir3, 2);
        mac3 = self.set_mac_flags(mac3 + c33 * ir3, 3);

        self.mac[1] = (mac1 >> self.sf) as i32;
        self.mac[2] = (mac2 >> self.sf) as i32;
        self.mac[3] = (mac3 >> self.sf) as i32;

        self.ir[1] = self.set_ir_flags(self.mac[1], 1, self.lm);
        self.ir[2] = self.set_ir_flags(self.mac[2], 2, self.lm);
        self.ir[3] = self.set_ir_flags(self.mac[3], 3, self.lm);

        let r = (self.rgbc.r as i64) << 4;
        let g = (self.rgbc.g as i64) << 4;
        let b = (self.rgbc.b as i64) << 4;

        let fcx = (self.fc.0 as i64) << 12;
        let fcy = (self.fc.1 as i64) << 12;
        let fcz = (self.fc.2 as i64) << 12;

        self.mac[1] = (self.set_mac_flags(fcx - r * self.ir[1] as i64, 1) >> self.sf) as i32;
        self.mac[2] = (self.set_mac_flags(fcy - g * self.ir[2] as i64, 2) >> self.sf) as i32;
        self.mac[3] = (self.set_mac_flags(fcz - b * self.ir[3] as i64, 3) >> self.sf) as i32;

        let previous_ir1 = self.ir[1] as i64;
        let previous_ir2 = self.ir[2] as i64;
        let previous_ir3 = self.ir[3] as i64;

        self.ir[1] = self.set_ir_flags(self.mac[1], 1, false);
        self.ir[2] = self.set_ir_flags(self.mac[2], 2, false);
        self.ir[3] = self.set_ir_flags(self.mac[3], 3, false);

        let ir0 = self.ir[0] as i64;
        let ir1 = self.ir[1] as i64;
        let ir2 = self.ir[2] as i64;
        let ir3 = self.ir[3] as i64;

        self.mac[1] = (self.set_mac_flags((r * previous_ir1) + ir0 * ir1, 1) >> self.sf) as i32;
        self.mac[2] = (self.set_mac_flags((g * previous_ir2) + ir0 * ir2, 2) >> self.sf) as i32;
        self.mac[3] = (self.set_mac_flags((b * previous_ir3) + ir0 * ir3, 3) >> self.sf) as i32;

        self.ir[1] = self.set_ir_flags(self.mac[1], 1, self.lm);
        self.ir[2] = self.set_ir_flags(self.mac[2], 2, self.lm);
        self.ir[3] = self.set_ir_flags(self.mac[3], 3, self.lm);

        let r = self.set_color_fifo_flags(self.mac[1] >> 4, 1);
        let g = self.set_color_fifo_flags(self.mac[2] >> 4, 2);
        let b = self.set_color_fifo_flags(self.mac[3] >> 4, 3);
        let c = self.rgbc.c;

        self.push_rgb(r,g,b,c);
    }

    fn push_rgb(&mut self, r: u8, g: u8, b: u8, c: u8) {
        self.rgb_fifo[0] = self.rgb_fifo[1];
        self.rgb_fifo[1] = self.rgb_fifo[2];

        self.rgb_fifo[2].r = r;
        self.rgb_fifo[2].g = g;
        self.rgb_fifo[2].b = b;
        self.rgb_fifo[2].c = c;
    }

    fn set_color_fifo_flags(&mut self, value: i32, index: usize) -> u8 {
        if value < 0 {
            self.flags |= 1 << (21 - (index - 1));
            return 0;
        }

        if value > 0xff {
            self.flags |= 1 << (21 - (index - 1));
            return 0xff;
        }

        value as u8
    }

    fn set_mac_flags(&mut self, value: i64, index: usize) -> i64 {
        let largest = 0x7ff_ffff_ffff;
        let smallest = -0x800_0000_0000;

        if value > largest {
            self.flags |= 1 << (30 - (index - 1));
        }
        if value < smallest {
            self.flags |= 1 << (27 - (index - 1));
        }

        (value << 20) >> 20
    }

    fn rtp(&mut self, index: usize, dq: bool) {
        let tr_x = (self.tr.0 as i64) << 12;
        let tr_y = (self.tr.1 as i64) << 12;
        let mut tr_z = (self.tr.2 as i64) << 12;

        let vx = self.v[index].0 as i64;
        let vy = self.v[index].1 as i64;
        let vz = self.v[index].2 as i64;

        let r11 = self.rotation[0][0] as i64;
        let r12 = self.rotation[0][1] as i64;
        let r13 = self.rotation[0][2] as i64;

        let r21 = self.rotation[1][0] as i64;
        let r22 = self.rotation[1][1] as i64;
        let r23 = self.rotation[1][2] as i64;

        let r31 = self.rotation[2][0] as i64;
        let r32 = self.rotation[2][1] as i64;
        let r33 = self.rotation[2][2] as i64;

        let mut ssx = self.set_mac_flags(tr_x + r11 * vx, 1);
        let mut ssy = self.set_mac_flags(tr_y + r21 * vx, 2);
        let mut ssz = self.set_mac_flags(tr_z + r31 * vx, 3);

        ssx = self.set_mac_flags(ssx + r12 * vy, 1);
        ssy = self.set_mac_flags(ssy + r22 * vy, 2);
        ssz = self.set_mac_flags(ssz + r32 * vy, 3);

        ssx = self.set_mac_flags(ssx + r13 * vz, 1);
        ssy = self.set_mac_flags(ssy + r23 * vz, 2);
        ssz = self.set_mac_flags(ssz + r33 * vz, 3);

        tr_z = ssz;

        let zs = tr_z >> 12;

        self.mac[1] = (ssx >> self.sf) as i32;
        self.mac[2] = (ssy >> self.sf) as i32;
        self.mac[3] = (ssz >> self.sf) as i32;

        self.ir[1] = self.set_ir_flags(self.mac[1], 1, self.lm);
        self.ir[2] = self.set_ir_flags(self.mac[2], 2, self.lm);
        // self.ir[3] = self.set_ir_flags(self.mac[3], 3);
        // not sure why ir3 checks the old value instead of the current like ir1 and 2, but this was
        // found in another emulator and it seems to break things if done otherwise
        self.ir[3] = self.set_ir_flag3(zs, self.mac[3]);

        let sz3 = self.set_sz3_or_otz_flags(zs);

        self.push_sz(sz3);

        // per https://psx-spx.consoledev.net/geometrytransformationenginegte/#gte-division-inaccuracy
        let h_divided_by_sz: u32 = if sz3 > (self.h / 2) {
            let leading_zeros = sz3.leading_zeros();
            let n = (self.h as u64) << leading_zeros;
            let mut d = (sz3 as u64) << leading_zeros;

            let u = UNR_TABLE[((d - 0x7fc0) >> 7) as usize] as u64 + 0x101;
            d = (0x2000080 - (d * u)) >> 8;
            d = (0x80 + (d * u)) >> 8;

            cmp::min(0x1_ffff, (((n * d) + 0x8000) >> 16) as u32)
        } else {
            self.flags |= 1 << 17;
            0x1_ffff
        };

        let mut sx2 = (self.ofx as i64) + (self.ir[1] as i64) * h_divided_by_sz as i64;
        let mut sy2 = (self.ofy as i64) + (self.ir[2] as i64) * h_divided_by_sz as i64;

        self.set_mac0_flags(sx2);
        self.set_mac0_flags(sy2);

        sx2 = sx2 >> 16;
        sy2 = sy2 >> 16;

        // finally saturate sx2 and sy2 to -0x400 to 0x3ff
        let sx2_saturated = self.set_sn_flags(sx2, 1);
        let sy2_saturated = self.set_sn_flags(sy2, 2);

        self.push_sx(sx2_saturated);
        self.push_sy(sy2_saturated);

        if dq {
            let p = self.dqb as i64 + self.dqa as i64 * h_divided_by_sz as i64;
            self.set_mac0_flags(p);
            self.mac[0] = p as i32;
            self.ir[0] = self.set_ir0_flags(p >> 12);
        }
    }

    fn set_ir0_flags(&mut self, value: i64) -> i16 {
        if value < 0 {
            self.flags |= 1 << 12;
            return 0;
        }
        if value > 0x1000 {
            self.flags |= 1 << 12;
            return 0x1000;
        }

        value as i16
    }

    fn set_mac0_flags(&mut self, value: i64) {
        if value < -0x8000_0000 {
            self.flags |= 1 << 15;
        } else if value > 0x7fff_ffff {
            self.flags |= 1 << 16;
        }
    }

    fn set_sn_flags(&mut self, value: i64, index: usize) -> i16 {
        if value < -0x400 {
            self.flags |= 1 << (14 - (index - 1));
            return -0x400;
        }

        if value > 0x3ff {
            self.flags |= 1 << (14 - (index - 1));
            return 0x3ff;
        }

        value as i16
    }

    fn set_ir_flags(&mut self, value: i32, index: usize, lm: bool) -> i16 {
        let flag_set = 1 << (24 - (index - 1));
        if lm && value < 0 {
            self.flags |= flag_set;
            return 0;
        } else if !lm && value < -0x8000 {
            self.flags |= flag_set;
            return -0x8000;
        }

        if value > 0x7fff {
            self.flags |= flag_set;

            return 0x7fff;
        }

        value as i16
    }

    fn set_ir_flag3(&mut self, previous: i64, value: i32) -> i16 {
        if previous < -0x8000 || previous > 0x7fff {
            self.flags |= 1 << 22;
        }

        if self.lm && value < 0 {
            return 0;
        }

        if !self.lm && value < -0x8000 {
            return -0x8000;
        }

        if value > 0x7fff {
            return 0x7fff;
        }

        value as i16
    }

    fn set_sz3_or_otz_flags(&mut self, value: i64) -> u16 {
        if value < 0 {
            self.flags |= 1 << 18;
            return 0;
        }

        if value > 0xffff {
            self.flags |= 1 << 18;
            return 0xffff;
        }

        value as u16
    }

    fn rtpt(&mut self) {
        self.rtp(0, false);
        self.rtp(1, false);
        self.rtp(2, true);
    }

    fn rtps(&mut self) {
        self.rtp(0, true);
    }

    fn push_sx(&mut self, sx: i16) {
        self.sxy_fifo[0].0 = self.sxy_fifo[1].0;
        self.sxy_fifo[1].0 = self.sxy_fifo[2].0;
        self.sxy_fifo[2].0 = sx;
    }

    fn push_sy(&mut self, sy: i16) {
        self.sxy_fifo[0].1 = self.sxy_fifo[1].1;
        self.sxy_fifo[1].1 = self.sxy_fifo[2].1;
        self.sxy_fifo[2].1 = sy;
    }

    pub fn push_sz(&mut self, sz: u16) {
        self.sz_fifo[0] = self.sz_fifo[1];
        self.sz_fifo[1] = self.sz_fifo[2];
        self.sz_fifo[2] = self.sz_fifo[3];
        self.sz_fifo[3] = sz;
    }

    fn to_u5(val: i16) -> u8 {
        if val > 0x1f {
            return 0x1f;
        }
        if val < 0 {
            return 0;
        }

        return val as u8;
    }

    pub fn read_data(&mut self, destination: usize) -> u32 {
        match destination {
            0 => (self.v[0].0 as u16 as u32) | (self.v[0].1 as u16 as u32) << 16,
            1 => self.v[0].2 as u32,
            2 => (self.v[1].0 as u16 as u32) | (self.v[1].1 as u16 as u32) << 16,
            3 => self.v[1].2 as u32,
            4 => (self.v[2].0 as u16 as u32) | (self.v[2].1 as u16 as u32) << 16,
            5 => self.v[2].2 as u32,
            6 => {
                (self.rgbc.r as u32) | (self.rgbc.g as u32) << 8 | (self.rgbc.b as u32) << 16 | (self.rgbc.c as u32) << 24
            }
            7 => self.otz as u32,
            8..=11 => self.ir[destination - 8] as u32,
            12..=14 => (self.sxy_fifo[destination - 12].0 as u16 as u32) | (self.sxy_fifo[destination - 12].1 as u16 as u32) << 16,
            15 => (self.sxy_fifo[2].0 as u16 as u32) | (self.sxy_fifo[2].1 as u16 as u32) << 16,
            16..=19 => self.sz_fifo[destination - 16] as u32,
            20..=22 => {
                (self.rgb_fifo[destination - 20].r as u32) | (self.rgb_fifo[destination - 20].g as u32) << 8 | (self.rgb_fifo[destination - 20].b as u32) << 16 | (self.rgb_fifo[destination - 20].c as u32) << 24
            }
            23 => self.res1,
            24..=27 => self.mac[destination - 24] as u32,
            28 | 29 => {
                let r = Gte::to_u5(self.ir[1] >> 7) as u32;
                let g = Gte::to_u5(self.ir[2] >> 7) as u32;
                let b = Gte::to_u5(self.ir[3] >> 7) as u32;

                r | (g << 5) | (b << 10)
            }
            30 => self.lzcs as u32,
            31 => self.lzcr as u32,
            _ => panic!("unsupported destination: {destination}")
        }
    }

    pub fn write_data(&mut self, destination: usize, value: u32) {
        match destination {
            0 => {
                self.v[0].0 = value as i16;
                self.v[0].1 = (value >> 16) as i16;
            }
            1 => self.v[0].2 = value as i16,
            2 => {
                self.v[1].0 = value as i16;
                self.v[1].1 = (value >> 16) as i16;
            }
            3 => self.v[1].2 = value as i16,
            4 => {
                self.v[2].0 = value as i16;
                self.v[2].1 = (value >> 16) as i16;
            }
            5 => self.v[2].2 = value as i16,
            6 => {
                self.rgbc.r = value as u8;
                self.rgbc.g = (value >> 8) as u8;
                self.rgbc.b = (value >> 16) as u8;
                self.rgbc.c = (value >> 24) as u8;
            }
            7 => self.otz = value as u16,
            8..=11 => self.ir[destination - 8] = value as i16,
            12..=14 => {
                self.sxy_fifo[destination - 12].0 = value as i16;
                self.sxy_fifo[destination - 12].1 = (value >> 16) as i16;
            }
            15 => {
                self.push_sx(value as i16);
                self.push_sy((value >> 16) as i16);
            }
            16..=19 => self.sz_fifo[destination - 16] = value as u16,
            20..=22 => {
                self.rgb_fifo[destination - 20].r = value as u8;
                self.rgb_fifo[destination - 20].g = (value >> 8) as u8;
                self.rgb_fifo[destination - 20].b = (value >> 16) as u8;
                self.rgb_fifo[destination - 20].c = (value >> 24) as u8;
            }
            23 => self.res1 = value,
            24..=27 => self.mac[destination - 24] = value as i32,
            28 => {
                self.ir[1] = ((value & 0x1f) << 7) as i16;
                self.ir[2] = (((value >> 5) & 0x1f) << 7) as i16;
                self.ir[3] = (((value >> 10) & 0x1f) << 7) as i16;
            }
            29 => (),
            30 => {
                self.lzcs = value as i32;
                self.lzcr = Self::get_num_leading_bits(self.lzcs);
            }
            31 => (),
            _ => panic!("unhandled destination received: {destination}")
        }
    }

    pub fn write_control(&mut self, destination: usize, value: u32) {

        match destination {
            0 => {
                self.rotation[0][0] = value as i16;
                self.rotation[0][1] = (value >> 16) as i16;
            }
            1 => {
                self.rotation[0][2] = value as i16;
                self.rotation[1][0] = (value >> 16) as i16;
            }
            2 => {
                self.rotation[1][1] = value as i16;
                self.rotation[1][2] = (value >> 16) as i16;
            }
            3 => {
                self.rotation[2][0] = value as i16;
                self.rotation[2][1] = (value >> 16) as i16;
            }
            4 => self.rotation[2][2] = value as i16,
            5 => self.tr.0 = value as i32,
            6 => self.tr.1 = value as i32,
            7 => self.tr.2 = value as i32,
            8 => {
                self.light[0][0] = value as i16;
                self.light[0][1] = (value >> 16) as i16;
            }
            9 => {
                self.light[0][2] = value as i16;
                self.light[1][0] = (value >> 16) as i16;
            }
            10 => {
                self.light[1][1] = value as i16;
                self.light[1][2] = (value >> 16) as i16;
            }
            11 => {
                self.light[2][0] = value as i16;
                self.light[2][1] = (value >> 16) as i16;
            }
            12 => self.light[2][2] = value as i16,
            13 => self.bk.0 = value as i32,
            14 => self.bk.1 = value as i32,
            15 => self.bk.2 = value as i32,
            16 => {
                self.color[0][0] = value as i16;
                self.color[0][1] = (value >> 16) as i16;
            }
            17 => {
                self.color[0][2] = value as i16;
                self.color[1][0] = (value >> 16) as i16;
            }
            18 => {
                self.color[1][1] = value as i16;
                self.color[1][2] = (value >> 16) as i16;
            }
            19 => {
                self.color[2][0] = value as i16;
                self.color[2][1] = (value >> 16) as i16;
            }
            20 => self.color[2][2] = value as i16,
            21 => self.fc.0 = value as i32,
            22 => self.fc.1 = value as i32,
            23 => self.fc.2 = value as i32,
            24 => self.ofx = value as i32,
            25 => self.ofy = value as i32,
            26 => self.h = value as u16,
            27 => self.dqa = value as i16,
            28 => self.dqb = value as i32,
            29 => self.zsf3 = value as i16,
            30 => self.zsf4 = value as i16,
            31 => {
                self.flags = value & 0x7fff_f000;

                if (value & 0x7f87e000) != 0 {
                    self.flags |= 1 << 31;
                }
            }
            _ => panic!("unhandled destination received: {destination}")
        }
    }

    fn get_num_leading_bits(num: i32) -> i32 {
        let leading_bit = ((num as u32) >> 31) & 0b1;

        let mut num_bits = 1;

        for i in 1..32 {
            let bit = ((num as u32) >> (31 - i)) & 0b1;

            if bit == leading_bit {
                num_bits += 1;
            } else {
                break;
            }
        }

        num_bits
    }

    pub fn read_control(&self, destination: usize) -> u32 {
        match destination {
            0 => {
                (self.rotation[0][0] as u16 as u32) | (self.rotation[0][1] as u16 as u32) << 16
            }
            1 => {
                (self.rotation[0][2] as u16 as u32) | (self.rotation[1][0] as u16 as u32) << 16
            }
            2 => {
                (self.rotation[1][1] as u16 as u32) | (self.rotation[1][2] as u16 as u32) << 16
            }
            3 => {
                (self.rotation[2][0] as u16 as u32) | (self.rotation[2][1] as u16 as u32) << 16
            }
            4 => self.rotation[2][2] as u32,
            5 => self.tr.0 as u32,
            6 => self.tr.1 as u32,
            7 => self.tr.2 as u32,
            8 => {
                (self.light[0][0] as u16 as u32) | (self.light[0][1] as u16 as u32) << 16
            }
            9 => {
                (self.light[0][2] as u16 as u32) | (self.light[1][0] as u16 as u32) << 16
            }
            10 => {
                (self.light[1][1] as u16 as u32) | (self.light[1][2] as u16 as u32) << 16
            }
            11 => {
                (self.light[2][0] as u16 as u32) | (self.light[2][1] as u16 as u32) << 16
            }
            12 => self.light[2][2] as u32,
            13 => self.bk.0 as u32,
            14 => self.bk.1 as u32,
            15 => self.bk.2 as u32,
            16 => {
                (self.color[0][0] as u16 as u32) | (self.color[0][1] as u16 as u32) << 16
            }
            17 => {
                (self.color[0][2] as u16 as u32) | (self.color[1][0] as u16 as u32) << 16
            }
            18 => {
                (self.color[1][1] as u16 as u32) | (self.color[1][2] as u16 as u32) << 16
            }
            19 => {
                (self.color[2][0] as u16 as u32) | (self.color[2][1] as u16 as u32) << 16
            }
            20 => self.color[2][2] as u32,
            21 => self.fc.0 as u32,
            22 => self.fc.1 as u32,
            23 => self.fc.2 as u32,
            24 => self.ofx as u32,
            25 => self.ofy as u32,
            26 => self.h as i16 as u32,
            27 => self.dqa as u32,
            28 => self.dqb as u32,
            29 => self.zsf3 as u32,
            30 => self.zsf4 as u32,
            31 => self.flags,
            _ => unreachable!("can't happen")
        }
    }
}