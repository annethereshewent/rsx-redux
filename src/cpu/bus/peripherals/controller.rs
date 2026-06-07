use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
pub struct Controller {
    state: usize,
    pub digital_mode: bool,
    buttons: u16,
    left_joy_x: u8,
    left_joy_y: u8,
    right_joy_x: u8,
    right_joy_y: u8,
    rumble_on: bool,
    config_mode: bool,
    controller_command: u8,
    variable_byte: u8,
    current_vibration: [u8; 6],
    vibration_latch: [u8; 6],
    small_motor: bool,
    large_motor: u8,

}

impl Controller {
    pub fn new() -> Self {
        Self {
            state: 0,
            digital_mode: true,
            buttons: 0xffff,
            left_joy_x: 0x80,
            right_joy_x: 0x80,
            left_joy_y: 0x80,
            right_joy_y: 0x80,
            rumble_on: false,
            config_mode: false,
            controller_command: 0,
            variable_byte: 0,
            current_vibration: [0xff; 6],
            vibration_latch: [0xff; 6],
            small_motor: false,
            large_motor: 0,
        }
    }

    pub fn in_ack(&self) -> bool {
        self.state != 0
    }

    pub fn update_input(&mut self, index: usize, is_pressed: bool) {
        if is_pressed {
            self.buttons &= !(1 << index)
        } else {
            self.buttons |= 1 << index;
        }
    }

    pub fn reset(&mut self) {
        self.state = 0;
    }

    pub fn set_leftx(&mut self, value: u8) {
        self.left_joy_x = value;
    }

    pub fn set_lefty(&mut self, value: u8) {
        self.left_joy_y = value;
    }

    pub fn set_rightx(&mut self, value: u8) {
        self.right_joy_x = value;
    }

    pub fn set_righty(&mut self, value: u8) {
        self.right_joy_y = value;
    }

