use infersim::utils::prelude::*;

mod cli;
mod commands;

fn main() -> Result<()> {
    // panic setup should be done early
    utils::panic::setup();
    // basic logging setup
    let _guard = utils::logging::setup()?;

    // initialize Configuration
    utils::app_config::init()?;

    trace!("Start cli execution");

    // Match Commands
    cli::execute()
}
