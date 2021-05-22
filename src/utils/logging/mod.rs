use std::fmt;

use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_subscriber::fmt::{format::FmtSpan, Layer as FmtLayer};
use tracing_subscriber::{prelude::*, registry::Registry, reload, EnvFilter};

mod combined;

use super::app_config::config;
use super::error::Result;
use std::fmt::Write;
use std::path::PathBuf;
use tracing_appender::rolling::Rotation;
use tracing_subscriber::fmt::time::FormatTime;

pub mod prelude {
    pub use tracing::{debug, error, info, trace, warn};
    pub use tracing::{debug_span, error_span, info_span, trace_span, warn_span};
    pub use tracing::{event, field::Empty, instrument, span};
}

pub fn setup() -> Result<GlobalLoggingContext> {
    GlobalLoggingContext::new()
}

/// This needs to be hold in main
pub struct GlobalLoggingContext {
    worker_guards: Vec<WorkerGuard>,
    reload_handle: reload::Handle<combined::Layer<Registry>, Registry>,
}

impl GlobalLoggingContext {
    /// Basic setup
    pub fn new() -> Result<Self> {
        let (layer, handle) = reload::Layer::new(combined::Layer::empty());
        let s = Registry::default().with(layer);
        s.try_init()?;

        let mut ctx = GlobalLoggingContext {
            worker_guards: vec![],
            reload_handle: handle,
        };

        ctx.reconfigure_with(Default::default(), false)?;

        Ok(ctx)
    }

    pub fn reconfigure(&mut self, produces_output: bool) -> Result<()> {
        let cfg: LoggingConfig = config().get("logging")?;
        self.reconfigure_with(cfg, produces_output)
    }

    fn reconfigure_with(&mut self, cfg: LoggingConfig, produces_output: bool) -> Result<()> {
        let layers: Result<Vec<_>> = cfg
            .outputs
            .iter()
            .map(|output| self.new_layer(&output, &cfg.filter, produces_output))
            .collect();
        let layers = layers?.into_iter().flatten();

        let layer = combined::Layer::new(layers);
        self.reload_handle.reload(layer)?;

        Ok(())
    }

    fn new_layer(
        &mut self,
        output: &LoggingOutput,
        global_filter: &FilterConfig,
        produces_output: bool,
    ) -> Result<Option<combined::Layer<Registry>>> {
        if !output.enabled {
            return Ok(None);
        }

        // prepare span events
        let span_events = output
            .span_events
            .iter()
            .fold(FmtSpan::NONE, |f, e| f | (*e).into());

        // prepare a writer as specified in the config
        let (writer, guard) = output.target.to_writer(produces_output);
        self.worker_guards.push(guard);

        // combine a filtering and a formatting layer
        let mut layers = combined::Layer::empty();
        layers.add(
            output
                .filter
                .with_default(global_filter)
                .to_env_filter(),
        );
        layers.add(
            FmtLayer::default()
                .with_ansi(output.target.supports_color())
                .with_target(false)
                .with_span_events(span_events)
                .with_timer(ISOTimeFormat)
                .with_writer(writer),
        );

        Ok(Some(layers))
    }
}

struct ISOTimeFormat;

impl FormatTime for ISOTimeFormat {
    fn format_time(&self, w: &mut dyn Write) -> fmt::Result {
        write!(w, "{}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"))
    }
}

// ====== Config to Layer ======

impl FilterConfig {
    pub fn to_env_filter(&self) -> EnvFilter {
        let filter = match &self.from_env {
            Some(env) => EnvFilter::from_env(env),
            None => EnvFilter::default(),
        };

        if let Some(dirs) = &self.directives {
            dirs.split(',')
                .filter_map(|s| match s.parse() {
                    Ok(d) => Some(d),
                    Err(err) => {
                        eprintln!("ignoring `{}`: {}", s, err);
                        None
                    }
                })
                .fold(filter, |f, dir| f.add_directive(dir))
        } else {
            filter
        }
    }

    pub fn with_default(&self, default: &FilterConfig) -> FilterConfig {
        Self {
            directives: self
                .directives
                .clone()
                .or_else(|| default.directives.clone()),
            from_env: self
                .directives
                .clone()
                .or_else(|| default.directives.clone()),
        }
    }
}

