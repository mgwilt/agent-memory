use std::fmt::{Display, Formatter};

pub type MemoryResult<T> = Result<T, MemoryError>;

#[derive(Debug, Clone, PartialEq)]
pub enum MemoryError {
    Validation(String),
    NotFound(String),
    Conflict(String),
    ThresholdMiss { activation: f64, threshold: f64 },
    StoreUnavailable(String),
    Serialization(String),
}

impl Display for MemoryError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation(msg) => write!(f, "validation error: {msg}"),
            Self::NotFound(msg) => write!(f, "not found: {msg}"),
            Self::Conflict(msg) => write!(f, "conflict: {msg}"),
            Self::ThresholdMiss {
                activation,
                threshold,
            } => {
                write!(
                    f,
                    "activation {activation:.6} is below retrieval threshold {threshold:.6}"
                )
            }
            Self::StoreUnavailable(msg) => write!(f, "store unavailable: {msg}"),
            Self::Serialization(msg) => write!(f, "serialization error: {msg}"),
        }
    }
}

impl std::error::Error for MemoryError {}
