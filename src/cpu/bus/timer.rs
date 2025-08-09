use counter_mode_register::CounterModeRegister;

use super::{registers::interrupt_register::InterruptRegister, scheduler::{EventType, Scheduler}};

pub mod counter_mode_register;

#[derive(Copy, Clone)]
pub struct Timer {
    pub counter_register: CounterModeRegister,
    pub counter_target: u16,
    pub counter: u32,
    timer_id: usize,
    initial_time: usize,
    initial_cycles: usize,
    pub clock_source: ClockSource,
    pub is_active: bool,
    pub switch_free_run: Option<bool>
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
            initial_time: 0,
            initial_cycles: 0,
            clock_source: ClockSource::SystemClock,
            is_active: false,
            switch_free_run: None
        }
    }

    pub fn write_counter_register(&mut self, value: u16, scheduler: &mut Scheduler) {
        let mut bits = self.counter_register.bits();

        // clear the bottom bits except bits 10-12
        bits &= 0x7 << 10;
        // set bit 10 after writing to this register
        bits |= 1 << 10;
        // finally set the lower 9 bits to the value given
        bits |= value & 0x3ff;

        self.switch_free_run = None;

        if self.counter_register.sync_mode() == 2 {
            self.is_active = false;
        } else {
            self.is_active = true;
        }

        self.counter_register = CounterModeRegister::from_bits_retain(bits);

        self.schedule_next_timer(scheduler);

        self.counter = 0;
    }

    pub fn read_counter(&self, scheduler: &Scheduler) -> u16 {
        let prescalar = match self.timer_id {
            2 => match self.counter_register.clock_source() {
                0 | 2 => 1,
                1 | 3 => 8,
                _ => unreachable!()
            }
            _ => 1
        };

        (self.initial_time + (scheduler.cycles - self.initial_cycles) / prescalar) as u16
    }

    fn trigger_irq(&self, interrupt_stat: &mut InterruptRegister) {
        match self.timer_id {
            0 => interrupt_stat.insert(InterruptRegister::TMR0),
            1 => interrupt_stat.insert(InterruptRegister::TMR1),
            2 => interrupt_stat.insert(InterruptRegister::TMR2),
            _ => unreachable!("shouldn't happen")
        }
    }

    pub fn tick(&mut self, cycles: usize, scheduler: &mut Scheduler, interrupt_stat: &mut InterruptRegister) {
        let prescalar = match self.clock_source {
            ClockSource::DotClock => 165,
            ClockSource::Hblank => 2153,
            ClockSource::SystemClockDiv8 => 8,
            ClockSource::SystemClock => 1
        };

        self.counter = self.counter + (cycles * prescalar) as u32;

        if self.counter >= 0xffff {
            self.on_overflow_or_target(scheduler, interrupt_stat);
        }
    }

    pub fn schedule_next_timer(&mut self, scheduler: &mut Scheduler) {
        let clock_source = match self.timer_id {
            0 => match self.counter_register.clock_source() {
                0 | 2 => ClockSource::SystemClock,
                1 | 3 => ClockSource::DotClock,
                _ => unreachable!()
            }
            1 => match self.counter_register.clock_source() {
                0 | 2 => ClockSource::SystemClock,
                1 | 3 => ClockSource::Hblank,
                _ => unreachable!()
            }
            2 => match self.counter_register.clock_source() {
                0 | 2 => ClockSource::SystemClock,
                1 | 3 => ClockSource::SystemClockDiv8,
                _ => unreachable!()
            }
            _ => unreachable!("invalid channel id")
        };

        self.initial_time = self.counter as usize;
        self.initial_cycles = scheduler.cycles;

        if clock_source == ClockSource::SystemClock {
            if !self.counter_register.contains(CounterModeRegister::RESET_COUNTER) || ((self.counter_target as u32) < self.counter) {
                scheduler.schedule(EventType::Timer(self.timer_id), (0xffff - self.counter) as usize);
            } else if self.counter_register.contains(CounterModeRegister::RESET_COUNTER) {
                scheduler.schedule(EventType::Timer(self.timer_id), (self.counter_target as u32 - self.counter) as usize);
            }
        } else if clock_source == ClockSource::SystemClockDiv8 {
            if !self.counter_register.contains(CounterModeRegister::RESET_COUNTER) || (self.counter_target as u32) < self.counter {
                scheduler.schedule(EventType::Timer(self.timer_id), (0xffff - self.counter) as usize * 8);
            } else if self.counter_register.contains(CounterModeRegister::RESET_COUNTER) {
                scheduler.schedule(EventType::Timer(self.timer_id), (self.counter_target as u32 - self.counter) as usize * 8);
            }
        }
    }

    pub fn on_overflow_or_target(&mut self, scheduler: &mut Scheduler, interrupt_stat: &mut InterruptRegister) {
        let prescalar = match self.timer_id {
            2 => match self.counter_register.clock_source() {
                0 | 2 => 1,
                1 | 3 => 8,
                _ => unreachable!()
            }
            _ => 1
        };
        let current_cycles = self.initial_time + (scheduler.cycles - self.initial_cycles) / prescalar;
        if current_cycles >= 0xffff {
            self.counter = 0;

            if !self.counter_register.contains(CounterModeRegister::RESET_COUNTER) {
                self.counter_register.insert(CounterModeRegister::REACHED_FFFF);
            }

            if self.counter_register.contains(CounterModeRegister::COUNTER_IRQ_FFFF) {
                self.trigger_irq(interrupt_stat);
            }
        } else if self.counter < self.counter_target as u32 && current_cycles >= self.counter_target as usize {
            if self.counter_register.contains(CounterModeRegister::RESET_COUNTER) {

                self.counter_register.insert(CounterModeRegister::REACHED_TARGET);

                self.counter = 0;
            }

            if self.counter_register.contains(CounterModeRegister::COUNTER_IRQ_TARGET) {
                self.trigger_irq(interrupt_stat);
            }
        }
        // schedule next timer if applicable
        self.schedule_next_timer(scheduler);
    }
}