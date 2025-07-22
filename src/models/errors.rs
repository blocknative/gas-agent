use thiserror::Error;

#[derive(Error, Debug)]
pub enum ModelError {
    #[error("Missing required data: {message}")]
    MissingData { message: String },

    #[error("Insufficient data for computation: {message}")]
    InsufficientData { message: String },
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
}
