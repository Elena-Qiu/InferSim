use tracing::level_filters::LevelFilter;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::util::SubscriberInitExt;

use super::error::Result;
use prelude::*;

pub mod prelude {
    pub use tracing::{debug, error, info, trace, warn};
    pub use tracing::{debug_span, error_span, info_span, trace_span, warn_span};
    pub use tracing::{event, field::Empty, instrument, span};
}

pub struct GlobalLoggingGuard {
    #[allow(dead_code)]
    worker_guard: WorkerGuard,
}

/// Basic setup
pub fn setup() -> Result<GlobalLoggingGuard> {
    // Setup Logging
    let (non_blocking, guard) = tracing_appender::non_blocking::NonBlockingBuilder::default()
        .lossy(false)
        .finish(std::io::stdout());
    let filter = tracing_subscriber::EnvFilter::from_default_env()
        // base level if not matched by any directive in env var
        .add_directive(LevelFilter::INFO.into());
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(non_blocking)
        .with_ansi(true)
        .with_target(false) // don't disable event's target
        .with_span_events(FmtSpan::ENTER)
        .finish()
        .try_init()?;

    Ok(GlobalLoggingGuard { worker_guard: guard })
}

/// Apply settings from config
pub fn apply_config() -> Result<()> {
    warn!("STUB");
    Ok(())
}
