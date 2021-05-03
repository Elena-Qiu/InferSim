use structopt::StructOpt;

use crate::utils::prelude::*;

/// Should be implemented by individual subcommand
pub trait Cmd {
    fn run(self) -> Result<()>;

    /// Whether this command will produce output on stdout
    fn produces_output(&self) -> bool {
        false
    }
}

/// Dump the current active configuration as toml
#[derive(StructOpt)]
pub struct Config {
    /// Show available presets instead
    #[structopt(long, short)]
    presets: bool,
}

impl Cmd for Config {
    fn run(self) -> Result<()> {
        if self.presets {
            let presets: toml::Value = config().get("presets")?;
            println!(
                "{}",
                toml::to_string_pretty(&presets).expect("toml can't format its own value!")
            );
        } else {
            let mut config: toml::Value = config().fetch()?;

            // strip presets table from the output
            {
                let table = config
                    .as_table_mut()
                    .ok_or_else(|| Error::invalid_config("expect a table as the top level"))?;
                table.remove("presets");
            }
            println!(
                "{}",
                toml::to_string_pretty(&config).expect("toml can't format its own value!")
            );
        };
        Ok(())
    }

    fn produces_output(&self) -> bool {
        true
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
