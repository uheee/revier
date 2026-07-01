pub mod cli;
pub mod commands;
pub mod error;
pub mod git;
pub mod index;
pub mod json;
pub mod spike;

use clap::Parser;
use cli::{Cli, Command, IndexSubcommand, SpikeSubcommand};
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
                serialize_json(&output, args.pretty)
            }
        },
        Command::Index(index) => match index.command {
            IndexSubcommand::Status(args) => commands::index_status::run(args),
            IndexSubcommand::Build(args) => commands::index_build::run(args),
            IndexSubcommand::QueryFiles(args) => commands::query_files::run(args),
        },
    }
}

pub(crate) fn serialize_json<T: serde::Serialize>(
    output: &T,
    pretty: bool,
) -> Result<String, AppError> {
    let json = if pretty {
        serde_json::to_string_pretty(output)?
    } else {
        serde_json::to_string(output)?
    };
    Ok(format!("{json}\n"))
}
