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
            let presets: serde_yaml::Value = config().get("presets")?;
            serde_yaml::to_writer(std::io::stdout(), &presets).expect("serde_yaml can't format its own value!");
        } else {
            let mut config: serde_yaml::Mapping = config().fetch()?;

            // strip presets table from the output
            config.remove(&"presets".into());

            serde_yaml::to_writer(std::io::stdout(), &config).expect("serde_yaml can't format its own value!");
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
