use bitflags::bitflags;
/*
  0     IRQ0 VBLANK (PAL=50Hz, NTSC=60Hz)
  1     IRQ1 GPU   Can be requested via GP0(1Fh) command (rarely used)
  2     IRQ2 CDROM
  3     IRQ3 DMA
  4     IRQ4 TMR0  Timer 0 aka Root Counter 0 (Sysclk or Dotclk)
  5     IRQ5 TMR1  Timer 1 aka Root Counter 1 (Sysclk or H-blank)
  6     IRQ6 TMR2  Timer 2 aka Root Counter 2 (Sysclk or Sysclk/8)
  7     IRQ7 Controller and Memory Card - Byte Received Interrupt
  8     IRQ8 SIO
  9     IRQ9 SPU
  10    IRQ10 Controller - Lightpen Interrupt. Also shared by PIO and DTL cards.
*/
bitflags! {
    pub struct InterruptRegister: u32 {
        const VBLANK = 1 << 0;
        const GPU = 1 << 1;
        const CDROM = 1 << 2;
        const DMA = 1 << 3;
        const TMR0 = 1 << 4;
        const TMR1 = 1 << 5;
        const TMR2 = 1 << 6;
        const PERIPHERAL = 1 << 7;
        const SIO = 1 << 8;
        const SPU = 1 << 9;
        const LIGHTPEN = 1 << 10;
    }
}