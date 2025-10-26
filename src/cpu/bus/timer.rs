use counter_mode_register::CounterModeRegister;

use super::registers::interrupt_register::InterruptRegister;

pub mod counter_mode_register;

#[derive(Copy, Clone, Debug)]
pub struct Timer {
    pub counter_register: CounterModeRegister,
    pub counter_target: u16,
    pub counter: u32,
    timer_id: usize,
    pub clock_source: ClockSource,
    pub is_active: bool,
    pub switch_free_run: Option<bool>,
    prescalar_cycles: Option<isize>,
    pub in_xblank: bool, // either vblank or hblank, depending on the timer
    one_shot_fired: bool,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum ClockSource {
    SystemClock,
    DotClock,
    Hblank,
    SystemClockDiv8,
}

impl Timer {
    pub fn new(timer_id: usize) -> Self {
        Self {
            counter_register: CounterModeRegister::from_bits_retain(0),
            counter_target: 0,
            counter: 0,
            timer_id,
            clock_source: ClockSource::SystemClock,
            is_active: true,
            switch_free_run: None,
            prescalar_cycles: None,
            in_xblank: false,
            one_shot_fired: false,
        }
    }

    pub fn write_counter(&mut self, value: u32) {
        self.counter = value;
        self.one_shot_fired = false;
    }

    pub fn write_counter_register(&mut self, value: u16) {
        self.one_shot_fired = false;

        let mut bits = self.counter_register.bits();

        self.counter = 0;

        // clear the bottom bits except bits 10-12
        bits &= 0x7 << 10;
        // set bit 10 after writing to this register
        bits |= 1 << 10;
        // finally set the lower 9 bits to the value given
        bits |= value & 0x3ff;

        self.switch_free_run = None;
        self.prescalar_cycles = None;

        self.counter_register = CounterModeRegister::from_bits_retain(bits);

        self.clock_source = self.get_clock_source();

        if self.timer_id == 2 {
            self.is_active = !self
                .counter_register
                .contains(CounterModeRegister::SYNC_ENABLE)
                || [1, 2].contains(&self.counter_register.sync_mode())
        }

        if [0, 1].contains(&self.timer_id)
            && self
                .counter_register
                .contains(CounterModeRegister::SYNC_ENABLE)
        {
            match self.counter_register.sync_mode() {
                0 => self.is_active = !self.in_xblank,
                1 => self.is_active = true,
                2 => self.is_active = self.in_xblank,
                _ => (),
            }
        }
    }

    fn get_clock_source(&mut self) -> ClockSource {
        match self.timer_id {
            0 => match self.counter_register.clock_source() {
                0 | 2 => ClockSource::SystemClock,
                1 | 3 => ClockSource::DotClock,
                _ => unreachable!(),
            },
            1 => match self.counter_register.clock_source() {
                0 | 2 => ClockSource::SystemClock,
                1 | 3 => ClockSource::Hblank,
                _ => unreachable!(),
            },
            2 => match self.counter_register.clock_source() {
                0 | 1 => ClockSource::SystemClock,
                2 | 3 => ClockSource::SystemClockDiv8,
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
    }

    fn trigger_irq(&mut self, interrupt_stat: &mut InterruptRegister) {
        if !self.one_shot_fired {
            if !self
                .counter_register
                .contains(CounterModeRegister::IRQ_REPEAT_MODE)
            {
                self.one_shot_fired = true;
            }
            match self.timer_id {
                0 => interrupt_stat.insert(InterruptRegister::TMR0),
                1 => interrupt_stat.insert(InterruptRegister::TMR1),
                2 => interrupt_stat.insert(InterruptRegister::TMR2),
                _ => unreachable!("shouldn't happen"),
            }
        }
    }

    pub fn tick(&mut self, cycles: usize, interrupt_stat: &mut InterruptRegister) {
        if self.is_active {
            if self.prescalar_cycles.is_none() {
                self.update_prescalar(0);
            }

            if self.prescalar_cycles.is_none() {
                let previous_counter = self.counter;
                self.counter += cycles as u32;

                self.check_if_overflow(previous_counter, interrupt_stat);
            } else {
                if let Some(prescalar_cycles) = &mut self.prescalar_cycles {
                    *prescalar_cycles -= cycles as isize;

                    if *prescalar_cycles <= 0 {
                        let previous_counter = self.counter;
                        let cycles_left = *prescalar_cycles;
                        self.counter += 1;

                        self.update_prescalar(cycles_left);

                        self.check_if_overflow(previous_counter, interrupt_stat);
                    }
                }
            }
        }
    }

    fn check_if_overflow(&mut self, previous_counter: u32, interrupt_stat: &mut InterruptRegister) {
        if (self.counter >= 0xffff
            && !self
                .counter_register
                .contains(CounterModeRegister::RESET_COUNTER))
            || (previous_counter < self.counter_target as u32
                && self.counter >= self.counter_target as u32
                && self
                    .counter_register
                    .contains(CounterModeRegister::RESET_COUNTER))
        {
            self.on_overflow_or_target(previous_counter, interrupt_stat);
        }
    }

    fn update_prescalar(&mut self, cycles_left: isize) {
        // we add cycles_left because it's either 0 or a negative number,
        // and we want to subtract the cycles left from the prescalar
        if self.clock_source == ClockSource::SystemClockDiv8 {
            self.prescalar_cycles = Some(8 + cycles_left);
        } else {
            self.prescalar_cycles = None;
        }
    }

    pub fn on_overflow_or_target(
        &mut self,
        previous_counter: u32,
        interrupt_stat: &mut InterruptRegister,
    ) {
        if self
            .counter_register
            .contains(CounterModeRegister::RESET_COUNTER)
            && self.counter_target == 0
            && self.counter == 0
        {
            self.counter_register
                .insert(CounterModeRegister::REACHED_TARGET);

            if self
                .counter_register
                .contains(CounterModeRegister::COUNTER_IRQ_TARGET)
            {
                self.trigger_irq(interrupt_stat);
            }

            return;
        }

        let current_cycles = self.counter;

        if current_cycles >= 0xffff {
            self.counter = current_cycles as u32 - 0xffff;

            if !self
                .counter_register
                .contains(CounterModeRegister::RESET_COUNTER)
            {
                self.counter_register
                    .insert(CounterModeRegister::REACHED_FFFF);
            }

            if self
                .counter_register
                .contains(CounterModeRegister::COUNTER_IRQ_FFFF)
            {
                self.trigger_irq(interrupt_stat);
            }
        } else if previous_counter < self.counter_target as u32
            && current_cycles >= self.counter_target as u32
        {
            if self
                .counter_register
                .contains(CounterModeRegister::RESET_COUNTER)
            {
                self.counter = current_cycles as u32 - self.counter_target as u32;

                self.counter_register
                    .insert(CounterModeRegister::REACHED_TARGET);
            }

            if self
                .counter_register
                .contains(CounterModeRegister::COUNTER_IRQ_TARGET)
            {
                self.trigger_irq(interrupt_stat);
            }
        }
    }
}
