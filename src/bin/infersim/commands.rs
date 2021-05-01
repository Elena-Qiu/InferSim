use structopt::StructOpt;

use crate::utils::{self, prelude::*};

/// Should be implemented by individual subcommand
pub trait Cmd {
    fn run(self) -> Result<()>;
}

/// Show the configuration file
#[derive(StructOpt)]
pub struct Config {
    preset: Option<String>,
}

impl Cmd for Config {
    fn run(self) -> Result<()> {
        if let Some(preset) = self.preset {
            config_mut().use_preset(&preset)?;
        }
        // apply settings from config
        utils::logging::apply_config()?;

        let config: utils::app_config::DumpableConfig = config().fetch()?;
        println!("{:#?}", config);

        Ok(())
    }
}

/// Run simulation end-to-end
#[derive(StructOpt)]
pub struct Run {
    preset: Option<String>,
}

impl Cmd for Run {
    fn run(self) -> Result<()> {
        if let Some(preset) = self.preset {
            config_mut().use_preset(&preset)?;
        }
        // apply settings from config
        utils::logging::apply_config()?;

        infersim::run_sim()
    }
}

/// Step simulation
#[derive(StructOpt)]
pub struct Step {}

impl Cmd for Step {
    fn run(self) -> Result<()> {
        // apply settings from config
        utils::logging::apply_config()?;

        info!("Step");
        todo!()
    }
}
