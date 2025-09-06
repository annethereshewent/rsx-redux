use std::cmp;

use crate::cpu::bus::spu::{SoundRam, SPU};

pub struct Reverb {
    m_base: u32,
    d_apf1: u32,
    d_apf2: u32,
    v_iir: i16,
    v_comb1: i16,
    v_comb2: i16,
    v_comb3: i16,
    v_comb4: i16,
    v_wall: i16,
    v_apf1: i16,
    v_apf2: i16,
    ml_same: u32,
    mr_same: u32,
    m_l_comb1: u32,
    m_r_comb1: u32,
    m_l_comb2: u32,
    m_r_comb2: u32,
    d_l_same: u32,
    d_r_same: u32,
    m_l_diff: u32,
    m_r_diff: u32,
    m_l_comb3: u32,
    m_r_comb3: u32,
    m_l_comb4: u32,
    m_r_comb4: u32,
    d_l_diff: u32,
    d_r_diff: u32,
    m_lapf1: u32,
    m_rapf1: u32,
    m_lapf2: u32,
    m_rapf2: u32,
    v_lin: i16,
    v_rin: i16,
    v_l_out: i16,
    v_r_out: i16,
    buffer_address: u32,
    pub reverb_out_left: f32,
    pub reverb_out_right: f32,
    pub is_left: bool
}

impl Reverb {
    pub fn new() -> Self {
        Self {
            m_base: 0,
            d_apf1: 0,
            d_apf2: 0,
            v_iir: 0,
            v_comb1: 0,
            v_comb2: 0,
            v_comb3: 0,
            v_comb4: 0,
            v_wall: 0,
            v_apf1: 0,
            v_apf2: 0,
            ml_same: 0,
            mr_same: 0,
            m_l_comb1: 0,
            m_r_comb1: 0,
            m_l_comb2: 0,
            m_r_comb2: 0,
            d_l_same: 0,
            d_r_same: 0,
            m_l_diff: 0,
            m_r_diff: 0,
            m_l_comb3: 0,
            m_r_comb3: 0,
            m_l_comb4: 0,
            m_r_comb4: 0,
            d_l_diff: 0,
            d_r_diff: 0,
            m_lapf1: 0,
            m_rapf1: 0,
            m_lapf2: 0,
            m_rapf2: 0,
            v_lin: 0,
            v_rin: 0,
            v_l_out: 0,
            v_r_out: 0,
            buffer_address: 0,
            reverb_out_left: 0.0,
            reverb_out_right: 0.0,
            is_left: true
        }
    }
    /*
    ___Input from Mixer (Input volume multiplied with incoming data)_____________
    Lin = vLIN * LeftInput    ;from any channels that have Reverb enabled
    Rin = vRIN * RightInput   ;from any channels that have Reverb enabled
    ____Same Side Reflection (left-to-left and right-to-right)___________________
    [mLSAME] = (Lin + [dLSAME]*vWALL - [mLSAME-2])*vIIR + [mLSAME-2]  ;L-to-L
    [mRSAME] = (Rin + [dRSAME]*vWALL - [mRSAME-2])*vIIR + [mRSAME-2]  ;R-to-R
    ___Different Side Reflection (left-to-right and right-to-left)_______________
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
    Wait one 22050Hz cycle, then repeat the above stuff
    */
    pub fn calculate_right(&mut self, reverb_right: i16, sound_ram: &mut SoundRam) {
        let rin = SPU::apply_volume(
            SPU::to_f32(reverb_right),
            self.v_rin
        );

        let d_r_same = sound_ram.readf32(self.calculate_address(self.d_r_same as usize));
        let mr_same2 = sound_ram.readf32(self.calculate_address(self.mr_same as usize - 2));

        let mr_same_val =
            rin + SPU::apply_volume(d_r_same, self.v_wall) -
            SPU::apply_volume(mr_same2, self.v_iir) + mr_same2;

        sound_ram.writef32(
            self.calculate_address(self.mr_same as usize),
            mr_same_val
        );

        let dl_diff = sound_ram.readf32(self.calculate_address(self.d_l_diff as usize));
        let mr_diff2 = sound_ram.readf32(self.calculate_address(self.m_r_diff as usize - 2));

        let dl_diff_volume = SPU::apply_volume(dl_diff, self.v_wall);

        let mr_diff_val = SPU::apply_volume(
            rin + dl_diff_volume - mr_diff2,
            self.v_iir
        ) + mr_diff2;

        sound_ram.writef32(self.calculate_address(self.m_r_diff as usize), mr_diff_val);

        let mr_comb1 = sound_ram.readf32(self.calculate_address(self.m_r_comb1 as usize));
        let mr_comb2 = sound_ram.readf32(self.calculate_address(self.m_r_comb2 as usize));
        let mr_comb3 = sound_ram.readf32(self.calculate_address(self.m_r_comb3 as usize));
        let mr_comb4 = sound_ram.readf32(self.calculate_address(self.m_r_comb4 as usize));

        let mut rout = SPU::apply_volume(mr_comb1, self.v_comb1) +
            SPU::apply_volume(mr_comb2, self.v_comb2) +
            SPU::apply_volume(mr_comb3, self.v_comb3) +
            SPU::apply_volume(mr_comb4, self.v_comb4);

        let rapf1 = sound_ram.readf32(self.calculate_address(self.m_rapf1 as usize - self.d_apf1 as usize));

        rout = rout - SPU::apply_volume(rapf1, self.v_apf1);

        sound_ram.writef32(self.calculate_address(self.m_rapf1 as usize), rout);

        rout = SPU::apply_volume(rout, self.v_apf1) + rapf1;

        let rapf2 = sound_ram.readf32(self.calculate_address(self.m_rapf2 as usize - self.d_apf2 as usize));

        rout = rout - SPU::apply_volume(rapf2, self.v_apf2);

        sound_ram.writef32(self.calculate_address(self.m_rapf2 as usize), rout);

        rout = SPU::apply_volume(rout, self.v_apf2) + rapf2;

        let right_output = SPU::apply_volume(rout, self.v_r_out);

        self.reverb_out_right = right_output;

        self.buffer_address = cmp::max(self.m_base, (self.buffer_address + 2) & 0x7_fffe);

    }

