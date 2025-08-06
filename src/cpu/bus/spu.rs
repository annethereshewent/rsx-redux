pub struct SPU {
    pub main_volume_left: u16,
    pub main_volume_right: u16,
    pub reverb_volume_left: u16,
    pub reverb_volume_right: u16
}

impl SPU {
    pub fn new() -> Self {
        Self {
            main_volume_left: 0,
            main_volume_right: 0,
            reverb_volume_left: 0,
            reverb_volume_right: 0
        }
    }
}