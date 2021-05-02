use structopt::StructOpt;

use crate::utils::prelude::*;

/// Should be implemented by individual subcommand
pub trait Cmd {
    fn run(self) -> Result<()>;
}

/// Dump the current active configuration as toml
#[derive(StructOpt)]
pub struct Config {}

impl Cmd for Config {
    fn run(self) -> Result<()> {
        let config: utils::app_config::DumpableConfig = config().fetch()?;
        println!("{:#?}", config);

        Ok(())
    }
}

/// Run simulation end-to-end
#[derive(StructOpt)]
pub struct Run {}

impl Cmd for Run {
    fn run(self) -> Result<()> {
        infersim::run_sim()
    }
}

/// Step simulation
#[derive(StructOpt)]
pub struct Step {}

impl Cmd for Step {
    fn run(self) -> Result<()> {
        info!("Step");
        todo!()
    }
}
