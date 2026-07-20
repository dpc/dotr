use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Subcommand)]
pub enum Command {
    Link,
    Unlink,
}

#[derive(Parser)]
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
    /// Replace or delete existing non-directory destinations
    #[arg(long)]
    pub force: bool,

    /// Increase logging verbosity when `DOTR_LOG` is unset or empty
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
}
