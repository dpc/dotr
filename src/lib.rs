//! Filesystem operations for linking and unlinking a dotfile tree.

use std::{
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
};

use tracing::{debug, info, trace, warn};
use walkdir::{DirEntry, WalkDir};

const DOTR_CONFIG_FILE: &str = ".dotr";

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum Traverse {
    Link,
}

#[derive(Default, serde::Deserialize)]
struct DirConfig {
    traverse: Option<Traverse>,
}

#[derive(Clone, Copy)]
enum Operation {
    Link,
    Unlink,
}

fn read_dir_config(dir: &Path) -> DirConfig {
    let config_path = dir.join(DOTR_CONFIG_FILE);
    fs::read_to_string(&config_path)
        .ok()
        .and_then(|content| toml::from_str(&content).ok())
        .unwrap_or_default()
}

/// Configures and runs dotfile link and unlink operations.
///
/// `Dotr` trusts its paths and does not confine mutations beneath the
/// destination base. Callers must provide non-overlapping bases in quiescent,
/// trusted directory trees. See the repository's `SECURITY.md` for the complete
/// safety model.
#[derive(Debug, Default)]
#[must_use]
pub struct Dotr {
    /// Whether operations should report actions without modifying the filesystem.
    dry_run: bool,
    /// Whether operations may replace destinations.
    force: bool,
}

impl Dotr {
    /// Creates an instance with force and dry-run modes disabled.
    pub const fn new() -> Self {
        Self {
            dry_run: false,
            force: false,
        }
    }

    /// Enables destructive handling of existing destinations.
    ///
    /// Force mode may remove entries that `dotr` did not create. It does not
    /// make overlapping or untrusted directory trees safe.
    pub const fn set_force(self) -> Self {
        Self {
            force: true,
            ..self
        }
    }

    /// Enables dry-run mode, which suppresses filesystem mutations.
    pub const fn set_dry_run(self) -> Self {
        Self {
            dry_run: true,
            ..self
        }
    }