    pub fn calculate_left(&mut self, reverb_left: i16, sound_ram: &mut SoundRam) {
        let lin = SPU::apply_volume(
            SPU::to_f32(reverb_left),
            self.v_lin
        );

        let d_l_same = sound_ram.readf32(self.calculate_address(self.d_l_same as usize));
        let ml_same2 = sound_ram.readf32(self.calculate_address(self.ml_same as usize - 2));

        let ml_same_val =
            lin + SPU::apply_volume(d_l_same, self.v_wall) -
            SPU::apply_volume(ml_same2, self.v_iir) + ml_same2;

        sound_ram.writef32(
            self.calculate_address(self.ml_same as usize),
            ml_same_val
        );

        let dr_diff = sound_ram.readf32(self.calculate_address(self.d_r_diff as usize));
        let ml_diff2 = sound_ram.readf32(self.calculate_address(self.m_l_diff as usize - 2));

        let dr_diff_volume = SPU::apply_volume(dr_diff, self.v_wall);

        let ml_diff_val = SPU::apply_volume(
            lin + dr_diff_volume - ml_diff2,
            self.v_iir
        ) + ml_diff2;

        sound_ram.writef32(self.calculate_address(self.m_l_diff as usize), ml_diff_val);

        let ml_comb1 = sound_ram.readf32(self.calculate_address(self.m_l_comb1 as usize));
        let ml_comb2 = sound_ram.readf32(self.calculate_address(self.m_l_comb2 as usize));
        let ml_comb3 = sound_ram.readf32(self.calculate_address(self.m_l_comb3 as usize));
        let ml_comb4 = sound_ram.readf32(self.calculate_address(self.m_l_comb4 as usize));

        let mut lout = SPU::apply_volume(ml_comb1, self.v_comb1) +
            SPU::apply_volume(ml_comb2, self.v_comb2) +
            SPU::apply_volume(ml_comb3, self.v_comb3) +
            SPU::apply_volume(ml_comb4, self.v_comb4);

        let lapf1 = sound_ram.readf32(self.calculate_address(self.m_lapf1 as usize - self.d_apf1 as usize));

        lout = lout - SPU::apply_volume(lapf1, self.v_apf1);

        sound_ram.writef32(self.calculate_address(self.m_lapf1 as usize), lout);

        lout = SPU::apply_volume(lout, self.v_apf1) + lapf1;

        let lapf2 = sound_ram.readf32(self.calculate_address(self.m_lapf2 as usize - self.d_apf2 as usize));

        lout = lout - SPU::apply_volume(lapf2, self.v_apf2);

        sound_ram.writef32(self.calculate_address(self.m_lapf2 as usize), lout);

        lout = SPU::apply_volume(lout, self.v_apf2) + lapf2;

        let left_output = SPU::apply_volume(lout, self.v_l_out);

        self.reverb_out_left = left_output;
    }

