use counter_mode_register::CounterModeRegister;

pub mod counter_mode_register;

#[derive(Copy, Clone)]
pub struct Timer {
    pub counter_register: CounterModeRegister,
    pub counter_target: u16,
    pub counter: u16
}

impl Timer {
    pub fn new() -> Self {
        Self {
            counter_register: CounterModeRegister::from_bits_retain(0),
            counter_target: 0,
            counter: 0
        }
    }

    pub fn write_counter_register(&mut self, value: u16) {
        self.counter_register = CounterModeRegister::from_bits_retain(value);
        self.counter = 0;
    }
}