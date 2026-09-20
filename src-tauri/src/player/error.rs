use libmpv2::Error as LibMpvError;
use std::fmt;
use std::sync::PoisonError;

#[derive(Debug)]
#[allow(dead_code)]
#[allow(clippy::enum_variant_names)]
pub enum Error {
    InitError(String),
    PropertyError(String, i32),
    CommandError(String, i32),
    MutexError(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InitError(msg) => write!(f, "Initialization error: {}", msg),
            Error::PropertyError(msg, code) => write!(f, "Property error ({}): {}", code, msg),
            Error::CommandError(msg, code) => write!(f, "Command error ({}): {}", code, msg),
            Error::MutexError(msg) => write!(f, "Mutex lock error: {}", msg),
        }
    }
}

impl<T> From<PoisonError<T>> for Error {
    fn from(err: PoisonError<T>) -> Self {
        Error::MutexError(format!("Mutex was poisoned: {}", err))
    }
}

impl From<LibMpvError> for Error {
    fn from(err: LibMpvError) -> Self {
        Error::PropertyError(err.to_string(), -1)
    }
}
