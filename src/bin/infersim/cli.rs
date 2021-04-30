use std::fmt;
use std::path::PathBuf;

use structopt::clap::AppSettings;
use structopt::StructOpt;

use crate::commands::{self, Cmd};
use crate::utils;
use crate::utils::logging::prelude::*;
use crate::{AppConfig, Result};

#[derive(StructOpt)]
#[structopt(
    setting = AppSettings::SubcommandRequiredElseHelp,
    setting = AppSettings::UnifiedHelpMessage,
    setting = AppSettings::VersionlessSubcommands,
    global_setting = AppSettings::ColoredHelp,
)]
pub struct CLI {
    /// Set a custom config file
    #[structopt(short, long, value_name = "FILE", parse(from_os_str))]
    config: Option<PathBuf>,
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
                            let _s = info_span!(concat!("cmd:", stringify!($x))).entered();
                            inner.run()
                        },
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

pub fn execute() -> Result<()> {
    let cli = CLI::from_args();
    // handle global options
    AppConfig::merge_config(cli.config.as_deref())?;

    // apply settings from config
    utils::logging::apply_config()?;

    cli.cmd.run()
}
