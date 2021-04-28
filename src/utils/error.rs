use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    InvalidConfig(#[from] config::ConfigError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Others(#[from] anyhow::Error),
}

/// A type alias that forces the usage of the custom error type.
pub type Result<T> = std::result::Result<T, Error>;
