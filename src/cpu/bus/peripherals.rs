use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::cpu::bus::{
    peripherals::{
        controller::Controller, memory_card::MemoryCard, sio_control::SIOControl, sio_mode::SIOMode,
    },
    registers::interrupt_register::InterruptRegister,
    scheduler::{EventType, Scheduler},
};

pub mod controller;
pub mod memory_card;
pub mod sio_control;
pub mod sio_mode;

const CONTROLLER_CYCLES: usize = 338;

#[derive(Copy, Clone, PartialEq, Serialize, Deserialize)]
enum SelectedPeripheral {
    None,
    MemoryCard,
    Controller,
}

#[derive(Copy, Clone, PartialEq, Serialize, Deserialize)]
enum PeripheralState {
    Idle,
    Transferring,
    Acknowledge,
}

#[derive(Serialize, Deserialize)]
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
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    pub memory_card: MemoryCard,
    interrupt: bool,
    rx_parity_error: bool,
}

impl Default for Peripherals {
    fn default() -> Self {
        Self::new()
    }
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
            memory_card: MemoryCard::new(),
            interrupt: false,
            rx_parity_error: false,
        }
    }

    pub fn write_ctrl(&mut self, value: u16, scheduler: &mut Scheduler) {
        self.ctrl = SIOControl::from_bits_retain(value);

        if !self.ctrl.contains(SIOControl::DTR_OUT) {
            scheduler.remove(EventType::ControllerByteTransfer);
            self.selected_peripheral = SelectedPeripheral::None;
            self.state = PeripheralState::Idle;
            self.controller.reset();
            self.memory_card.reset();
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

        // TODO: actually figure out what rx_parity_error flag is supposed to be set besides false
        // currently it's always false so this does nothing.
        if self.ctrl.contains(SIOControl::ACK) && self.state != PeripheralState::Acknowledge {
            self.rx_parity_error = false;
            self.interrupt = false;
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
        let command = self.tx_fifo.pop_front().unwrap();
        // port 1 aka controller 2 is unsupported. returning back a dummy byte
        if self.ctrl.contains(SIOControl::SIO_PORT_SELECT) {
            self.rx_fifo.push_back(0xff);
            return;
        }

        if self.selected_peripheral == SelectedPeripheral::None {
            if command == 0x1 {
                self.selected_peripheral = SelectedPeripheral::Controller;
            } else if command == 0x81 {
                self.selected_peripheral = SelectedPeripheral::MemoryCard;
            }
        }

        let (reply, in_ack) = match self.selected_peripheral {
            SelectedPeripheral::Controller => {
                (self.controller.reply(command), self.controller.in_ack())
            }
            SelectedPeripheral::MemoryCard => {
                (self.memory_card.reply(command), self.memory_card.in_ack())
            }
            _ => (0xff, false),
        };

        if in_ack {
            self.state = PeripheralState::Acknowledge;
            scheduler.schedule(EventType::ControllerByteTransfer, CONTROLLER_CYCLES);
        } else {
            self.selected_peripheral = SelectedPeripheral::None;
            self.state = PeripheralState::Idle;
        }

        self.rx_fifo.push_back(reply);

        self.tx_idle = true;
    }

    fn handle_ack(&mut self, interrupt: &mut InterruptRegister) {
        self.state = PeripheralState::Idle;

        self.interrupt = true;

        interrupt.insert(InterruptRegister::PERIPHERAL);
    }

    pub fn write_byte(&mut self, value: u8, scheduler: &mut Scheduler) {
        // TODO: deal with latched TXEN values, apparently it's an issue in Wipeout
        if self.tx_fifo.is_empty() {
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
            | (self.rx_parity_error as u16) << 3
            | ((self.state == PeripheralState::Acknowledge) as u16) << 7
            | (self.interrupt as u16) << 9
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
