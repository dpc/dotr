use std::ffi::OsStr;
use std::path::Path;
use std::{fs, io};

use tracing::{debug, info, trace, warn};
use walkdir::WalkDir;

const DOTR_CONFIG_FILE: &str = ".dotr";

#[derive(serde::Deserialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
enum Traverse {
    Link,
}

#[derive(serde::Deserialize, Default)]
struct DirConfig {
    traverse: Option<Traverse>,
}

fn read_dir_config(dir: &Path) -> DirConfig {
    let config_path = dir.join(DOTR_CONFIG_FILE);
    fs::read_to_string(&config_path)
        .ok()
        .and_then(|content| toml::from_str(&content).ok())
        .unwrap_or_default()
}

pub struct Dotr {
    dry_run: bool,
    force: bool,
}

impl Dotr {
    pub fn new() -> Self {
        Dotr {
            dry_run: false,
            force: false,
        }
    }

    pub fn set_force(self) -> Self {
        Self {
            force: true,
            ..self
        }
    }

    pub fn set_dry_run(self) -> Self {
        Self {
            dry_run: true,
            ..self
        }
    }

    fn link_dir(&self, src: &Path, src_base: &Path, dst_base: &Path) -> io::Result<()> {
        let src_rel = src.strip_prefix(src_base).unwrap();
        let dst = dst_base.join(src_rel);

        if dst.exists() || dst.symlink_metadata().is_ok() {
            if self.force {
                if dst
                    .symlink_metadata()
                    .map(|m| m.file_type().is_dir())
                    .unwrap_or(false)
                    && !dst
                        .symlink_metadata()
                        .map(|m| m.file_type().is_symlink())
                        .unwrap_or(false)
                {
                    return Err(io::Error::other(format!(
                        "Can't safely remove {} as it's a real directory",
                        dst.display()
                    )));
                }
                if !self.dry_run {
                    debug!(src = %src.display(), dst = %dst.display(), "Force removing destination for directory link");
                    fs::remove_file(&dst)?;
                }
            } else {
                if dst
                    .symlink_metadata()
                    .map(|m| m.file_type().is_symlink())
                    .unwrap_or(false)
                {
                    let dst_link = dst.read_link()?;
                    if dst_link == src {
                        debug!(src = %src.display(), dst = %dst.display(), "Directory symlink already correct");
                        return Ok(());
                    }
                }
                warn!(src = %src.display(), dst = %dst.display(), "Destination already exists");
                return Ok(());
            }
        } else if !self.dry_run {
            fs::create_dir_all(dst.parent().unwrap())?;
        }

        if !self.dry_run {
            trace!(src = %src.display(), dst = %dst.display(), "Creating symlink to directory");
            std::os::unix::fs::symlink(src, &dst)?;
        }
        Ok(())
    }

    fn unlink_dir(&self, src: &Path, src_base: &Path, dst_base: &Path) -> io::Result<()> {
        let src_rel = src.strip_prefix(src_base).unwrap();
        let dst = dst_base.join(src_rel);

        if dst.symlink_metadata().is_ok() {
            let meta = dst.symlink_metadata()?;
            if meta.file_type().is_symlink() {
                let dst_link = dst.read_link()?;
                if dst_link == src {
                    if !self.dry_run {
                        debug!(src = %src.display(), dst = %dst.display(), "Removing directory symlink");
                        fs::remove_file(&dst)?;
                    }
                } else if self.force {
                    if !self.dry_run {
                        debug!(src = %src.display(), dst = %dst.display(), "Force removing directory symlink");
                        fs::remove_file(&dst)?;
                    }
                } else {
                    warn!(src = %src.display(), dst = %dst.display(), "Directory symlink points elsewhere");
                }
            } else if self.force {
                warn!(src = %src.display(), dst = %dst.display(), "Destination is not a symlink, refusing to remove");
            } else {
                warn!(src = %src.display(), dst = %dst.display(), "Destination exists but is not a symlink");
            }
        } else {
            debug!(src = %src.display(), dst = %dst.display(), "Destination doesn't exist - nothing to unlink");
        }
        Ok(())
    }

