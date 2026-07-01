use clap::{Args, Parser, Subcommand, ValueEnum};
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
    Index(IndexCommand),
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
pub struct IndexCommand {
    #[command(subcommand)]
    pub command: IndexSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum IndexSubcommand {
    Status(IndexStatusArgs),
    Build(IndexBuildArgs),
    QueryFiles(QueryFilesArgs),
}

#[derive(Debug, Args)]
pub struct IndexCommonArgs {
    #[arg(long)]
    pub repo: PathBuf,

    #[arg(long)]
    pub db: Option<PathBuf>,

    #[arg(long, default_value = "json")]
    pub format: OutputFormat,

    #[arg(long)]
    pub pretty: bool,
}

#[derive(Debug, Parser)]
pub struct IndexStatusArgs {
    #[command(flatten)]
    pub common: IndexCommonArgs,
}

#[derive(Debug, Parser)]
pub struct IndexBuildArgs {
    #[command(flatten)]
    pub common: IndexCommonArgs,

    #[arg(long)]
    pub branch: String,
}

#[derive(Debug, Parser)]
pub struct QueryFilesArgs {
    #[command(flatten)]
    pub common: IndexCommonArgs,

    #[arg(long)]
    pub base: String,

    #[arg(long)]
    pub head: String,

    #[arg(long)]
    pub branch: String,

    #[arg(long = "author")]
    pub authors: Vec<String>,

    #[arg(long = "author-query")]
    pub author_query: Option<String>,

    #[arg(long = "message")]
    pub message: Option<String>,

    #[arg(long = "since")]
    pub since: Option<String>,

    #[arg(long = "until")]
    pub until: Option<String>,

    #[arg(long = "glob")]
    pub globs: Vec<String>,
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
