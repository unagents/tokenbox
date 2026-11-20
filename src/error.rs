//! Failure kinds and their exit codes (SPEC §3).

use std::fmt;

#[derive(Debug)]
pub enum Failure {
    /// Wrong usage or unusable input. Exit code 1.
    Usage(String),
    /// The key name is wrong or not registered. Exit code 2.
    Decrypt(String),
    /// The store cannot be reached, fetched from or pushed to. Exit code 3.
    Store(String),
    /// The store asked for authentication on read; it must be public. Exit code 4.
    AuthRequired(String),
}

impl Failure {
    pub fn exit_code(&self) -> i32 {
        match self {
            Failure::Usage(_) => 1,
            Failure::Decrypt(_) => 2,
            Failure::Store(_) => 3,
            Failure::AuthRequired(_) => 4,
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Failure::Usage(m) | Failure::Decrypt(m) | Failure::Store(m) | Failure::AuthRequired(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for Failure {}

pub type Result<T> = std::result::Result<T, Failure>;