    /*
    1f801DA2h spu   mBASE   base    Reverb Work Area Start Address in Sound RAM
    1f801DC0h rev00 dAPF1   disp    Reverb APF Offset 1
    1f801DC2h rev01 dAPF2   disp    Reverb APF Offset 2
    1f801DC4h rev02 vIIR    volume  Reverb Reflection Volume 1
    1f801DC6h rev03 vCOMB1  volume  Reverb Comb Volume 1
    1f801DC8h rev04 vCOMB2  volume  Reverb Comb Volume 2
    1f801DCAh rev05 vCOMB3  volume  Reverb Comb Volume 3
    1f801DCCh rev06 vCOMB4  volume  Reverb Comb Volume 4
    1f801DCEh rev07 vWALL   volume  Reverb Reflection Volume 2
    1f801DD0h rev08 vAPF1   volume  Reverb APF Volume 1
    1f801DD2h rev09 vAPF2   volume  Reverb APF Volume 2
    1f801DD4h rev0A mLSAME  src/dst Reverb Same Side Reflection Address 1 Left
    1f801DD6h rev0B mRSAME  src/dst Reverb Same Side Reflection Address 1 Right
    1f801DD8h rev0C mLCOMB1 src     Reverb Comb Address 1 Left
    1f801DDAh rev0D mRCOMB1 src     Reverb Comb Address 1 Right
    1f801DDCh rev0E mLCOMB2 src     Reverb Comb Address 2 Left
    1f801DDEh rev0F mRCOMB2 src     Reverb Comb Address 2 Right
    1f801DE0h rev10 dLSAME  src     Reverb Same Side Reflection Address 2 Left
    1f801DE2h rev11 dRSAME  src     Reverb Same Side Reflection Address 2 Right
    1f801DE4h rev12 mLDIFF  src/dst Reverb Different Side Reflect Address 1 Left
    1f801DE6h rev13 mRDIFF  src/dst Reverb Different Side Reflect Address 1 Right
    1f801DE8h rev14 mLCOMB3 src     Reverb Comb Address 3 Left
    1f801DEAh rev15 mRCOMB3 src     Reverb Comb Address 3 Right
    1f801DECh rev16 mLCOMB4 src     Reverb Comb Address 4 Left
    1f801DEEh rev17 mRCOMB4 src     Reverb Comb Address 4 Right
    1f801DF0h rev18 dLDIFF  src     Reverb Different Side Reflect Address 2 Left
    1f801DF2h rev19 dRDIFF  src     Reverb Different Side Reflect Address 2 Right
    1f801DF4h rev1A mLAPF1  src/dst Reverb APF Address 1 Left
    1f801DF6h rev1B mRAPF1  src/dst Reverb APF Address 1 Right
    1f801DF8h rev1C mLAPF2  src/dst Reverb APF Address 2 Left
    1f801DFAh rev1D mRAPF2  src/dst Reverb APF Address 2 Right
    1f801DFCh rev1E vLIN    volume  Reverb Input Volume Left
    1f801DFEh rev1F vRIN    volume  Reverb Input Volume Right
    */
    pub fn write16(&mut self, address: usize, value: u16) {
        match address {
            0x1f801d84 => self.v_l_out = value as i16,
            0x1f801d86 => self.v_r_out = value as i16,
            0x1f801da2 => {
                self.m_base = value as u32 * 8;
                self.buffer_address = self.m_base;
            }
            0x1f801dc0 => self.d_apf1 = value as u32 * 8,
            0x1f801dc2 => self.d_apf2 = value as u32 * 8,
            0x1f801dc4 => self.v_iir = value as i16,
            0x1f801dc6 => self.v_comb1 = value as i16,
            0x1f801dc8 => self.v_comb2 = value as i16,
            0x1f801dca => self.v_comb3 = value as i16,
            0x1f801dcc => self.v_comb4 = value as i16,
            0x1f801dce => self.v_wall = value as i16,
            0x1f801dd0 => self.v_apf1 = value as i16,
            0x1f801dd2 => self.v_apf2 = value as i16,
            0x1f801dd4 => self.ml_same = value as u32 * 8,
            0x1f801dd6 => self.mr_same = value as u32 * 8,
            0x1f801dd8 => self.m_l_comb1 = value as u32 * 8,
            0x1f801dda => self.m_r_comb1 = value as u32 * 8,
            0x1f801ddc => self.m_l_comb2 = value as u32 * 8,
            0x1f801dde => self.m_r_comb2 = value as u32 * 8,
            0x1f801de0 => self.d_l_same = value as u32 * 8,
            0x1f801de2 => self.d_r_same = value as u32 * 8,
            0x1f801de4 => self.m_l_diff = value as u32 * 8,
            0x1f801de6 => self.m_r_diff = value as u32 * 8,
            0x1f801de8 => self.m_l_comb3 = value as u32 * 8,
            0x1f801dea => self.m_r_comb3 = value as u32 * 8,
            0x1f801dec => self.m_l_comb4 = value as u32 * 8,
            0x1f801dee => self.m_r_comb4 = value as u32 * 8,
            0x1f801df0 => self.d_l_diff = value as u32 * 8,
            0x1f801df2 => self.d_r_diff = value as u32 * 8,
            0x1f801df4 => self.m_lapf1 = value as u32 * 8,
            0x1f801df6 => self.m_rapf1 = value as u32 * 8,
            0x1f801df8 => self.m_lapf2 = value as u32 * 8,
            0x1f801dfa => self.m_rapf2 = value as u32 * 8,
            0x1f801dfc => self.v_lin = value as i16,
            0x1f801dfe => self.v_rin = value as i16,
            _ => panic!("invalid address given to reverb: 0x{:x}", address)
        }
    }

    fn calculate_address(&self, offset: usize) -> usize {
        let address = (self.buffer_address as usize + offset) & 0x7_fffe;

        cmp::max(self.m_base as usize, address)
    }
}