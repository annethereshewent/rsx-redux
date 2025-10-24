use std::cmp;

use super::{SPU, SoundRam};

pub struct Reverb {
    pub mbase: u32,
    dapf1: u32,
    dapf2: u32,
    viir: i16,
    vcomb1: i16,
    vcomb2: i16,
    vcomb3: i16,
    vcomb4: i16,
    vwall: i16,
    vapf1: i16,
    vapf2: i16,
    vlin: i16,
    vrin: i16,
    mlsame: u32,
    mrsame: u32,
    mldiff: u32,
    mrdiff: u32,
    mlcomb1: u32,
    mlcomb2: u32,
    mlcomb3: u32,
    mlcomb4: u32,
    mrcomb1: u32,
    mrcomb2: u32,
    mrcomb3: u32,
    mrcomb4: u32,
    dldiff: u32,
    drdiff: u32,
    dlsame: u32,
    drsame: u32,
    mlapf1: u32,
    mlapf2: u32,
    mrapf1: u32,
    mrapf2: u32,
    calculate_left: bool,
    pub left_out: f32,
    pub right_out: f32,
    pub buffer_address: u32,
}

impl Reverb {
    pub fn new() -> Self {
        Self {
            mbase: 0,
            dapf1: 0,
            dapf2: 0,
            viir: 0,
            vcomb1: 0,
            vcomb2: 0,
            vcomb3: 0,
            vcomb4: 0,
            vwall: 0,
            vapf1: 0,
            vapf2: 0,
            vlin: 0,
            vrin: 0,
            mlsame: 0,
            mrsame: 0,
            mlcomb1: 0,
            mlcomb2: 0,
            mlcomb3: 0,
            mlcomb4: 0,
            mrcomb1: 0,
            mrcomb2: 0,
            mrcomb3: 0,
            mrcomb4: 0,
            dldiff: 0,
            drdiff: 0,
            dlsame: 0,
            drsame: 0,
            mlapf1: 0,
            mlapf2: 0,
            mrapf1: 0,
            mrapf2: 0,
            mldiff: 0,
            mrdiff: 0,
            left_out: 0.0,
            right_out: 0.0,
            calculate_left: false,
            buffer_address: 0,
        }
    }

    /* per https://psx-spx.consoledev.net/soundprocessingunitspu/#spu-reverb-formula
    * ___Input from Mixer (Input volume multiplied with incoming data)_____________
      Lin = vLIN * LeftInput    ;from any channels that have Reverb enabled
      Rin = vRIN * RightInput   ;from any channels that have Reverb enabled
      ____Same Side Reflection (left-to-left and right-to-right)___________________
      [mLSAME] = (Lin + [dLSAME]*vWALL - [mLSAME-2])*vIIR + [mLSAME-2]  ;L-to-L
      [mRSAME] = (Rin + [dRSAME]*vWALL - [mRSAME-2])*vIIR + [mRSAME-2]  ;R-to-R
      [mLDIFF] = (Lin + [dRDIFF]*vWALL - [mLDIFF-2])*vIIR + [mLDIFF-2]  ;R-to-L
      [mRDIFF] = (Rin + [dLDIFF]*vWALL - [mRDIFF-2])*vIIR + [mRDIFF-2]  ;L-to-R
      ___Early Echo (Comb Filter, with input from buffer)__________________________
      Lout=vCOMB1*[mLCOMB1]+vCOMB2*[mLCOMB2]+vCOMB3*[mLCOMB3]+vCOMB4*[mLCOMB4]
      Rout=vCOMB1*[mRCOMB1]+vCOMB2*[mRCOMB2]+vCOMB3*[mRCOMB3]+vCOMB4*[mRCOMB4]
      ___Late Reverb APF1 (All Pass Filter 1, with input from COMB)________________
      Lout=Lout-vAPF1*[mLAPF1-dAPF1], [mLAPF1]=Lout, Lout=Lout*vAPF1+[mLAPF1-dAPF1]
      Rout=Rout-vAPF1*[mRAPF1-dAPF1], [mRAPF1]=Rout, Rout=Rout*vAPF1+[mRAPF1-dAPF1]
      ___Late Reverb APF2 (All Pass Filter 2, with input from APF1)________________
      Lout=Lout-vAPF2*[mLAPF2-dAPF2], [mLAPF2]=Lout, Lout=Lout*vAPF2+[mLAPF2-dAPF2]
      Rout=Rout-vAPF2*[mRAPF2-dAPF2], [mRAPF2]=Rout, Rout=Rout*vAPF2+[mRAPF2-dAPF2]
      ___Output to Mixer (Output volume multiplied with input from APF2)___________
      LeftOutput  = Lout*vLOUT
      RightOutput = Rout*vROUT
      ___Finally, before repeating the above steps_________________________________
      BufferAddress = MAX(mBASE, (BufferAddress+2) AND 7FFFEh)
    */
    pub fn calculate_reverb(&mut self, input: [f32; 2], ram: &mut SoundRam) {
        self.calculate_left = !self.calculate_left;

        // per no$psx specs, reverb spends one cycle calculating left output, and the right on the next.
        if self.calculate_left {
            self.calculate_left_reverb(ram, input[0]);
        } else {
            self.calculate_right_reverb(ram, input[1]);

            // once we have finished calculating both lin and rout, we can update the buffer address to the next address.
            self.buffer_address = cmp::max((self.buffer_address + 2) & 0x7fffe, self.mbase);
        }
    }

