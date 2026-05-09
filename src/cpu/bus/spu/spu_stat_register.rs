use bitflags::bitflags;
use serde::{Deserialize, Serialize};

bitflags! {
    #[derive(Serialize, Deserialize)]
    pub struct SpuStatRegister: u16 {
        const IRQ9_FLAG = 1 << 6;
        const DMA_REQUEST_BIT = 1 << 7;
        const DMA_WRITE_REQUEST = 1 << 8;
        const DMA_READ_REQUEST = 1 << 9;
        const DMA_TRANSFER_BUSY = 1 << 10;
        const CAPTURE_BUFFER_ID = 1 << 11;
    }
}
