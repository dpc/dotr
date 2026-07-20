//! Command-line interface for `dotr`.

mod opts;

use std::{io, process};

use anyhow::Context;
use clap::Parser;
use dotr::Dotr;
use tracing_subscriber::{EnvFilter, FmtSubscriber, filter::LevelFilter};

fn init_tracing(verbosity: u8) -> anyhow::Result<()> {
    let level = match verbosity {
        0 => LevelFilter::INFO,
        1 => LevelFilter::DEBUG,
        _ => LevelFilter::TRACE,
    };
    let filter = EnvFilter::builder()
        .with_default_directive(level.into())
        .with_env_var("DOTR_LOG")
        .from_env()
        .context("invalid DOTR_LOG filter")?;

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(filter)
        .with_writer(io::stderr)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    Ok(())
}

fn run() -> anyhow::Result<()> {
    let opts = opts::Options::parse();

    init_tracing(opts.verbose)?;

    let mut dotr = Dotr::new();
    if opts.force {
        dotr = dotr.set_force();
    }
    if opts.dry_run {
        dotr = dotr.set_dry_run();
    }

    match opts.command {
        opts::Command::Link => dotr.link(&opts.src_dir, &opts.dst_dir)?,
        opts::Command::Unlink => dotr.unlink(&opts.src_dir, &opts.dst_dir)?,
    }

    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error}");
        process::exit(-1);
    }
}

#[cfg(test)]
mod tests;
