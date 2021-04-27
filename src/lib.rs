#![feature(generators, generator_trait, backtrace)]
pub mod utils;

use utils::Result;

pub fn start() -> Result<()> {
    // does nothing
    log::info!("Started");

    Ok(())
}
