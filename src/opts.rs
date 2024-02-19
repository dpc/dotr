use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Copy, Clone, Subcommand)]
pub enum Command {
    Link,
    Unlink,
}

#[derive(Parser, Debug, Clone)]
#[command(version, about)]
pub struct Options {
    #[arg(long)]
    pub dst_dir: PathBuf,
    #[arg(long, default_value = ".")]
    pub src_dir: PathBuf,
    #[command(subcommand)]
    pub command: Command,
    /// Dry Run
    #[arg(long)]
    pub dry_run: bool,
    /// Force file deletion/overwritting
    #[arg(long)]
    pub force: bool,

    /// Paths to ignore
    #[arg(long)]
    pub ignore: Vec<PathBuf>,

    #[clap(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
}
