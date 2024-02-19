use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs::{self};
use std::io::{self};
use std::path::{Path, PathBuf};

use tracing::{debug, info, trace, warn};
use walkdir::WalkDir;

pub struct Dotr {
    ignore: HashSet<PathBuf>,

    dry_run: bool,
    force: bool,
}

impl Dotr {
    pub fn new() -> Self {
        Dotr {
            ignore: HashSet::new(),
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

    pub fn link_entry(
        &self,
        src: &walkdir::DirEntry,
        src_base: &Path,
        dst_base: &Path,
    ) -> io::Result<()> {
        trace!(path = %src.path().display(), "Walking path");

        let src = src.path();
        let src_rel = src.strip_prefix(src_base).unwrap();

        if self.ignore.contains(src_rel) {
            debug!(path = %src.display(), "Ignoring file");
            return Ok(());
        }

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

        for src in WalkDir::new(&src_base)
            .into_iter()
            .filter_entry(should_traverse)
            .filter_map(|e| e.ok())
        {
            self.link_entry(&src, &src_base, &dst_base)?;
        }

        Ok(())
    }

    pub fn unlink(&self, src_base: &Path, dst_base: &Path) -> io::Result<()> {
        info!(src = %src_base.display(), dst = %dst_base.display(), "Starting unlink operation");

        let dst_base = dst_base.canonicalize()?;
        let src_base = src_base.canonicalize()?;

        assert!(dst_base.is_absolute());
        assert!(src_base.is_absolute());

        for src in WalkDir::new(&src_base)
            .into_iter()
            .filter_entry(should_traverse)
            .filter_map(|e| e.ok())
        {
            self.unlink_entry(&src, &src_base, &dst_base)?;
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

        if self.ignore.contains(src_rel) {
            debug!(path = %src.display(), "Ignoring file");
            return Ok(());
        }

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
