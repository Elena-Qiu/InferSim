use std::fmt;
use std::path::{Path, PathBuf};

use structopt::clap::AppSettings;
use structopt::StructOpt;

use crate::commands::{self, Cmd};
use crate::utils::logging::GlobalLoggingContext;
use crate::utils::prelude::*;

// usage has to be set statically to force `[preset]` appear at the end
#[derive(StructOpt)]
#[structopt(
    setting = AppSettings::SubcommandRequiredElseHelp,
    setting = AppSettings::UnifiedHelpMessage,
    setting = AppSettings::VersionlessSubcommands,
    global_setting = AppSettings::ColoredHelp,
    usage = "infersim [OPTIONS] <SUBCOMMAND> [preset]"
)]
pub struct CLI {
    /// Set a custom config file, defaults to './config.yml'
    #[structopt(short, long, global = true, value_name = "FILE", parse(from_os_str))]
    config: Option<PathBuf>,
    /// The config preset to load, if missing, the global default is used
    #[structopt(global = true)]
    preset: Option<String>,
    #[structopt(subcommand)]
    cmd: Command,
}

/// A macro to create a enum holding all subcommands
/// and also forwarding the Cmd trait impl to inner.
macro_rules! make_command {
    ($($x:ident),*) => {
        #[derive(StructOpt)]
        enum Command {
            $(
                $x(commands::$x),
            )*
        }

        impl Cmd for Command {
            fn run(self) -> Result<()> {
                match self {
                    $(
                        Command::$x(inner) => {
                            // let _s = info_span!(concat!("cmd:", stringify!($x))).entered();
                            inner.run()
                        },
                    )*
                }
            }

            fn produces_output(&self) -> bool {
                match self {
                    $(
                        Command::$x(inner) => inner.produces_output(),
                    )*
                }
            }
        }

        impl fmt::Display for Command {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    $(
                        Command::$x(_) => write!(f, concat!("Command::", stringify!($x))),
                    )*
                }
            }
        }
    };
}

make_command![Config, Run, Step];

pub fn execute(logging: &mut GlobalLoggingContext) -> Result<()> {
    let cli = CLI::from_args();
    // handle global options
    // load config file
    if let Some(path) = cli.config {
        config_mut().use_file(&path)?;
    } else {
        // automatically use config.yml if there's one
        let path = Path::new("config.yml");
        if path.exists() {
            info!(?path, "Using default config file");
            config_mut().use_file(path)?;
        }
    }
    // load preset
    let preset = if let Some(preset) = &cli.preset {
        config_mut().use_preset(preset)?;
        preset
    } else {
        "default"
    };
    // apply settings from config
    logging.reconfigure(cli.cmd.produces_output())?;
    // render preset name in output_dir
    {
        let mut cfg = config_mut();
        let output_dir: String = cfg.get("output_dir")?;
        let output_dir = output_dir.replace("{preset}", preset);
        cfg.set_once("output_dir", output_dir)?;
    }

    cli.cmd.run()
}
