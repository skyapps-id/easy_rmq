use thiserror::Error;

#[derive(Error, Debug)]
pub enum AmqpError {
    #[error("Connection error: {0}")]
    ConnectionError(#[from] lapin::Error),

    #[error("Pool error: {0}")]
    PoolError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Channel error: {0}")]
    ChannelError(String),
}

pub type Result<T> = std::result::Result<T, AmqpError>;
