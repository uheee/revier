use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "revier-analysis")]
#[command(about = "Revier local analysis spike CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Spike(SpikeCommand),
}

#[derive(Debug, Parser)]
pub struct SpikeCommand {
    #[command(subcommand)]
    pub command: SpikeSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum SpikeSubcommand {
    Run(SpikeRunArgs),
}

#[derive(Debug, Parser)]
pub struct SpikeRunArgs {
    #[arg(long)]
    pub repo: PathBuf,

    #[arg(long)]
    pub fixture: SpikeFixture,

    #[arg(long, default_value = "json")]
    pub format: OutputFormat,

    #[arg(long)]
    pub pretty: bool,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum SpikeFixture {
    Linear,
    EarlyFeatureMerge,
    MergeConflict,
    MultiParentAmbiguous,
    RenameMerge,
    DeletionMerge,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum OutputFormat {
    Json,
}
