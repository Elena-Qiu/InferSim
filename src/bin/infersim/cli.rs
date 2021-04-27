use clap::{crate_authors, crate_description, crate_version};
use clap::{App, AppSettings, Arg};

use crate::commands;
use crate::{AppConfig, Result};

/// Match commands
pub fn cli_match() -> Result<()> {
    // Get matches
    let cli_matches = cli_config()?;

    // Merge clap config file if the value is set
    AppConfig::merge_config(cli_matches.value_of("config"))?;

    // Matches Commands or display help
    match cli_matches.subcommand_name() {
        Some("config") => {
            commands::config()?;
        }
        _ => {
            // Arguments are required by default (in Clap)
            // This section should never execute and thus
            // should probably be logged in case it executed.
        }
    }
    Ok(())
}

/// Configure Clap
/// This function will configure clap and match arguments
pub fn cli_config() -> Result<clap::ArgMatches<'static>> {
    let cli_app = App::new("InferSim")
        .setting(AppSettings::ArgRequiredElseHelp)
        .version(crate_version!())
        .about(crate_description!())
        .author(crate_authors!("\n"))
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .help("Set a custom config file")
                .takes_value(true)
                .value_name("FILE"),
        )
        .subcommand(App::new("config").about("Show Configuration"));

    // Get matches
    let cli_matches = cli_app.get_matches();

    Ok(cli_matches)
}
