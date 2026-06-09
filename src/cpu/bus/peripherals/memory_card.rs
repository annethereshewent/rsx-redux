#[cfg(not(target_arch = "wasm32"))]
use memmap2::MmapMut;
use serde::{Deserialize, Serialize};

#[derive(Default, PartialEq, Serialize, Deserialize)]
enum CardState {
    #[default]
    Idle,
    AwaitingCommand,
    Writing,
    Reading,
    GetId,
}

pub const MEMORY_SIZE: usize = 0x20000;

#[derive(Default, Serialize, Deserialize)]
pub struct MemoryCard {
    card_state: CardState,
    flag_byte: u8,
    step: usize,
    finished_transferring: bool,
    current_sector: u16,
    current_byte: usize,
    checksum: u8,
    checksum_match: bool,
    previous: u8,
    #[cfg(not(target_arch = "wasm32"))]
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    memory_file: Option<MmapMut>,
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    #[cfg(target_arch = "wasm32")]
    memory_bytes: Option<Vec<u8>>
}

impl MemoryCard {
    pub fn in_ack(&self) -> bool {
        self.card_state != CardState::Idle
    }
    pub fn new() -> Self {
        Self {
            card_state: CardState::Idle,
            flag_byte: 0x8,
            step: 0,
            finished_transferring: false,
            current_sector: 0,
            current_byte: 0,
            checksum: 0,
            previous: 0,
            checksum_match: false,
            #[cfg(not(target_arch = "wasm32"))]
            memory_file: None,
            #[cfg(target_arch = "wasm32")]
            memory_bytes: None,
        }
    }
    pub fn reply(&mut self, command: u8) -> u8 {
        let mut reply = 0xff;
        match self.card_state {
            CardState::Idle => self.card_state = CardState::AwaitingCommand,
            CardState::AwaitingCommand => {
                reply = self.flag_byte;
                match command {
                    0x57 => {
                        self.card_state = CardState::Writing;
                        self.step = 0;
                        self.checksum = 0;
                        self.previous = 0;
                        self.current_byte = 0;
                        self.current_sector = 0;
                        self.finished_transferring = false;
                        self.checksum_match = false;
                    }
                    0x52 => {
                        self.card_state = CardState::Reading;
                        self.step = 0;
                        self.checksum = 0;
                        self.previous = 0;
                        self.current_byte = 0;
                        self.current_sector = 0;
                        self.finished_transferring = false;
                    }
                    0x53 => {
                        self.card_state = CardState::GetId;
                        self.step = 0;
                    }
                    _ => {
                        println!("[WARN]: invalid byte received for memory card: 0x{command:x}");
                        self.card_state = CardState::Idle;
                    }
                }
            }
            CardState::GetId => {
                reply = self.handle_get_id();
            }
            CardState::Reading => {
                reply = self.handle_read_command(command);
            }
            CardState::Writing => {
                reply = self.handle_write_command(command);
            }
        }

        reply
    }

    fn handle_get_id(&mut self) -> u8 {
        let return_byte = match self.step {
            0 => 0x5a,
            1 => 0x5d,
            2 => 0x5c,
            3 => 0x5d,
            4 => 0x4,
            5 => 0x0,
            6 => 0x0,
            7 => 0x80,
            _ => unreachable!(),
        };

        if self.step == 7 {
            self.card_state = CardState::Idle;
            self.step = 0;
        } else {
            self.step += 1;
        }

        return_byte
    }

    fn handle_write_command(&mut self, command: u8) -> u8 {
        let return_byte = match self.step {
            0 => {
                self.flag_byte &= !0x8;
                0x5a
            }
            1 => 0x5d,
            2 => {
                self.previous = command;
                self.current_sector = (command as u16) << 8;
                self.checksum = command;
                0x0
            }
            3 => {
                let previous = self.previous;
                self.previous = command;
                self.current_sector |= command as u16;

                self.checksum ^= command;

                previous
            }
            4 => {
                let previous = self.previous;
                self.previous = command;
                self.checksum ^= command;

                #[cfg(not(target_arch = "wasm32"))]
                if let Some(memory_file) = &mut self.memory_file {
                    memory_file[(128 * self.current_sector as usize) + self.current_byte] = command;
                }
                #[cfg(target_arch = "wasm32")]
                if let Some(memory_bytes) = &mut self.memory_bytes {
                    memory_bytes[(128 * self.current_sector as usize) + self.current_byte] = command;
                }

                self.current_byte += 1;
                if self.current_byte == 128 {
                    #[cfg(not(target_arch = "wasm32"))]
                    if let Some(memory_file) = &mut self.memory_file {
                        memory_file.flush().unwrap();
                    }
                    self.finished_transferring = true;
                }
                previous
            }
            5 => {
                self.checksum_match = self.checksum == command;
                self.checksum
            }
            6 => 0x5c,
            7 => 0x5d,
            8 => {
                if self.checksum_match {
                    0x47
                } else {
                    0x4e
                }
            }
            _ => unreachable!(),
        };

        if self.step == 8 {
            self.card_state = CardState::Idle;
        } else if (self.step == 4 && self.finished_transferring) || self.step != 4 {
            self.step += 1;
        }

        return_byte
    }

    fn handle_read_command(&mut self, command: u8) -> u8 {
        let return_byte = match self.step {
            0 => 0x5a,
            1 => 0x5d,
            2 => {
                self.current_sector = (command as u16) << 8;
                self.checksum = command;

                self.previous = command;

                0x0
            }
            3 => {
                self.current_sector |= command as u16;
                self.checksum ^= command;

                self.previous
            }
            4 => 0x5c,
            5 => 0x5d,
            6 => (self.current_sector >> 8) as u8,
            7 => {
                let return_byte = self.current_sector as u8;

                if self.current_sector > 0x3ff {
                    self.card_state = CardState::Idle;
                }

                return_byte
            }
            8 => {
                #[cfg(not(target_arch = "wasm32"))]
                let return_byte = if let Some(memory_file) = &self.memory_file {
                    memory_file[(128 * self.current_sector as usize) + self.current_byte]
                } else {
                    0xff
                };
                #[cfg(target_arch = "wasm32")]
                let return_byte = if let Some(memory_bytes) = &self.memory_bytes {
                    memory_bytes[(128 * self.current_sector as usize) + self.current_byte]
                } else {
                    0xff
                };

                self.current_byte += 1;
                if self.current_byte == 128 {
                    self.finished_transferring = true;
                }

                self.checksum ^= return_byte;

                return_byte
            }
            9 => self.checksum,
            10 => 0x47,
            _ => unreachable!(),
        };

        if self.step == 10 {
            self.card_state = CardState::Idle;
            self.step = 0;
        } else if (self.step == 8 && self.finished_transferring) || self.step != 8 {
            self.step += 1;
        }

        return_byte
    }

    pub fn reset(&mut self) {
        self.card_state = CardState::Idle;
        self.step = 0;
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_memory_file(&mut self, memory_file: Option<MmapMut>) {
        self.memory_file = memory_file;
    }

    #[cfg(target_arch = "wasm32")]
    pub fn set_memory_bytes(&mut self, memory_bytes: Vec<u8>) {
        self.memory_bytes = Some(memory_bytes);
    }
}
