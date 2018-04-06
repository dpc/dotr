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
//! ### TODO:
//!
//! * Make it a separate library + binary

#[macro_use]
extern crate clap;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate slog;
extern crate slog_async;
extern crate slog_term;
extern crate toml;
extern crate walkdir;

use slog::Drain;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::{env, process};
use walkdir::WalkDir;

fn create_logger(verbosity: Option<u32>) -> slog::Logger {
    match verbosity {
        None => slog::Logger::root(slog::Discard, o!()),
        Some(v) => {
            let level = match v {
                0 => slog::Level::Warning,
                1 => slog::Level::Info,
                2 => slog::Level::Debug,
                _ => slog::Level::Trace,
            };
            let drain = slog_term::term_compact();
            let drain = std::sync::Mutex::new(drain);
            let drain = slog::LevelFilter(drain, level);
            slog::Logger::root(drain.fuse(), o!())
        }
    }
}

fn should_traverse(de: &walkdir::DirEntry) -> bool {
    if !de.path().is_dir() {
        return true;
    }

    if de.path().file_name().and_then(|s| s.to_str()) == Some(".git") {
        return false;
    }

    true
}

struct Dotr {
    force: bool,
    dry_run: bool,
    ignore: HashSet<PathBuf>,
    log: slog::Logger,
}

impl Dotr {
    fn new() -> Self {
        Dotr {
            force: false,
            dry_run: false,
            ignore: HashSet::new(),
            log: slog::Logger::root(slog::Discard, o!()),
        }
    }

    fn set_dry_run(&mut self) -> &mut Self {
        self.dry_run = true;
        self
    }

    fn set_force(&mut self) -> &mut Self {
        self.force = true;
        self
    }

    fn set_ignore(&mut self, ignore: HashSet<PathBuf>) -> &mut Self {
        self.ignore = ignore;
        self
    }

    fn set_log(&mut self, log: slog::Logger) -> &mut Self {
        self.log = log;
        self
    }

