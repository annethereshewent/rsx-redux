use bitflags::bitflags;
/*
  0-2   DMA0, MDECin  Priority      (0..7; 0=Highest, 7=Lowest)
  3     DMA0, MDECin  Master Enable (0=Disable, 1=Enable)
  4-6   DMA1, MDECout Priority      (0..7; 0=Highest, 7=Lowest)
  7     DMA1, MDECout Master Enable (0=Disable, 1=Enable)
  8-10  DMA2, GPU     Priority      (0..7; 0=Highest, 7=Lowest)
  11    DMA2, GPU     Master Enable (0=Disable, 1=Enable)
  12-14 DMA3, CDROM   Priority      (0..7; 0=Highest, 7=Lowest)
  15    DMA3, CDROM   Master Enable (0=Disable, 1=Enable)
  16-18 DMA4, SPU     Priority      (0..7; 0=Highest, 7=Lowest)
  19    DMA4, SPU     Master Enable (0=Disable, 1=Enable)
  20-22 DMA5, PIO     Priority      (0..7; 0=Highest, 7=Lowest)
  23    DMA5, PIO     Master Enable (0=Disable, 1=Enable)
  24-26 DMA6, OTC     Priority      (0..7; 0=Highest, 7=Lowest)
  27    DMA6, OTC     Master Enable (0=Disable, 1=Enable)
  28-30 CPU memory access priority  (0..7; 0=Highest, 7=Lowest)
  31    No effect, should be CPU memory access enable (R/W)
*/
bitflags! {
    pub struct DmaControlRegister: u32 {
        const DMA0_ENABLE = 1 << 3;
        const DMA1_ENABLE = 1 << 7;
        const DMA2_ENABLE = 1 << 11;
        const DMA3_ENABLE = 1 << 15;
        const DMA4_ENABLE = 1 << 19;
        const DMA5_ENABLE = 1 << 23;
        const DMA6_ENABLE = 1 << 27;
    }
}

impl DmaControlRegister {
    pub fn dma0_priority(&self) -> u32 {
        self.bits() & 0x3
    }

    pub fn dma1_priority(&self) -> u32 {
        (self.bits() >> 4) & 0x3
    }

    pub fn dma2_priority(&self) -> u32 {
        (self.bits() >> 8) & 0x3
    }

    pub fn dma3_priority(&self) -> u32 {
        (self.bits() >> 12) & 0x3
    }

    pub fn dma4_priority(&self) -> u32 {
        (self.bits() >> 12) & 0x3
    }

    pub fn dma5_priority(&self) -> u32 {
        (self.bits() >> 12) & 0x3
    }

    pub fn dma6_priority(&self) -> u32 {
        (self.bits() >> 12) & 0x3
    }
}