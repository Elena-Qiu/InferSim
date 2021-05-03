use anyhow::anyhow;
use std::backtrace::Backtrace;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    InvalidConfig(anyhow::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Logging(anyhow::Error),
    #[error(transparent)]
    Others(#[from] anyhow::Error),
    #[error("lock poisoned: {msg}")]
    Poisoned { msg: String, backtrace: Backtrace },
}

/// A type alias that forces the usage of the custom error type.
pub type Result<T> = std::result::Result<T, Error>;

impl From<tracing::subscriber::SetGlobalDefaultError> for Error {
    fn from(err: tracing::subscriber::SetGlobalDefaultError) -> Self {
        Self::Logging(anyhow::Error::from(err))
    }
}

impl From<tracing_subscriber::util::TryInitError> for Error {
    fn from(err: tracing_subscriber::util::TryInitError) -> Self {
        Self::Logging(anyhow::Error::from(err))
    }
}

impl From<tracing_subscriber::reload::Error> for Error {
    fn from(err: tracing_subscriber::reload::Error) -> Self {
        Self::Logging(anyhow::Error::from(err))
    }
}

impl<T> From<std::sync::PoisonError<T>> for Error {
    fn from(err: std::sync::PoisonError<T>) -> Self {
        Self::Poisoned {
            msg: err.to_string(),
            backtrace: Backtrace::capture(),
        }
    }
}

impl From<config::ConfigError> for Error {
    fn from(err: config::ConfigError) -> Self {
        Self::InvalidConfig(anyhow::Error::from(err))
    }
}

impl Error {
    pub fn adhoc(msg: &'static str) -> Self {
        Self::Others(anyhow!(msg))
    }

    pub fn invalid_config(msg: &'static str) -> Self {
        Self::InvalidConfig(anyhow!(msg))
    }
}