    fn link(&self, src_base: &Path, dst_base: &Path) -> io::Result<()> {
        info!(self.log, "Starting link operation"; "src" => src_base.display(), "dst" => dst_base.display());

        if !dst_base.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Destination doesn't exist",
            ));
        }

        if !dst_base.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Destination is not a directory",
            ));
        }

        let dst_base = dst_base.canonicalize()?;
        let src_base = src_base.canonicalize()?;

        assert!(dst_base.is_absolute());
        assert!(src_base.is_absolute());

        for src in WalkDir::new(&src_base)
            .into_iter()
            .filter_entry(should_traverse)
            .filter_map(|e| e.ok())
        {
            trace!(self.log, "Walking path"; "path" => src.path().display());

            let src = src.path();
            let src_rel = src.strip_prefix(&src_base).unwrap();

            if self.ignore.contains(src_rel) {
                debug!(self.log, "Ignoring file"; "path" => src.display());
                continue;
            }

            let dst = dst_base.join(src_rel);
            let dst_metadata = dst.symlink_metadata().ok();
            let dst_type = dst_metadata.map(|m| m.file_type());

            let src_metadata = src.symlink_metadata()?;
            let src_type = src_metadata.file_type();

            let log = self.log.new(
                o!("src" => format!("{}", src.display()), "dst" => format!("{}", dst.display())),
            );

            if src_type.is_dir() {
                continue;
            } else if src_type.is_file() {
                trace!(log, "Source is a file"; );
                if dst.exists() || dst.symlink_metadata().is_ok() {
                    if self.force {
                        if !self.dry_run {
                            debug!(log, "Force removing destination");
                            fs::remove_file(&dst)?;
                        } else {
                            debug!(log, "Force removing destination (dry-run)");
                        }
                    } else {
                        if dst_type.map(|t| t.is_symlink()).unwrap_or(false) {
                            let dst_link_dst = dst.read_link()?;
                            if *dst_link_dst == *src {
                                debug!(log, "Destination already points to the source");
                                continue;
                            } else {
                                warn!(log, "Destination already exists and points elsewhere";
                                      "dst_dst" => %dst_link_dst.display());
                            }
                        } else {
                            warn!(log, "Destination already exists and is not a symlink");
                        }
                        continue;
                    }
                } else if !self.dry_run {
                    trace!(log, "Creating a base directory (if doesn't exist)");
                    fs::create_dir_all(dst.parent().unwrap())?;
                }

                if !self.dry_run {
                    trace!(log, "Creating symlink to a src file");
                    std::os::unix::fs::symlink(&src, &dst)?;
                }
            } else if src_type.is_symlink() {
                let src_link = src.read_link()?;
                trace!(log, "Source is a symlink"; "src-link" => &src_link.display());
                if dst.exists() || dst.symlink_metadata().is_ok() {
                    if self.force {
                        if !self.dry_run {
                            debug!(log, "Force removing destination");
                            fs::remove_file(&dst)?;
                        } else {
                            debug!(log, "Force removing destination (dry-run)");
                        }
                    } else if Some(src_link.clone()) == dst.read_link().ok() {
                        debug!(
                            log,
                            "Destination already points to the source (symlink source)"
                        );
                        continue;
                    } else {
                        warn!(log, "Destination already exists");
                        continue;
                    }
                } else if !self.dry_run {
                    trace!(log, "Creating a base directory (if doesn't exist)");
                    fs::create_dir_all(dst.parent().unwrap())?;
                }
                if !self.dry_run {
                    trace!(log, "Duplicating symlink"; "src-link" => src_link.display());
                    std::os::unix::fs::symlink(&src_link, &dst)?;
                }
            } else {
                warn!(log, "Skipping unknown source file type");
            }
        }

        Ok(())
    }
    fn unlink(&self, src_base: &Path, dst_base: &Path) -> io::Result<()> {
        info!(self.log, "Starting unlink operation"; "src" => src_base.display(), "dst" => dst_base.display());

        let dst_base = dst_base.canonicalize()?;
        let src_base = src_base.canonicalize()?;

        assert!(dst_base.is_absolute());
        assert!(src_base.is_absolute());

        for src in WalkDir::new(&src_base)
            .into_iter()
            .filter_entry(should_traverse)
            .filter_map(|e| e.ok())
        {
            trace!(self.log, "Walking path"; "path" => src.path().display());

            let src = src.path();
            let src_rel = src.strip_prefix(&src_base).unwrap();

            if self.ignore.contains(src_rel) {
                debug!(self.log, "Ignoring file"; "path" => src.display());
                continue;
            }

            let dst = dst_base.join(src_rel);

            let src_metadata = src.symlink_metadata()?;
            let src_type = src_metadata.file_type();

            let log = self.log.new(
                o!("src" => format!("{}", src.display()), "dst" => format!("{}", dst.display())),
            );
            if src_type.is_dir() {
                continue;
            } else if src_type.is_file() {
                trace!(log, "Unlink a file");
                let dst_metadata = dst.symlink_metadata();
                // exists follows symlinks :/
                if dst.exists() || dst_metadata.is_ok() {
                    let dst_metadata = dst_metadata?;
                    if self.force {
                        if !self.dry_run {
                            debug!(log, "Force removing");
                            fs::remove_file(&dst)?;
                            continue;
                        } else {
                            debug!(log, "Force removing (dry run)");
                        }
                    } else if dst_metadata.file_type().is_file() {
                        warn!(log, "Destination already exists and is a file");
                        continue;
                    } else if dst_metadata.file_type().is_dir() {
                        warn!(log, "Destination already exists and is a directory");
                        continue;
                    } else if dst_metadata.file_type().is_symlink() {
                        let dst_link = dst.read_link()?;
                        if dst_link != src {
                            warn!(
                                    log,
                                    "Destination already exists and is a symlink pointing to something else"
                                );
                            continue;
                        } else if !self.dry_run {
                            fs::remove_file(&dst)?;
                        }
                    } else {
                        warn!(log, "Destination exists and is of unknown file type");
                    }
                } else {
                    debug!(log, "Destination doesn't exist - nothing to unlink");
                    continue;
                }
            } else if src_type.is_symlink() {
                let src_link = src.read_link()?;
                trace!(log, "Unlink a symlink");
                let dst_metadata = dst.symlink_metadata();
                // exists follows symlinks :/
                if dst.exists() || dst_metadata.is_ok() {
                    let dst_metadata = dst_metadata?;
                    if self.force {
                        if !self.dry_run {
                            fs::remove_file(&dst)?;
                            continue;
                        }
                    } else if dst_metadata.file_type().is_file() {
                        warn!(log, "Destination already exists and is a file");
                        continue;
                    } else if dst_metadata.file_type().is_dir() {
                        warn!(log, "Destination already exists and is a directory");
                        continue;
                    } else if dst_metadata.file_type().is_symlink() {
                        let dst_link = dst.read_link()?;
                        if dst_link != src_link {
                            warn!(log,
                                      "Destination already exists and is a symlink pointing to something else";
                                      "dst-link" => dst_link.display(),
                                      "src-link" => src_link.display(),
                                      );
                            continue;
                        } else if !self.dry_run {
                            fs::remove_file(&dst)?;
                        }
                    } else {
                        warn!(log, "Destination exists and is of unknown file type");
                    }
                } else {
                    debug!(log, "Destination doesn't exist - nothing to unlink");
                    continue;
                }
            } else {
                warn!(log, "Skipping unknown source file type");
            }
        }

        Ok(())
    }
}
#[derive(Copy, Clone)]
enum Command {
    Link,
    Unlink,
}