    pub fn link_entry(
        &self,
        src: &walkdir::DirEntry,
        src_base: &Path,
        dst_base: &Path,
    ) -> io::Result<()> {
        trace!(path = %src.path().display(), "Walking path");

        let src = src.path();
        let src_rel = src.strip_prefix(src_base).unwrap();

        let dst = dst_base.join(src_rel);
        let dst_metadata = dst.symlink_metadata().ok();
        let dst_type = dst_metadata.map(|m| m.file_type());

        let src_metadata = src.symlink_metadata()?;
        let src_type = src_metadata.file_type();

        if src_type.is_dir() {
            return Ok(());
        } else if src_type.is_file() {
            trace!(src = %src.display(), dst=%dst.display(), "Source is a file");
            if dst.exists() || dst.symlink_metadata().is_ok() {
                if self.force {
                    if dst_type.is_some_and(|t| t.is_dir()) {
                        io::Error::other(format!(
                            "Can't safely remove {} as it's a directory",
                            dst.display()
                        ));
                    }
                    if !self.dry_run {
                        debug!(src = %src.display(), dst=%dst.display(), "Force removing destination");
                        fs::remove_file(&dst)?;
                    } else {
                        debug!(src = %src.display(), dst=%dst.display(), "Force removing destination (dry-run)");
                    }
                } else {
                    if dst_type.map(|t| t.is_symlink()).unwrap_or(false) {
                        let dst_link_dst = dst.read_link()?;
                        if *dst_link_dst == *src {
                            debug!(src = %src.display(), dst=%dst.display(), "Destination already points to the source");
                            return Ok(());
                        } else {
                            warn!(src = %src.display(), dst = %dst.display(), dst_dst = %dst_link_dst.display(), "Destination already exists and points elsewhere");
                        }
                    } else {
                        warn!(src = %src.display(), dst=%dst.display(),  "Destination already exists and is not a symlink");
                    }
                    return Ok(());
                }
            } else if !self.dry_run {
                trace!(src = %src.display(), dst=%dst.display(), "Creating a base directory (if doesn't exist)");
                fs::create_dir_all(dst.parent().unwrap())?;
            }

            if !self.dry_run {
                trace!(src = %src.display(), dst=%dst.display(), "Creating symlink to a src file");
                std::os::unix::fs::symlink(src, &dst)?;
            }
        } else if src_type.is_symlink() {
            let src_link = src.read_link()?;
            trace!(src = %src.display(), dst=%dst.display(), "src-link" = %src_link.display(), "Source is a symlink");
            if dst.exists() || dst.symlink_metadata().is_ok() {
                if self.force {
                    if !self.dry_run {
                        debug!(src = %src.display(), dst = %dst.display(), "Force removing destination");
                        fs::remove_file(&dst)?;
                    } else {
                        debug!(src = %src.display(), dst = %dst.display(), "Force removing destination (dry-run)");
                    }
                } else if Some(src_link.clone()) == dst.read_link().ok() {
                    debug!(
                        src = %src.display(), dst = %dst.display(),
                        "Destination already points to the source (symlink source)"
                    );
                    return Ok(());
                } else {
                    warn!(src = %src.display(), dst = %dst.display(), "Destination already exists");
                    return Ok(());
                }
            } else if !self.dry_run {
                trace!(src = %src.display(), dst = %dst.display(), "Creating a base directory (if doesn't exist)");
                fs::create_dir_all(dst.parent().unwrap())?;
            }
            if !self.dry_run {
                trace!(src = %src.display(), dst = %dst.display(), "src-link" = %src_link.display(), "Duplicating symlink");
                std::os::unix::fs::symlink(&src_link, &dst)?;
            }
        } else {
            warn!(src = %src.display(), dst = %dst.display(), "Skipping unknown source file type");
        }
        Ok(())
    }

    pub fn link(&self, src_base: &Path, dst_base: &Path) -> io::Result<()> {
        info!(src = %src_base.display(), dst = %dst_base.display(), "Starting link operation");

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

        let mut iter = WalkDir::new(&src_base).into_iter();
        while let Some(entry) = iter.next() {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    warn!("Error walking: {}", e);
                    continue;
                }
            };

            // Skip .dotr config files
            if entry.path().file_name() == Some(OsStr::new(DOTR_CONFIG_FILE)) {
                continue;
            }

            if entry.file_type().is_dir() {
                if !should_traverse(&entry) {
                    iter.skip_current_dir();
                    continue;
                }

                // Check .dotr config for non-root directories
                if entry.path() != src_base.as_path() {
                    let config = read_dir_config(entry.path());
                    if config.traverse == Some(Traverse::Link) {
                        debug!(path = %entry.path().display(), "Linking directory per .dotr traverse=link");
                        self.link_dir(entry.path(), &src_base, &dst_base)?;
                        iter.skip_current_dir();
                        continue;
                    }
                }

                continue;
            }

