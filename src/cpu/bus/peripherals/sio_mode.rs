pub struct SIOMode {
    value: u16,
}

impl SIOMode {
    pub fn new() -> Self {
        Self { value: 0 }
    }

    pub fn write(&mut self, value: u16) {
        self.value = value;
    }
}
