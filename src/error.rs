use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum NightError {
    TaskExecutionError(String),
    QueueOperationError(String),
    TaskNotFound(String),
    SerializationError(String),
    DeserializationError(String),
    PersistenceError(String),
    NetworkError(String),
    DistributedError(String),
}

impl fmt::Display for NightError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            NightError::TaskExecutionError(msg) => write!(f, "Task execution error: {}", msg),
            NightError::QueueOperationError(msg) => write!(f, "Queue operation error: {}", msg),
            NightError::TaskNotFound(msg) => write!(f, "Task not found: {}", msg),
            NightError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            NightError::DeserializationError(msg) => write!(f, "Deserialization error: {}", msg),
            NightError::PersistenceError(msg) => write!(f, "Persistence error: {}", msg),
            NightError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            NightError::DistributedError(msg) => write!(f, "Distributed error: {}", msg),
        }
    }
}

impl Error for NightError {}

pub type NightResult<T> = Result<T, NightError>;
