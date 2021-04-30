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

impl From<tracing::subscriber::SetGlobalDefaultError> for Error {
    fn from(err: tracing::subscriber::SetGlobalDefaultError) -> Self {
        Self::Others(anyhow::Error::from(err))
    }
}

impl From<tracing_subscriber::util::TryInitError> for Error {
    fn from(err: tracing_subscriber::util::TryInitError) -> Self {
        Self::Others(anyhow::Error::from(err))
    }
}
