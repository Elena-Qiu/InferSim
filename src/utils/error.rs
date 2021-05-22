use thiserror::Error;

pub use error_kind::Kind;
use error_kind::WithKind;

/// A type alias that defaults to the custom error type.
pub type Result<T, E = Error> = std::result::Result<T, E>;

mod error_kind {
    //! A generic error kind implementation supporting multiple error types
    //! See: https://github.com/dtolnay/anyhow/issues/75#issuecomment-602796794

    /// Custom error type needs to implement `From<WithKind<CustomErrorKind>>`
    pub struct WithKind<K> {
        pub kind: K,
        pub source: anyhow::Error,
    }

    /// Attach a `kind` method to result
    pub trait Kind {
        type Ok;
        fn kind<K>(self, kind: K) -> Result<Self::Ok, WithKind<K>>;
    }

    impl<T, E> Kind for Result<T, E>
    where
        E: Into<anyhow::Error>,
    {
        type Ok = T;
        fn kind<K>(self, kind: K) -> Result<T, WithKind<K>> {
            self.map_err(|e| WithKind { kind, source: e.into() })
        }
    }
}

#[derive(Error, Debug, Clone, Eq, PartialEq)]
pub enum ErrorKind {
    #[error("invalid config")]
    InvalidConfig,
    #[error("logging")]
    Logging,
    #[error("chrome tracing")]
    ChromeTracing,
    #[error("{0}")]
    Others(String),
}

#[derive(Error, Debug)]
#[error("{kind}")]
pub struct Error {
    kind: ErrorKind,
    source: anyhow::Error,
}

impl Error {
    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }
}

impl From<WithKind<ErrorKind>> for Error {
    fn from(w: WithKind<ErrorKind>) -> Self {
        Error {
            kind: w.kind,
            source: w.source,
        }
    }
}

// ====== Pre-defined Conversions ======
macro_rules! impl_from_err {
    ($err:ty, $kind:ident) => {
        impl From<$err> for Error {
            fn from(err: $err) -> Self {
                WithKind {
                    kind: ErrorKind::$kind,
                    source: err.into(),
                }
                .into()
            }
        }
    };
}

// ====== Logging ======

impl_from_err!(tracing::subscriber::SetGlobalDefaultError, Logging);
impl_from_err!(tracing_subscriber::util::TryInitError, Logging);
impl_from_err!(tracing_subscriber::reload::Error, Logging);

// ====== Config ======
impl_from_err!(config::ConfigError, InvalidConfig);
impl_from_err!(rand_distr::NormalError, InvalidConfig);
impl_from_err!(rand_distr::PoissonError, InvalidConfig);
impl_from_err!(rand_distr::ExpError, InvalidConfig);
