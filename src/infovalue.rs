use crate::ducoapi::StatusValue;

pub const UNKNOWN: &str = "UNKNOWN";

pub struct InfoValue {
    value: StatusValue,
    modified: bool,
}

impl InfoValue {
    pub fn new(value: StatusValue) -> Self {
        Self { value, modified: true }
    }

    pub fn set(&mut self, val: StatusValue) {
        if val != self.value {
            self.modified = true;
            self.value = val;
        }
    }

    pub fn modified(&self) -> bool {
        self.modified
    }

    pub fn get_and_reset(&mut self) -> StatusValue {
        self.modified = false;
        self.value.clone()
    }
}
