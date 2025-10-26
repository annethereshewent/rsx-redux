pub struct Controller {
    state: usize,
    digital_mode: bool,
    buttons: u16,
    left_joy_x: u8,
    left_joy_y: u8,
    right_joy_x: u8,
    right_joy_y: u8,
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

    pub fn reply(&mut self, command: u8) -> u8 {
        let mut reset_state = false;
        let reply = match self.state {
            0 => 0xff,
            1 => {
                if command == 0x42 {
                    // these are gotten from psx-spx, basically the lower bits
                    // of the halfwords identifying the controller
                    //
                    // 5A73h=Analog Pad (in normal analog mode; LED=Red)
                    // 5A41h=Digital Pad (or analog pad/stick in digital mode; LED=Off)
                    if self.digital_mode { 0x41 } else { 0x73 }
                } else {
                    reset_state = true;

                    0xff
                }
            }
            2 => 0x5a,
            3 => self.buttons as u8,
            4 => {
                if self.digital_mode {
                    reset_state = true;
                }

                (self.buttons >> 8) as u8
            }
            5 => self.right_joy_x,
            6 => self.right_joy_y,
            7 => self.left_joy_x,
            8 => {
                reset_state = true;
                self.left_joy_y
            }
            _ => unreachable!(),
        };

        self.state = if reset_state { 0 } else { self.state + 1 };

        reply
    }
}
