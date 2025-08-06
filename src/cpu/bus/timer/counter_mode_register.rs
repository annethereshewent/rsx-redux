use bitflags::bitflags;

/*
  0     Synchronization Enable (0=Free Run, 1=Synchronize via Bit1-2)
  1-2   Synchronization Mode   (0-3, see lists below)
         Synchronization Modes for Counter 0:
           0 = Pause counter during Hblank(s)
           1 = Reset counter to 0000h at Hblank(s)
           2 = Reset counter to 0000h at Hblank(s) and pause outside of Hblank
           3 = Pause until Hblank occurs once, then switch to Free Run
         Synchronization Modes for Counter 1:
           Same as above, but using Vblank instead of Hblank
         Synchronization Modes for Counter 2:
           0 or 3 = Stop counter at current value (forever, no h/v-blank start)
           1 or 2 = Free Run (same as when Synchronization Disabled)
  3     Reset counter to 0000h  (0=After Counter=FFFFh, 1=After Counter=Target)
  4     IRQ when Counter=Target (0=Disable, 1=Enable)
  5     IRQ when Counter=FFFFh  (0=Disable, 1=Enable)
  6     IRQ Once/Repeat Mode    (0=One-shot, 1=Repeatedly)
  7     IRQ Pulse/Toggle Mode   (0=Short Bit10=0 Pulse, 1=Toggle Bit10 on/off)
  8-9   Clock Source (0-3, see list below)
         Counter 0:  0 or 2 = System Clock,  1 or 3 = Dotclock
         Counter 1:  0 or 2 = System Clock,  1 or 3 = Hblank
         Counter 2:  0 or 1 = System Clock,  2 or 3 = System Clock/8
  10    Interrupt Request       (0=Yes, 1=No) (Set after Writing)    (W=1) (R)
  11    Reached Target Value    (0=No, 1=Yes) (Reset after Reading)        (R)
  12    Reached FFFFh Value     (0=No, 1=Yes) (Reset after Reading)        (R)
*/
bitflags! {
    #[derive(Copy, Clone)]
    pub struct CounterModeRegister: u16 {
        const SYNC_ENABLE = 1;
        const RESET_COUNTER = 1 << 3;
        const COUNTER_IRQ_TARGET = 1 << 4;
        const COUNTER_IRQ_FFFF = 1 << 5;
        const IRQ_REPEAT_MODE = 1 << 6;
        const IRQ_PULSE_TOGGLE = 1 << 7;
        const INTERRUPT_REQUEST = 1 << 10;
        const REACHED_TARGET = 1 << 11;
        const REACHED_FFFF = 1 << 12;
    }
}

impl CounterModeRegister {
    pub fn sync_mode(&self) -> u32 {
        ((self.bits() >> 1) & 0x3) as u32
    }

    pub fn clock_source(&self) -> u32 {
        ((self.bits() >> 8) & 0x3) as u32
    }
}