    fn link_dir(&self, src: &Path, src_base: &Path, dst_base: &Path) -> io::Result<()> {
        let dst = destination_path(src, src_base, dst_base);

        if dst.exists() || dst.symlink_metadata().is_ok() {
            if self.force {
                if dst
                    .symlink_metadata()
                    .is_ok_and(|metadata| metadata.file_type().is_dir())
                    && !dst
                        .symlink_metadata()
                        .is_ok_and(|metadata| metadata.file_type().is_symlink())
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
                    .is_ok_and(|metadata| metadata.file_type().is_symlink())
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
        let dst = destination_path(src, src_base, dst_base);

        if dst.symlink_metadata().is_ok() {
            let dst_metadata = dst.symlink_metadata()?;
            if dst_metadata.file_type().is_symlink() {
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

    /// Links one source entry into the destination tree.
    ///
    /// Directory entries are ignored because [`Self::link`] handles traversal.
    ///
    /// # Errors
    ///
    /// Returns an error when source inspection or a required filesystem mutation
    /// fails.
    ///
    /// # Panics
    ///
    /// Panics when `src` is not below `src_base`.
    pub fn link_entry(&self, src: &DirEntry, src_base: &Path, dst_base: &Path) -> io::Result<()> {
        trace!(path = %src.path().display(), "Walking path");

        let src = src.path();
        let dst = destination_path(src, src_base, dst_base);
        let dst_metadata = dst.symlink_metadata().ok();

        let src_metadata = src.symlink_metadata()?;
        let src_type = src_metadata.file_type();

        if src_type.is_dir() {
            return Ok(());
        }

        if src_type.is_file() {
            return self.link_file(src, &dst, dst_metadata);
        }

        if src_type.is_symlink() {
            return self.link_symlink(src, &dst);
        }

        warn!(src = %src.display(), dst = %dst.display(), "Skipping unknown source file type");
        Ok(())
    }

    fn link_file(
        &self,
        src: &Path,
        dst: &Path,
        dst_metadata: Option<fs::Metadata>,
    ) -> io::Result<()> {
        trace!(src = %src.display(), dst=%dst.display(), "Source is a file");
        if dst.exists() || dst.symlink_metadata().is_ok() {
            if self.force {
                if self.dry_run {
                    debug!(src = %src.display(), dst=%dst.display(), "Force removing destination (dry-run)");
                } else {
                    debug!(src = %src.display(), dst=%dst.display(), "Force removing destination");
                    fs::remove_file(dst)?;
                }
            } else {
                if dst_metadata.is_some_and(|metadata| metadata.file_type().is_symlink()) {
                    let dst_link = dst.read_link()?;
                    if dst_link == src {
                        debug!(src = %src.display(), dst=%dst.display(), "Destination already points to the source");
                        return Ok(());
                    }
                    warn!(src = %src.display(), dst = %dst.display(), dst_dst = %dst_link.display(), "Destination already exists and points elsewhere");
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
            std::os::unix::fs::symlink(src, dst)?;
        }
        Ok(())
    }

    fn link_symlink(&self, src: &Path, dst: &Path) -> io::Result<()> {
        let src_link = src.read_link()?;
        trace!(src = %src.display(), dst=%dst.display(), "src-link" = %src_link.display(), "Source is a symlink");
        if dst.exists() || dst.symlink_metadata().is_ok() {
            if self.force {
                if self.dry_run {
                    debug!(src = %src.display(), dst = %dst.display(), "Force removing destination (dry-run)");
                } else {
                    debug!(src = %src.display(), dst = %dst.display(), "Force removing destination");
                    fs::remove_file(dst)?;
                }
            } else if Some(&src_link) == dst.read_link().ok().as_ref() {
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
            std::os::unix::fs::symlink(&src_link, dst)?;
        }
        Ok(())
    }

    /// Links the contents of `src_base` into `dst_base`.
    ///
    /// # Errors
    ///
    /// Returns an error when either base cannot be inspected or canonicalized,
    /// or when a required filesystem mutation fails. Walk errors below the source
    /// root are logged and skipped.
    ///
    /// # Security
    ///
    /// Source and destination must be trusted, quiescent trees with no overlap.
    /// Symlinks in destination path components can redirect writes outside
    /// `dst_base`; the operation is not transactional and provides no rollback.
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
        self.walk(&src_base, &dst_base, Operation::Link)
    }

    /// Removes links created from `src_base` beneath `dst_base`.
    ///
    /// # Errors
    ///
    /// Returns an error when either base cannot be canonicalized, or when a
    /// required filesystem mutation fails. Walk errors below the source root are
    /// logged and skipped.
    ///
    /// # Security
    ///
    /// Source and destination must be trusted, quiescent trees with no overlap.
    /// With force enabled, this method removes same-name non-directory entries
    /// without proving that `dotr` created them. Symlinks in destination path
    /// components can redirect removals outside `dst_base`; the operation is not
    /// transactional and provides no rollback.
    pub fn unlink(&self, src_base: &Path, dst_base: &Path) -> io::Result<()> {
        info!(src = %src_base.display(), dst = %dst_base.display(), "Starting unlink operation");

        let dst_base = dst_base.canonicalize()?;
        let src_base = src_base.canonicalize()?;
        self.walk(&src_base, &dst_base, Operation::Unlink)
    }

    fn walk(&self, src_base: &Path, dst_base: &Path, operation: Operation) -> io::Result<()> {
        let mut iter = WalkDir::new(src_base).into_iter();
        while let Some(entry) = iter.next() {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    warn!("Error walking: {error}");
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

                if entry.path() != src_base {
                    let config = read_dir_config(entry.path());
                    if config.traverse == Some(Traverse::Link) {
                        match operation {
                            Operation::Link => {
                                debug!(path = %entry.path().display(), "Linking directory per .dotr traverse=link");
                                self.link_dir(entry.path(), src_base, dst_base)?;
                            }
                            Operation::Unlink => {
                                debug!(path = %entry.path().display(), "Unlinking directory per .dotr traverse=link");
                                self.unlink_dir(entry.path(), src_base, dst_base)?;
                            }
                        }
                        iter.skip_current_dir();
                        continue;
                    }
                }

                continue;
            }

            match operation {
                Operation::Link => self.link_entry(&entry, src_base, dst_base)?,
                Operation::Unlink => self.unlink_entry(&entry, src_base, dst_base)?,
            }
        }

        Ok(())
    }

    /// Unlinks one source entry from the destination tree.
    ///
    /// Directory entries are ignored because [`Self::unlink`] handles traversal.
    ///
    /// # Errors
    ///
    /// Returns an error when source inspection or a required filesystem mutation
    /// fails. Destination inspection failures may be treated as a missing
    /// destination.
    ///
    /// # Panics
    ///
    /// Panics when `src` is not below `src_base`.
    pub fn unlink_entry(&self, src: &DirEntry, src_base: &Path, dst_base: &Path) -> io::Result<()> {
        trace!(path = %src.path().display(), "Walking path");

        let src = src.path();
        let dst = destination_path(src, src_base, dst_base);

        let src_metadata = src.symlink_metadata()?;
        let src_type = src_metadata.file_type();

        if src_type.is_dir() {
            return Ok(());
        }

        if src_type.is_file() {
            return self.unlink_file(src, &dst);
        }

        if src_type.is_symlink() {
            return self.unlink_symlink(src, &dst);
        }

        warn!(src = %src.display(), dst = %dst.display(), "Skipping unknown source file type");
        Ok(())
    }

    fn unlink_file(&self, src: &Path, dst: &Path) -> io::Result<()> {
        trace!(src = %src.display(), dst = %dst.display(), "Unlink a file");
        let dst_metadata = dst.symlink_metadata();
        if !dst.exists() && dst_metadata.is_err() {
            debug!(src = %src.display(), dst = %dst.display(), "Destination doesn't exist - nothing to unlink");
            return Ok(());
        }
        let dst_metadata = dst_metadata?;

        if self.force {
            if self.dry_run {
                debug!(src = %src.display(), dst = %dst.display(), "Force removing (dry run)");
            } else {
                debug!(src = %src.display(), dst = %dst.display(), "Force removing");
                fs::remove_file(dst)?;
                return Ok(());
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
            }
            if !self.dry_run {
                fs::remove_file(dst)?;
            }
        } else {
            warn!(src = %src.display(), dst = %dst.display(), "Destination exists and is of unknown file type");
        }
        Ok(())
    }

    fn unlink_symlink(&self, src: &Path, dst: &Path) -> io::Result<()> {
        let src_link = src.read_link()?;
        trace!(src = %src.display(), dst = %dst.display(),  "Unlink a symlink");
        let dst_metadata = dst.symlink_metadata();
        if !dst.exists() && dst_metadata.is_err() {
            debug!(src = %src.display(), dst = %dst.display(), "Destination doesn't exist - nothing to unlink");
            return Ok(());
        }
        let dst_metadata = dst_metadata?;

        if self.force {
            if !self.dry_run {
                fs::remove_file(dst)?;
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
            }
            if !self.dry_run {
                fs::remove_file(dst)?;
            }
        } else {
            warn!(src = %src.display(), dst = %dst.display(), "Destination exists and is of unknown file type");
        }
        Ok(())
    }
}

fn destination_path(src: &Path, src_base: &Path, dst_base: &Path) -> PathBuf {
    let src_relative = src.strip_prefix(src_base).unwrap();
    dst_base.join(src_relative)
}

fn should_traverse(entry: &DirEntry) -> bool {
    entry.path().file_name() != Some(OsStr::new(".git"))
}
