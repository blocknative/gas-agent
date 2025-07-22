use thiserror::Error;

#[derive(Error, Debug)]
pub enum ModelError {
    #[error("Missing required data: {message}")]
    MissingData { message: String },

    #[error("Insufficient data for computation: {message}")]
    InsufficientData { message: String },

    #[error("Invalid data provided: {message}")]
    InvalidData { message: String },

    #[error("Computation failed: {message}")]
    ComputationError { message: String },
}

impl ModelError {
    pub fn missing_data(message: impl Into<String>) -> Self {
        Self::MissingData {
            message: message.into(),
        }
    }

    pub fn insufficient_data(message: impl Into<String>) -> Self {
        Self::InsufficientData {
            message: message.into(),
        }
    }

    pub fn invalid_data(message: impl Into<String>) -> Self {
        Self::InvalidData {
            message: message.into(),
        }
    }

    pub fn computation_error(message: impl Into<String>) -> Self {
        Self::ComputationError {
            message: message.into(),
        }
    }
}
