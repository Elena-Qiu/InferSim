#![feature(generators, generator_trait, backtrace)]
pub mod utils;

use utils::logging::prelude::*;
use utils::Result;

pub fn start() -> Result<()> {
    // does nothing
    info!("Started");

    Ok(())
}

pub fn run_sim() -> Result<()> {
    info!("Running sim");

    Ok(())
}