#[derive(Clone)]
struct Options {
    dst_dir: PathBuf,
    src_dir: PathBuf,
    command: Command,
    log: slog::Logger,
    dry_run: bool,
    force: bool,
    ignore: HashSet<PathBuf>,
}

#[derive(Debug, Default, Clone, Deserialize)]
struct SourceOptions {
    ignore: Option<HashSet<PathBuf>>,
}

impl Options {
    fn from_clap() -> io::Result<Options> {
        let mut dst_dir: Option<PathBuf> = None;
        let mut src_dir: PathBuf = PathBuf::from(".");
        let command;
        let mut dry_run = false;
        let mut force = false;
        //let mut command : Option<Command> = None;

        let matches = clap_app!(
            dotr =>
            (version: env!("CARGO_PKG_VERSION"))
            (author: "Dawid Ciężarkiewicz <dpc@dpc.pw>")
            (about: "Simple dotfile manager")
            (@arg DST_DIR: -d --dst +takes_value "Path to destination. Default: $HOME")
            (@arg SRC_DIR: -s --src +takes_value "Path to source. Default: .")
            (@arg VERBOSE: -v ... "Increase debugging level")
            (@arg DRY_RUN: --dry... "Dry run")
            (@arg FORCE: --force ... "Force overwrite/delete")
            (@subcommand link =>
             (about: "Link to files from SRC_DIR in DST_DIR")
            )
            (@subcommand unlink =>
             (about: "Remove links created by `link`")
            )
            ).setting(clap::AppSettings::SubcommandRequiredElseHelp)
            .get_matches();

        if let Some(dir) = matches.value_of_os("DST_DIR") {
            dst_dir = Some(dir.into());
        }

        if let Some(dir) = matches.value_of_os("SRC_DIR") {
            src_dir = Path::new(&dir).into();
        }

        if matches.is_present("DRY_RUN") {
            dry_run = true;
        }

        if matches.is_present("FORCE") {
            force = true;
        }

        let log = create_logger(Some(matches.occurrences_of("VERBOSE") as u32));

        match matches.subcommand() {
            ("link", _) => {
                command = Some(Command::Link);
            }
            ("unlink", _) => {
                command = Some(Command::Unlink);
            }
            _ => panic!("Unrecognized subcommand"),
        }

        let dst_dir = if let Some(dir) = dst_dir {
            dir
        } else if let Some(home) = env::var_os("HOME") {
            Path::new(&home).into()
        } else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "$HOME not set"));
        };

        let config_file_name = PathBuf::from("dotr.toml");
        let config_file = src_dir.join(&config_file_name);
        let mut ignore = HashSet::new();
        if config_file.exists() {
            let mut file = File::open(&config_file)?;
            let mut string = String::new();
            file.read_to_string(&mut string)?;
            ignore.insert(config_file_name);
            match toml::from_str::<SourceOptions>(&string) {
                Ok(options) => {
                    ignore.extend(options.ignore.into_iter().flat_map(|x| x));
                }
                Err(e) => {
                    error!(log, "Unable to parse config file"; "path" => config_file.display(), "error" => %e);
                }
            }
        }

        Ok(Options {
            dst_dir,
            src_dir,
            command: command.unwrap(),
            dry_run,
            force,
            ignore,
            log,
        })
    }
}

fn run() -> io::Result<()> {
    let options = Options::from_clap()?;

    let mut dotr = Dotr::new();

    dotr.set_log(options.log);

    if options.dry_run {
        dotr.set_dry_run();
    }

    if options.force {
        dotr.set_force();
    }

    dotr.set_ignore(options.ignore);

    match options.command {
        Command::Link => dotr.link(&options.src_dir, &options.dst_dir)?,
        Command::Unlink => dotr.unlink(&options.src_dir, &options.dst_dir)?,
    }

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        process::exit(-1);
    }
}
