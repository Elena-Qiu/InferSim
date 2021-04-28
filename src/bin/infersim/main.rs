use infersim::utils::{self, AppConfig, Result};

mod cli;
mod commands;

static DEFAULT_CONFIG: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/resources/default_config.toml"));

fn main() -> Result<()> {
    // panic setup should be done early
    utils::panic::setup();
    // basic logging setup
    let _guard = utils::logging::setup()?;

    // initialize Configuration
    AppConfig::init(Some(DEFAULT_CONFIG))?;

    // Match Commands
    cli::execute()
}
