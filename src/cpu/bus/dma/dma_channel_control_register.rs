use bitflags::bitflags;

#[derive(Copy, Clone)]
pub enum SyncMode {
    Burst = 0,
    Slice = 1,
    LinkedList = 2
}

bitflags! {
    #[derive(Copy, Clone)]
    pub struct DmaChannelControlRegister: u32 {
        const TRANSFER_DIR = 1;
        const INCREMENT = 1 << 1;
        const MODE = 1 << 8;
        const START_TRANSFER = 1 << 24;
        const FORCE_TRANSFER = 1 << 28;
        const PAUSE_FORCED = 1 << 29;
        const BUS_SNOOPING = 1 << 30; // not used?
    }
}

impl DmaChannelControlRegister {
    pub fn sync_mode(&self) -> SyncMode {
        match (self.bits() >> 9) & 0x3 {
            0 => SyncMode::Burst,
            1 => SyncMode::Slice,
            2 => SyncMode::LinkedList,
            _ => panic!("reserved mode: {}", (self.bits() >> 9) & 0x3)
        }
    }

    pub fn chopping_dma_window_size(&self) -> u32 {
        (self.bits() >> 16) & 0x7
    }

    pub fn chopping_cpu_window_size(&self) -> u32 {
        (self.bits() >> 20) & 0x7
    }
}