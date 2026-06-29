pub mod cli;
pub mod error;
pub mod git;
pub mod json;
pub mod spike;

use clap::Parser;
use cli::{Cli, Command, SpikeSubcommand};
use error::AppError;

pub fn run_from<I, T>(args: I) -> Result<String, AppError>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = Cli::parse_from(args);
    match cli.command {
        Command::Spike(spike) => match spike.command {
            SpikeSubcommand::Run(args) => {
                let output = spike::run(&args.repo, &args.fixture)?;
                let json = if args.pretty {
                    serde_json::to_string_pretty(&output)?
                } else {
                    serde_json::to_string(&output)?
                };
                Ok(format!("{json}\n"))
            }
        },
    }
}
