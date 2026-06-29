//! A sample file used by the CLI golden tests.

/// Greets a person by name.
pub fn greet(name: &str) -> String {
    // Build the greeting.
    format!("Hello, {name}!")
}

pub struct Counter {
    value: i64,
}

impl Counter {
    pub fn new() -> Self {
        Counter { value: 0 }
    }

    pub fn increment(&mut self) -> i64 {
        self.value += 1;
        self.value
    }
}