impl LoggingTarget {
    pub fn supports_color(&self) -> bool {
        match self {
            LoggingTarget::Term(_) => true,
            LoggingTarget::File(_) => false,
        }
    }

    pub fn to_writer(&self, produces_output: bool) -> (NonBlocking, WorkerGuard) {
        match self {
            LoggingTarget::Term(term) => match term.name {
                TermTarget::Stdout if !term.auto_switch || produces_output => {
                    tracing_appender::non_blocking::NonBlockingBuilder::default()
                        .lossy(false)
                        .finish(std::io::stdout())
                }
                _ => tracing_appender::non_blocking::NonBlockingBuilder::default()
                    .lossy(false)
                    .finish(std::io::stderr()),
            },
            LoggingTarget::File(file) => tracing_appender::non_blocking::NonBlockingBuilder::default()
                .lossy(false)
                .finish(tracing_appender::rolling::RollingFileAppender::new(
                    Rotation::NEVER,
                    &file.directory,
                    &file.name,
                )),
        }
    }
}

impl From<SpanEvent> for FmtSpan {
    fn from(e: SpanEvent) -> Self {
        match e {
            SpanEvent::New => FmtSpan::NEW,
            SpanEvent::Enter => FmtSpan::ENTER,
            SpanEvent::Exit => FmtSpan::EXIT,
            SpanEvent::Close => FmtSpan::CLOSE,
            SpanEvent::Active => FmtSpan::ACTIVE,
            SpanEvent::Full => FmtSpan::FULL,
        }
    }
}

// ====== Logging Config ======

#[derive(Debug, serde::Deserialize)]
struct LoggingConfig {
    filter: FilterConfig,
    #[serde(default)]
    outputs: Vec<LoggingOutput>,
}

#[derive(Debug, serde::Deserialize)]
struct FilterConfig {
    #[serde(default)]
    directives: Option<String>,
    #[serde(deserialize_with = "deserialize_filter_from_env")]
    from_env: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct LoggingOutput {
    enabled: bool,
    #[serde(default)]
    span_events: Vec<SpanEvent>,
    #[serde(default = "FilterConfig::empty")]
    filter: FilterConfig,
    target: LoggingTarget,
}

#[derive(Copy, Clone, Debug, serde::Deserialize)]
enum SpanEvent {
    New,
    Enter,
    Exit,
    Close,
    Active,
    Full,
}

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
enum LoggingTarget {
    Term(TermOutput),
    File(FileOutput),
}

#[derive(Debug, serde::Deserialize)]
struct TermOutput {
    name: TermTarget,
    #[serde(default)]
    auto_switch: bool,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum TermTarget {
    Stdout,
    Stderr,
}

#[derive(Debug, serde::Deserialize)]
struct FileOutput {
    directory: PathBuf,
    name: PathBuf,
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            directives: Some("INFO".into()),
            from_env: Some("RUST_LOG".into()),
        }
    }
}

impl FilterConfig {
    pub fn empty() -> Self {
        Self {
            directives: None,
            from_env: None,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            filter: Default::default(),
            outputs: vec![LoggingOutput {
                enabled: true,
                span_events: vec![],
                filter: FilterConfig::empty(),
                target: LoggingTarget::Term(TermOutput {
                    name: TermTarget::Stdout,
                    auto_switch: true,
                }),
            }],
        }
    }
}

// ====== serde helpers ======

/// Deserialize `false` to `None`, `true` to `Some("RUST_LOG")`, and string to `Some(xxx)`
fn deserialize_filter_from_env<'de, D>(deserializer: D) -> std::result::Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct VisitFromEnv;

    impl<'de> serde::de::Visitor<'de> for VisitFromEnv {
        type Value = Option<String>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("string or map")
        }

        fn visit_bool<E>(self, value: bool) -> std::result::Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            if value {
                Ok(Some("RUST_LOG".into()))
            } else {
                Ok(None)
            }
        }

        fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Some(value.to_owned()))
        }
    }

    deserializer.deserialize_any(VisitFromEnv)
}
