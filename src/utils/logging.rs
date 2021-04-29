use slog::Drain as _;
use slog_scope::GlobalLoggerGuard;

use super::error::Result;
use prelude::*;

pub mod prelude {
    pub use slog::o;
    pub use slog_scope::{crit, debug, error, info, trace, warn};
}

/// Basic setup
pub fn setup() -> Result<GlobalLoggerGuard> {
    // Setup Logging
    let guard = slog_scope::set_global_logger(default_root_logger()?);
    slog_stdlog::init().unwrap();

    Ok(guard)
}

/// Apply settings from config
pub fn apply_config() -> Result<()> {
    warn!("STUB"; "what" => "logging::apply_config");
    Ok(())
}

pub fn default_root_logger() -> Result<slog::Logger> {
    // Create drains
    let drain = default_term_drain().unwrap_or(default_discard()?);

    // Optionally duplicate to syslog
    let logger = if cfg!(syslog) {
        let syslog_drain = default_syslog_drain().unwrap_or(default_discard()?);
        // Merge drains
        let drain = slog::Duplicate(syslog_drain, drain).fuse();
        // Create Logger
        slog::Logger::root(drain, o!())
    } else {
        // Create Logger
        slog::Logger::root(drain.fuse(), o!())
    };

    // Return Logger
    Ok(logger)
}

fn default_discard() -> Result<slog_async::Async> {
    let drain = slog_async::Async::default(slog::Discard);

    Ok(drain)
}

// term drain: Log to Terminal
fn default_term_drain() -> Result<slog_async::Async> {
    let plain = slog_term::PlainSyncDecorator::new(std::io::stdout());
    let term = slog_term::FullFormat::new(plain);

    let drain = slog_async::Async::default(term.build().fuse());

    Ok(drain)
}

/// syslog drain: Log to syslog
#[cfg(syslog)]
fn default_syslog_drain() -> Result<slog_async::Async> {
    let syslog = slog_syslog::unix_3164(slog_syslog::Facility::LOG_USER)?;

    let drain = slog_async::Async::default(syslog.fuse());

    Ok(drain)
}
#[cfg(not(syslog))]
fn default_syslog_drain() -> Result<slog_async::Async> {
    unreachable!()
}
