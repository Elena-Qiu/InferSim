use infersim::utils::prelude::*;

mod cli;
mod commands;

fn main() -> Result<()> {
    // panic setup should be done early
    utils::panic::setup();
    // basic logging setup
    let mut logging = utils::logging::setup()?;

    // initialize configuration store
    utils::app_config::setup()?;

    // run cli
    cli::execute(&mut logging)
}