    pub fn reply(&mut self, command: u8) -> u8 {
        let mut reset_state = false;
        let reply = match self.state {
            0 => 0xff,
            1 => {
                self.controller_command = command;

                match command {
                    0x42 => {
                        // these are gotten from psx-spx, basically the lower bits
                        // of the halfwords identifying the controller
                        //
                        // 5A73h=Analog Pad (in normal analog mode; LED=Red)
                        // 5A41h=Digital Pad (or analog pad/stick in digital mode; LED=Off)
                        if self.digital_mode { 0x41 } else { 0x73 }
                    }
                    0x43 =>  if self.config_mode {
                        0xf3
                    } else {
                        if self.digital_mode { 0x41 } else { 0x73 }
                    }
                    0x45 | 0x4c | 0x46 | 0x47 | 0x4d => 0xf3,
                    _ => {
                        println!("[WARN]got unimplemented command 0x{command:x}, resetting state");
                        reset_state = true;
                        0xff
                    }
                }
            }
            2 => 0x5a,
            3 => {
                match self.controller_command {
                    0x42 => {
                        self.update_vibration(command);
                        self.buttons as u8
                    }
                    0x43 => {
                        self.variable_byte = command;
                        if self.config_mode {
                            0x0
                        } else {
                            self.buttons as u8
                        }
                    }
                    0x45 => {
                        0x1
                    }
                    0x46 => {
                        self.variable_byte = command;
                        0x0
                    }
                    0x4c => {
                        self.variable_byte = command;
                        0x0
                    }
                    0x47 => 0,
                    0x4d => {
                        self.vibration_latch[self.state - 3] = command;
                        self.current_vibration[self.state - 3]
                    }
                    _ => panic!("config command not yet implemented: 0x{command:x}")

                }

            }
            4 => {
                match self.controller_command {
                    0x43 => if self.config_mode {
                        0x0
                    } else {
                        (self.buttons >> 8) as u8
                    }
                    0x45 => 0x2,
                    0x46 => 0x0,
                    0x4c => 0x0,
                    0x47 => 0x0,
                    0x4d => {
                        self.vibration_latch[self.state - 3] = command;
                        self.current_vibration[self.state - 3]
                    }
                    0x42 => {
                        self.update_vibration(command);
                        if self.digital_mode && !self.config_mode {
                            reset_state = true;
                        }

                        (self.buttons >> 8) as u8
                    }
                    _ => panic!("config command not yet implemented: 0x{command:x}")
                }

            }
            5 => match self.controller_command {
                0x43 => if self.config_mode {
                    0x0
                } else {
                    self.right_joy_x
                }
                0x45 => !self.digital_mode as u8,
                0x4c => 0x0,
                0x46 => match self.variable_byte {
                    0x0 | 0x1 => 0x1,
                    _ => 0x0,
                }
                0x47 => 0x2,
                0x4d => {
                    self.vibration_latch[self.state - 3] = command;
                    self.current_vibration[self.state - 3]
                }
                0x42 => {
                    self.update_vibration(command);
                    self.right_joy_x
                }
                _ => panic!("config command not yet implemented: 0x{command:x}")
            }
            6 => match self.controller_command {
                0x43 => if self.config_mode {
                    0x0
                } else {
                    self.right_joy_y
                }
                0x45 => 0x2,
                0x46 => match self.variable_byte {
                    0x0 => 0x2,
                    0x1 => 0x1,
                    _ => 0x0,
                }
                0x4c => match self.variable_byte {
                    0x0 => 0x4,
                    0x1 => 0x7,
                    _ => 0x0,
                },
                0x47 => 0x0,
                0x4d => {
                    self.vibration_latch[self.state - 3] = command;
                    self.current_vibration[self.state - 3]
                }
                0x42 => {
                    self.update_vibration(command);
                    self.right_joy_y
                }
                _ => panic!("config command not yet implemented: 0x{command:x}")
            },
            7 => match self.controller_command {
                0x43 => if self.config_mode {
                    0x0
                } else {
                    self.left_joy_x
                }
                0x45 => 0x1,
                0x46 => match self.variable_byte {
                    0x0 => 0x0,
                    0x1 => 0x1,
                    _ => 0x0,
                }
                0x4c => 0x0,
                0x47 => 0x1,
                0x4d => {
                    self.vibration_latch[self.state - 3] = command;
                    self.current_vibration[self.state - 3]
                }
                0x42 => {
                    self.update_vibration(command);
                    self.left_joy_x
                }
                _ => panic!("config command not yet implemented: 0x{command:x}")
            }
            8 => {
                reset_state = true;

                match self.controller_command {
                    0x43 => {
                        let return_byte = if self.config_mode {
                            0x0
                        } else {
                            self.left_joy_y
                        };

                        if self.variable_byte == 0x1 {
                            self.config_mode = true;
                        } else if self.variable_byte == 0x0 {
                            self.config_mode = false;
                        }

                        return_byte
                    }
                    0x45 => 0x0,
                    0x46 => match self.variable_byte {
                        0x0 => 0xa,
                        0x1 => 0x14,
                        _ => 0x0,
                    }
                    0x4c => 0x0,
                    0x47 => 0x0,
                    0x4d => {
                        self.vibration_latch[self.state - 3] = command;
                        let reply = self.current_vibration[self.state - 3];

                        for i in 0..6 {
                            self.current_vibration[i] = self.vibration_latch[i];
                        }

                        reply
                    }
                    0x42 => {
                        self.update_vibration(command);
                        self.left_joy_y
                    }
                    _ => panic!("config command not yet implemented: 0x{command:x}")
                }
            }
            _ => unreachable!(),
        };

        self.state = if reset_state { 0 } else { self.state + 1 };

        reply
    }

    fn update_vibration(&mut self, value: u8) {
        if self.current_vibration[self.state - 3] == 0x0 {
            self.small_motor = value & 1 == 1;
        } else if self.current_vibration[self.state - 3] == 0x1 {
            self.large_motor = value;
        }
    }

    pub fn get_rumble(&self) -> (bool, u8) {
        (self.small_motor, self.large_motor)
    }
}
