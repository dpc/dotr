mod opts;

use std::process;

use clap::Parser;
use dotr::Dotr;
use opts::Options;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

trait DotrExt {
    fn from_opts(opts: Options) -> Self;
}

impl DotrExt for Dotr {
    fn from_opts(opts: Options) -> Self {
        let mut dotr = Dotr::new();

        if opts.force {
            dotr = dotr.set_force();
        }

        if opts.dry_run {
            dotr = dotr.set_dry_run()
        }

        dotr
    }
}

fn init_tracing(verbosity: u8) -> anyhow::Result<()> {
    let level = match verbosity {
        0 => "error",
        1 => "warn",
        2 => "info",
        3 => "debug",
        _ => "trace",
    };

    let subscriber = FmtSubscriber::builder()
        // Use the environment variable, if set, falling back to the specified level if not
        .with_env_filter(EnvFilter::new(
            std::env::var(tracing_subscriber::EnvFilter::DEFAULT_ENV)
                .unwrap_or_else(|_| level.to_string()),
        ))
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    Ok(())
}

fn run() -> anyhow::Result<()> {
    let opts = opts::Options::parse();

    init_tracing(opts.verbose)?;

    let dotr = Dotr::from_opts(opts.clone());

    match opts.command {
        opts::Command::Link => dotr.link(&opts.src_dir, &opts.dst_dir)?,
        opts::Command::Unlink => dotr.unlink(&opts.src_dir, &opts.dst_dir)?,
    }

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        process::exit(-1);
    }
}

#[cfg(test)]
mod tests;
