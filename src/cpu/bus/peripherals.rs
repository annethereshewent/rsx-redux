use crate::cpu::bus::peripherals::sio_control::SIOControl;

pub mod sio_control;

pub struct Peripherals {
    ctrl: SIOControl,
    baudrate_timer: u16,
}

impl Peripherals {
    pub fn new() -> Self {
        Self {
            ctrl: SIOControl::from_bits_retain(0),
            baudrate_timer: 0,
        }
    }
    pub fn write_ctrl(&mut self, value: u16) {
        self.ctrl = SIOControl::from_bits_retain(value);

        if self.ctrl.contains(SIOControl::ACK) {
            todo!("implement sio ctrl ack");
        }

        if self.ctrl.contains(SIOControl::RESET) {
            println!("todo: reset SIO registers");
        }
    }

    pub fn write_reload_rate(&mut self, value: u16) {
        self.baudrate_timer = value;
    }
}
