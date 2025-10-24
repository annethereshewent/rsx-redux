use std::collections::VecDeque;

use crate::cpu::bus::peripherals::{sio_control::SIOControl, sio_mode::SIOMode};

pub mod sio_control;
pub mod sio_mode;

pub struct Peripherals {
    ctrl: SIOControl,
    baudrate_timer: u16,
    mode: SIOMode,
    tx_fifo: VecDeque<u8>,
    rx_fifo: VecDeque<u8>,
}

impl Peripherals {
    pub fn new() -> Self {
        Self {
            ctrl: SIOControl::from_bits_retain(0),
            baudrate_timer: 0,
            mode: SIOMode::new(),
            tx_fifo: VecDeque::new(),
            rx_fifo: VecDeque::new(),
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

    pub fn read_byte(&mut self) -> u8 {
        if let Some(byte) = self.tx_fifo.pop_front() {
            return byte;
        }

        0
    }

    pub fn write_byte(&mut self, value: u8) {
        // TODO: deal with latched TXEN values, apparently it's an issue in Wipeout
        if self.ctrl.contains(SIOControl::TX_ENABLE) {
            self.tx_fifo.push_back(value);
        }
    }

    pub fn read_stat(&self) -> u16 {
        1 | (!self.rx_fifo.is_empty() as u16) << 1
            | (self.ctrl.contains(SIOControl::TX_ENABLE) as u16) << 2
            | self.baudrate_timer << 11
    }

    pub fn write_reload_rate(&mut self, value: u16) {
        self.baudrate_timer = value;
    }

    pub fn read_ctrl(&self) -> u16 {
        self.ctrl.bits()
    }

    pub fn write_mode(&mut self, value: u16) {
        self.mode.write(value);
    }
}