    fn calculate_right_reverb(&mut self, ram: &mut SoundRam, right_input: f32) {
        let rin = SPU::to_f32(self.vrin) * right_input;

        let temp = self.get_from_ram(ram, self.mrsame - 2);
        let mrsame = rin + self.get_from_ram(ram, self.drsame) * SPU::to_f32(self.vwall)
            - temp * SPU::to_f32(self.viir)
            + temp;
        self.write_to_ram(ram, self.mrsame, mrsame);

        let temp = self.get_from_ram(ram, self.mrdiff - 2);
        let mrdiff = rin + self.get_from_ram(ram, self.dldiff) * SPU::to_f32(self.vwall)
            - temp * SPU::to_f32(self.viir)
            + temp;
        self.write_to_ram(ram, self.mrdiff, mrdiff);

        let mut rout = SPU::to_f32(self.vcomb1) * self.get_from_ram(ram, self.mrcomb1);
        rout += SPU::to_f32(self.vcomb2) * self.get_from_ram(ram, self.mrcomb2);
        rout += SPU::to_f32(self.vcomb3) * self.get_from_ram(ram, self.mrcomb3);
        rout += SPU::to_f32(self.vcomb4) * self.get_from_ram(ram, self.mrcomb4);

        rout -= SPU::to_f32(self.vapf1) * self.get_from_ram(ram, self.mrapf1 - self.dapf1);
        self.write_to_ram(ram, self.mrapf1, rout);
        rout = rout * SPU::to_f32(self.vapf1) + self.get_from_ram(ram, self.mrapf1 - self.dapf1);

        rout -= SPU::to_f32(self.vapf2) * self.get_from_ram(ram, self.mrapf2 - self.dapf2);
        self.write_to_ram(ram, self.mrapf2, rout);
        rout = rout * SPU::to_f32(self.vapf2) + self.get_from_ram(ram, self.mrapf2 - self.dapf2);

        self.right_out = rout;
    }
    fn calculate_left_reverb(&mut self, ram: &mut SoundRam, left_input: f32) {
        let lin = SPU::to_f32(self.vlin) * left_input;

        let temp = self.get_from_ram(ram, self.mlsame - 2);
        let mlsame = lin + self.get_from_ram(ram, self.dlsame) * SPU::to_f32(self.vwall)
            - temp * SPU::to_f32(self.viir)
            + temp;
        self.write_to_ram(ram, self.mlsame, mlsame);

        let temp = self.get_from_ram(ram, self.mldiff - 2);
        let mldiff = lin + self.get_from_ram(ram, self.drdiff) * SPU::to_f32(self.vwall)
            - temp * SPU::to_f32(self.viir)
            + temp;
        self.write_to_ram(ram, self.mldiff, mldiff);

        let mut lout = SPU::to_f32(self.vcomb1) * self.get_from_ram(ram, self.mlcomb1);
        lout += SPU::to_f32(self.vcomb2) * self.get_from_ram(ram, self.mlcomb2);
        lout += SPU::to_f32(self.vcomb3) * self.get_from_ram(ram, self.mlcomb3);
        lout += SPU::to_f32(self.vcomb4) * self.get_from_ram(ram, self.mlcomb4);

        lout -= SPU::to_f32(self.vapf1) * self.get_from_ram(ram, self.mlapf1 - self.dapf1);
        self.write_to_ram(ram, self.mlapf1, lout);
        lout = lout * SPU::to_f32(self.vapf1) + self.get_from_ram(ram, self.mlapf1 - self.dapf1);

        lout -= SPU::to_f32(self.vapf2) * self.get_from_ram(ram, self.mlapf2 - self.dapf2);
        self.write_to_ram(ram, self.mlapf2, lout);
        lout = lout * SPU::to_f32(self.vapf2) + self.get_from_ram(ram, self.mlapf2 - self.dapf2);

        self.left_out = lout;
    }

