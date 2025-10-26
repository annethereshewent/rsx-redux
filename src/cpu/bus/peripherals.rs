use std::collections::VecDeque;

use crate::cpu::bus::{
    peripherals::{controller::Controller, sio_control::SIOControl, sio_mode::SIOMode},
    registers::interrupt_register::InterruptRegister,
    scheduler::{EventType, Scheduler},
};

pub mod controller;
pub mod sio_control;
pub mod sio_mode;

const CONTROLLER_CYCLES: usize = 338;

#[derive(Copy, Clone, PartialEq)]
enum SelectedPeripheral {
    None,
    MemoryCard,
    Controller,
}

#[derive(Copy, Clone, PartialEq)]
enum PeripheralState {
    Idle,
    Transferring,
    Acknowledge,
}

pub struct Peripherals {
    ctrl: SIOControl,
    baudrate_timer: u16,
    mode: SIOMode,
    tx_fifo: VecDeque<u8>,
    rx_fifo: VecDeque<u8>,
    tx_idle: bool,
    tx_ready: bool,
    state: PeripheralState,
    selected_peripheral: SelectedPeripheral,
    pub controller: Controller,
}

impl Peripherals {
    pub fn new() -> Self {
        Self {
            ctrl: SIOControl::from_bits_retain(0),
            baudrate_timer: 0,
            mode: SIOMode::new(),
            tx_fifo: VecDeque::new(),
            rx_fifo: VecDeque::new(),
            tx_idle: false,
            tx_ready: false,
            state: PeripheralState::Idle,
            selected_peripheral: SelectedPeripheral::None,
            controller: Controller::new(),
        }
    }

    pub fn write_ctrl(&mut self, value: u16, scheduler: &mut Scheduler) {
        self.ctrl = SIOControl::from_bits_retain(value);

        // TODO: handle Acknowledgements as well as bits
        // related to them

        if !self.ctrl.contains(SIOControl::DTR_OUT) {
            scheduler.remove(EventType::ControllerByteTransfer);
            self.selected_peripheral = SelectedPeripheral::None;
            self.state = PeripheralState::Idle;
            self.controller.reset();
            // TODO: Reset memory card state
        }

        if self.ctrl.contains(SIOControl::RESET) {
            self.write_ctrl(0, scheduler);
            self.write_mode(0);
            self.write_reload_rate(0);

            self.tx_fifo.clear();
            self.rx_fifo.clear();

            self.tx_idle = true;
            self.tx_ready = true;
        }
    }

    pub fn read_byte(&mut self) -> u8 {
        if let Some(byte) = self.rx_fifo.pop_front() {
            return byte;
        }

        0
    }

    pub fn handle_peripherals(
        &mut self,
        interrupt: &mut InterruptRegister,
        scheduler: &mut Scheduler,
    ) {
        match self.state {
            PeripheralState::Acknowledge => self.handle_ack(interrupt),
            PeripheralState::Idle => unreachable!("shouldn't happen"),
            PeripheralState::Transferring => self.handle_transfer(scheduler),
        }
    }

    fn handle_transfer(&mut self, scheduler: &mut Scheduler) {
        // port 1 aka controller 2 is unsupported. returning back a dummy byte
        if self.ctrl.contains(SIOControl::SIO_PORT_SELECT) {
            self.rx_fifo.push_back(0xff);
            return;
        }

        let command = self.tx_fifo.pop_front().unwrap();

        if self.selected_peripheral == SelectedPeripheral::None {
            if command == 0x1 {
                self.selected_peripheral = SelectedPeripheral::Controller;
            } else if command == 0x81 {
                self.selected_peripheral = SelectedPeripheral::MemoryCard;
            }
        }

        let reply = match self.selected_peripheral {
            SelectedPeripheral::Controller => self.controller.reply(command),
            _ => 0xff,
        };

        if self.controller.in_ack() {
            self.state = PeripheralState::Acknowledge;
            scheduler.schedule(EventType::ControllerByteTransfer, CONTROLLER_CYCLES);
        } else {
            self.selected_peripheral = SelectedPeripheral::None;
            self.state = PeripheralState::Idle;
        }

        self.rx_fifo.push_back(reply);
    }

    fn handle_ack(&mut self, interrupt: &mut InterruptRegister) {
        self.state = PeripheralState::Idle;

        interrupt.insert(InterruptRegister::PERIPHERAL);
    }

    pub fn write_byte(&mut self, value: u8, scheduler: &mut Scheduler) {
        // TODO: deal with latched TXEN values, apparently it's an issue in Wipeout
        if self.ctrl.contains(SIOControl::TX_ENABLE) {
            self.tx_fifo.push_back(value);
        }

        let cycles = (self.baudrate_timer & !1) * 8;

        scheduler.schedule(EventType::ControllerByteTransfer, cycles as usize);

        self.state = PeripheralState::Transferring;

        self.tx_idle = false;
        self.tx_ready = true;
    }

    pub fn read_stat(&self) -> u16 {
        (self.tx_ready as u16)
            | (!self.rx_fifo.is_empty() as u16) << 1
            | (self.tx_idle as u16) << 2
            | ((self.state == PeripheralState::Acknowledge) as u16) << 7
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