            self.link_entry(&entry, &src_base, &dst_base)?;
        }

        Ok(())
    }

    pub fn unlink(&self, src_base: &Path, dst_base: &Path) -> io::Result<()> {
        info!(src = %src_base.display(), dst = %dst_base.display(), "Starting unlink operation");

        let dst_base = dst_base.canonicalize()?;
        let src_base = src_base.canonicalize()?;

        assert!(dst_base.is_absolute());
        assert!(src_base.is_absolute());

        let mut iter = WalkDir::new(&src_base).into_iter();
        while let Some(entry) = iter.next() {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    warn!("Error walking: {}", e);
                    continue;
                }
            };

            if entry.path().file_name() == Some(OsStr::new(DOTR_CONFIG_FILE)) {
                continue;
            }

            if entry.file_type().is_dir() {
                if !should_traverse(&entry) {
                    iter.skip_current_dir();
                    continue;
                }

                if entry.path() != src_base.as_path() {
                    let config = read_dir_config(entry.path());
                    if config.traverse == Some(Traverse::Link) {
                        debug!(path = %entry.path().display(), "Unlinking directory per .dotr traverse=link");
                        self.unlink_dir(entry.path(), &src_base, &dst_base)?;
                        iter.skip_current_dir();
                        continue;
                    }
                }

                continue;
            }

            self.unlink_entry(&entry, &src_base, &dst_base)?;
        }

        Ok(())
    }

    pub fn unlink_entry(
        &self,
        src: &walkdir::DirEntry,
        src_base: &Path,
        dst_base: &Path,
    ) -> io::Result<()> {
        trace!(path = %src.path().display(), "Walking path");

        let src = src.path();
        let src_rel = src.strip_prefix(src_base).unwrap();

        let dst = dst_base.join(src_rel);

        let src_metadata = src.symlink_metadata()?;
        let src_type = src_metadata.file_type();

        if src_type.is_dir() {
            return Ok(());
        } else if src_type.is_file() {
            trace!(src = %src.display(), dst = %dst.display(), "Unlink a file");
            let dst_metadata = dst.symlink_metadata();
            // exists follows symlinks :/
            if dst.exists() || dst_metadata.is_ok() {
                let dst_metadata = dst_metadata?;
                if self.force {
                    if !self.dry_run {
                        debug!(src = %src.display(), dst = %dst.display(), "Force removing");
                        fs::remove_file(&dst)?;
                        return Ok(());
                    } else {
                        debug!(src = %src.display(), dst = %dst.display(), "Force removing (dry run)");
                    }
                } else if dst_metadata.file_type().is_file() {
                    warn!(src = %src.display(), dst = %dst.display(), "Destination already exists and is a file");
                    return Ok(());
                } else if dst_metadata.file_type().is_dir() {
                    warn!(src = %src.display(), dst = %dst.display(), "Destination already exists and is a directory");
                    return Ok(());
                } else if dst_metadata.file_type().is_symlink() {
                    let dst_link = dst.read_link()?;
                    if dst_link != src {
                        warn!(src = %src.display(), dst = %dst.display(), "Destination already exists and is a symlink pointing to something else");
                        return Ok(());
                    } else if !self.dry_run {
                        fs::remove_file(&dst)?;
                    }
                } else {
                    warn!(src = %src.display(), dst = %dst.display(), "Destination exists and is of unknown file type");
                }
            } else {
                debug!(src = %src.display(), dst = %dst.display(), "Destination doesn't exist - nothing to unlink");
                return Ok(());
            }
        } else if src_type.is_symlink() {
            let src_link = src.read_link()?;
            trace!(src = %src.display(), dst = %dst.display(),  "Unlink a symlink");
            let dst_metadata = dst.symlink_metadata();
            // exists follows symlinks :/
            if dst.exists() || dst_metadata.is_ok() {
                let dst_metadata = dst_metadata?;
                if self.force {
                    if !self.dry_run {
                        fs::remove_file(&dst)?;
                        return Ok(());
                    }
                } else if dst_metadata.file_type().is_file() {
                    warn!(src = %src.display(), dst = %dst.display(),  "Destination already exists and is a file");
                    return Ok(());
                } else if dst_metadata.file_type().is_dir() {
                    warn!(src = %src.display(), dst = %dst.display(),  "Destination already exists and is a directory");
                    return Ok(());
                } else if dst_metadata.file_type().is_symlink() {
                    let dst_link = dst.read_link()?;
                    if dst_link != src_link {
                        warn!(
                            src = %src.display(),
                            dst = %dst.display(),
                            "dst-link" = %dst_link.display(),
                            "src-link" = %src_link.display(),
                            "Destination already exists and is a symlink pointing to something else",
                        );
                        return Ok(());
                    } else if !self.dry_run {
                        fs::remove_file(&dst)?;
                    }
                } else {
                    warn!(src = %src.display(), dst = %dst.display(), "Destination exists and is of unknown file type");
                }
            } else {
                debug!(src = %src.display(), dst = %dst.display(), "Destination doesn't exist - nothing to unlink");
                return Ok(());
            }
        } else {
            warn!(src = %src.display(), dst = %dst.display(), "Skipping unknown source file type");
        }
        Ok(())
    }
}

impl Default for Dotr {
    fn default() -> Self {
        Self::new()
    }
}

fn should_traverse(de: &walkdir::DirEntry) -> bool {
    if !de.path().is_dir() {
        return true;
    }

    if de.path().file_name() == Some(OsStr::new(".git")) {
        return false;
    }

    true
}