    fn get_from_ram(&self, ram: &mut SoundRam, address: u32) -> f32 {
        let value = ram.read_16(self.calculate_address(address)) as i16;
        SPU::to_f32(value)
    }

    fn write_to_ram(&self, ram: &mut SoundRam, address: u32, val: f32) {
        let address = self.calculate_address(address);
        let result = SPU::to_i16(val) as u16;

        ram.write_16(address, result);
    }

    fn calculate_address(&self, address: u32) -> u32 {
        let mut offset = self.buffer_address + address - self.mbase;
        offset %= 0x80000 - self.mbase;

        (self.mbase + offset) & 0x7fffe
    }

    pub fn write16(&mut self, address: usize, val: u16) {
        match address {
            0x1f80_1dc0 => self.dapf1 = (val as u32) * 8,
            0x1f80_1dc2 => self.dapf2 = (val as u32) * 8,
            0x1f80_1dc4 => self.viir = val as i16,
            0x1f80_1dc6 => self.vcomb1 = val as i16,
            0x1f80_1dc8 => self.vcomb2 = val as i16,
            0x1f80_1dca => self.vcomb3 = val as i16,
            0x1f80_1dcc => self.vcomb4 = val as i16,
            0x1f80_1dce => self.vwall = val as i16,
            0x1f80_1dd0 => self.vapf1 = val as i16,
            0x1f80_1dd2 => self.vapf2 = val as i16,
            0x1f80_1dd4 => self.mlsame = (val as u32) * 8,
            0x1f80_1dd6 => self.mrsame = (val as u32) * 8,
            0x1f80_1dd8 => self.mlcomb1 = (val as u32) * 8,
            0x1f80_1dda => self.mrcomb1 = (val as u32) * 8,
            0x1f80_1ddc => self.mlcomb2 = (val as u32) * 8,
            0x1f80_1dde => self.mrcomb2 = (val as u32) * 8,
            0x1f80_1de0 => self.dlsame = (val as u32) * 8,
            0x1f80_1de2 => self.drsame = (val as u32) * 8,
            0x1f80_1de4 => self.mldiff = (val as u32) * 8,
            0x1f80_1de6 => self.mrdiff = (val as u32) * 8,
            0x1f80_1de8 => self.mlcomb3 = (val as u32) * 8,
            0x1f80_1dea => self.mrcomb3 = (val as u32) * 8,
            0x1f80_1dec => self.mlcomb4 = (val as u32) * 8,
            0x1f80_1dee => self.mrcomb4 = (val as u32) * 8,
            0x1f80_1df0 => self.dldiff = (val as u32) * 8,
            0x1f80_1df2 => self.drdiff = (val as u32) * 8,
            0x1f80_1df4 => self.mlapf1 = (val as u32) * 8,
            0x1f80_1df6 => self.mrapf1 = (val as u32) * 8,
            0x1f80_1df8 => self.mlapf2 = (val as u32) * 8,
            0x1f80_1dfa => self.mrapf2 = (val as u32) * 8,
            0x1f80_1dfc => self.vlin = val as i16,
            0x1f80_1dfe => self.vrin = val as i16,
            _ => panic!("write to unhandled SPU address: {:X}", address),
        }
    }

    pub fn write_mbase(&mut self, val: u16) {
        self.mbase = (val as u32) * 8;
        self.buffer_address = self.mbase;
    }
}
