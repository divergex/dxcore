use std::fmt;

use super::Protocol;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Json(serde_json::Error),
    Polars(polars::error::PolarsError),
    UnsupportedProtocol {
        protocol: Protocol,
        ty: &'static str,
    },
    UnknownType(String),
    TypeMismatch {
        expected: &'static str,
        actual: &'static str,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "i/o error: {e}"),
            Error::Json(e) => write!(f, "json error: {e}"),
            Error::Polars(e) => write!(f, "polars error: {e}"),
            Error::UnsupportedProtocol { protocol, ty } => {
                write!(f, "{ty} does not support protocol {protocol:?}")
            }
            Error::UnknownType(name) => write!(f, "no serializable registered for type {name}"),
            Error::TypeMismatch { expected, actual } => {
                write!(f, "type mismatch: expected {expected}, got {actual}")
            }
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Json(e)
    }
}

impl From<polars::error::PolarsError> for Error {
    fn from(e: polars::error::PolarsError) -> Self {
        Error::Polars(e)
    }
}
