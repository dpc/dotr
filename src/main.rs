//! `dotr` is a very simple dotfile manager
//!
//! It supports `link` and `unlink` operations and couple
//! of basic flags like `force`.
//!
//! I wrote it for myself, so it's in Rust and does exactly what I want, so I
//! can fix/customize if I need something. But hey, maybe it also does
//! exactly what you want too!
//!
//! ### Installation:
//!
//! * [Install Rust](https://www.rustup.rs/)
//!
//! ```norust
//! cargo install dotr
//! ```
//!
//! ### Usage:
//!
//! ```norust
//! dotr help
//! ```
//!
//! ### Ignoring files:
//!
//! `dotr` can skip some of the files in the source directory. To configure
//! that, create a file called `dotr.toml` with an `ignore` key set to an array
//! of files to be excluded:
//!
//! ```toml
//! ignore = ["LICENSE", "user.js"]
//! ```
//!
//! The `dotr.toml` file will be loaded, if present, from the source directory.
//!
//! ### TODO:
//!
//! * Make it a separate library + binary

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
