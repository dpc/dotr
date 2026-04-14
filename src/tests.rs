use std::path::Path;
use std::{fs, io};

use tempfile::TempDir;

fn create_file(path: &Path) -> io::Result<()> {
    std::fs::File::create(path)?;
    Ok(())
}

fn assert_is_link(path: &Path, links_to: &Path) {
    let dst_path = fs::read_link(path).unwrap();
    assert_eq!(dst_path, links_to);
}

fn setup() -> (TempDir, TempDir) {
    (TempDir::new().unwrap(), TempDir::new().unwrap())
}

// ── link: basic operations ──────────────────────────────────────────

#[test]
fn link_single_file() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new();

    create_file(&src.join("a"))?;
    dotr.link(src, dst)?;
    assert_is_link(&dst.join("a"), &src.join("a"));
    Ok(())
}

#[test]
fn link_multiple_files() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new();

    create_file(&src.join("a"))?;
    create_file(&src.join("b"))?;
    create_file(&src.join("c"))?;
    dotr.link(src, dst)?;

    assert_is_link(&dst.join("a"), &src.join("a"));
    assert_is_link(&dst.join("b"), &src.join("b"));
    assert_is_link(&dst.join("c"), &src.join("c"));
    Ok(())
}

#[test]
fn link_nested_file() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new();

    fs::create_dir_all(src.join("foo"))?;
    create_file(&src.join("foo").join("a"))?;
    dotr.link(src, dst)?;

    assert_is_link(&dst.join("foo").join("a"), &src.join("foo").join("a"));
    Ok(())
}

#[test]
fn link_creates_parent_dirs() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new();

    fs::create_dir_all(src.join("a").join("b").join("c"))?;
    create_file(&src.join("a").join("b").join("c").join("f"))?;
    dotr.link(src, dst)?;

    assert!(dst.join("a").join("b").join("c").is_dir());
    assert_is_link(
        &dst.join("a").join("b").join("c").join("f"),
        &src.join("a").join("b").join("c").join("f"),
    );
    Ok(())
}

#[test]
fn link_symlink_duplicated() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new();

    create_file(&src.join("target"))?;
    std::os::unix::fs::symlink(src.join("target"), src.join("link"))?;

    dotr.link(src, dst)?;

    // the symlink in dst should have the same target as in src
    assert_is_link(&dst.join("link"), &src.join("target"));
    Ok(())
}

// ── link: idempotency ───────────────────────────────────────────────

#[test]
fn link_idempotent_file() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new();

    create_file(&src.join("a"))?;
    dotr.link(src, dst)?;
    dotr.link(src, dst)?; // second call should succeed silently

    assert_is_link(&dst.join("a"), &src.join("a"));
    Ok(())
}

#[test]
fn link_idempotent_symlink() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new();

    create_file(&src.join("target"))?;
    std::os::unix::fs::symlink(src.join("target"), src.join("link"))?;

    dotr.link(src, dst)?;
    dotr.link(src, dst)?;

    assert_is_link(&dst.join("link"), &src.join("target"));
    Ok(())
}

// ── link: conflict without force ────────────────────────────────────

#[test]
fn link_existing_regular_file_no_force() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new();

    create_file(&src.join("a"))?;
    create_file(&dst.join("a"))?; // pre-existing regular file

    dotr.link(src, dst)?; // should warn but not error

    // dst should still be a regular file, not a symlink
    assert!(dst.join("a").symlink_metadata()?.file_type().is_file());
    assert!(fs::read_link(dst.join("a")).is_err());
    Ok(())
}

#[test]
fn link_existing_symlink_elsewhere_no_force() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new();
    let other = TempDir::new().unwrap();

    create_file(&src.join("a"))?;
    create_file(&other.path().join("other"))?;
    std::os::unix::fs::symlink(other.path().join("other"), dst.join("a"))?;

    dotr.link(src, dst)?; // should warn, not overwrite

    // still points to other
    assert_is_link(&dst.join("a"), &other.path().join("other"));
    Ok(())
}

// ── link: force ─────────────────────────────────────────────────────

#[test]
fn link_force_overwrites_file() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new().set_force();

    create_file(&src.join("a"))?;
    create_file(&dst.join("a"))?; // pre-existing

    dotr.link(src, dst)?;
    assert_is_link(&dst.join("a"), &src.join("a"));
    Ok(())
}

#[test]
fn link_force_overwrites_symlink() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new().set_force();

    create_file(&src.join("a"))?;
    create_file(&src.join("target"))?;
    std::os::unix::fs::symlink(src.join("target"), src.join("link"))?;

    // pre-existing wrong symlink in dst
    create_file(&dst.join("wrong"))?;
    std::os::unix::fs::symlink(dst.join("wrong"), dst.join("link"))?;

    dotr.link(src, dst)?;
    assert_is_link(&dst.join("link"), &src.join("target"));
    Ok(())
}

// ── link: .git skip ─────────────────────────────────────────────────

#[test]
fn link_skips_git_dir() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new();

    fs::create_dir_all(src.join(".git"))?;
    create_file(&src.join(".git").join("config"))?;
    create_file(&src.join("a"))?;

    dotr.link(src, dst)?;

    assert!(!dst.join(".git").exists());
    assert_is_link(&dst.join("a"), &src.join("a"));
    Ok(())
}

// ── link: dry run ───────────────────────────────────────────────────

#[test]
fn link_dry_run_no_changes() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new().set_dry_run();

    create_file(&src.join("a"))?;
    dotr.link(src, dst)?;

    assert!(!dst.join("a").exists());
    Ok(())
}

// ── link: error cases ───────────────────────────────────────────────

#[test]
fn link_dst_not_exist() {
    let src = TempDir::new().unwrap();
    let dotr = super::Dotr::new();

    let result = dotr.link(src.path(), Path::new("/tmp/dotr_nonexistent_dir"));
    assert!(result.is_err());
}

#[test]
fn link_dst_not_dir() -> io::Result<()> {
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();
    let dotr = super::Dotr::new();

    let dst_file = dst.path().join("not_a_dir");
    create_file(&dst_file)?;

    let result = dotr.link(src.path(), &dst_file);
    assert!(result.is_err());
    Ok(())
}

// ── unlink: basic ───────────────────────────────────────────────────

#[test]
fn unlink_removes_linked_file() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new();

    create_file(&src.join("a"))?;
    dotr.link(src, dst)?;
    assert!(dst.join("a").exists());

    dotr.unlink(src, dst)?;
    assert!(!dst.join("a").exists());
    Ok(())
}

#[test]
fn unlink_removes_linked_symlink() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new();

    create_file(&src.join("target"))?;
    std::os::unix::fs::symlink(src.join("target"), src.join("link"))?;

    dotr.link(src, dst)?;
    assert!(dst.join("link").symlink_metadata().is_ok());

    dotr.unlink(src, dst)?;
    assert!(dst.join("link").symlink_metadata().is_err());
    Ok(())
}

#[test]
fn unlink_nested_file() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new();

    fs::create_dir_all(src.join("d"))?;
    create_file(&src.join("d").join("f"))?;

    dotr.link(src, dst)?;
    dotr.unlink(src, dst)?;
    assert!(!dst.join("d").join("f").exists());
    Ok(())
}

// ── unlink: no-op cases ─────────────────────────────────────────────

#[test]
fn unlink_nonexistent_dst_ok() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new();

    create_file(&src.join("a"))?;

    // never linked, so nothing to unlink
    dotr.unlink(src, dst)?;
    Ok(())
}

#[test]
fn unlink_regular_file_no_force() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new();

    create_file(&src.join("a"))?;
    create_file(&dst.join("a"))?; // regular file, not a symlink

    dotr.unlink(src, dst)?; // should warn but not remove

    assert!(dst.join("a").exists());
    Ok(())
}

#[test]
fn unlink_symlink_wrong_target_no_force() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new();

    create_file(&src.join("a"))?;
    create_file(&dst.join("other"))?;
    std::os::unix::fs::symlink(dst.join("other"), dst.join("a"))?;

    dotr.unlink(src, dst)?; // should warn, not remove

    assert!(dst.join("a").symlink_metadata().is_ok());
    Ok(())
}

// ── unlink: force ───────────────────────────────────────────────────

#[test]
fn unlink_force_removes_regular_file() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new().set_force();

    create_file(&src.join("a"))?;
    create_file(&dst.join("a"))?;

    dotr.unlink(src, dst)?;
    assert!(!dst.join("a").exists());
    Ok(())
}

#[test]
fn unlink_force_removes_wrong_symlink() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new().set_force();

    create_file(&src.join("a"))?;
    create_file(&dst.join("other"))?;
    std::os::unix::fs::symlink(dst.join("other"), dst.join("a"))?;

    dotr.unlink(src, dst)?;
    assert!(dst.join("a").symlink_metadata().is_err());
    Ok(())
}

// ── unlink: dry run ─────────────────────────────────────────────────

#[test]
fn unlink_dry_run_no_changes() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr_link = super::Dotr::new();
    let dotr_dry = super::Dotr::new().set_dry_run();

    create_file(&src.join("a"))?;
    dotr_link.link(src, dst)?;

    dotr_dry.unlink(src, dst)?;
    assert!(dst.join("a").exists()); // still there
    Ok(())
}

// ── round-trip: link then unlink ────────────────────────────────────

#[test]
fn roundtrip_multiple_files() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new();

    fs::create_dir_all(src.join("d"))?;
    create_file(&src.join("a"))?;
    create_file(&src.join("b"))?;
    create_file(&src.join("d").join("c"))?;

    dotr.link(src, dst)?;
    assert_is_link(&dst.join("a"), &src.join("a"));
    assert_is_link(&dst.join("b"), &src.join("b"));
    assert_is_link(&dst.join("d").join("c"), &src.join("d").join("c"));

    dotr.unlink(src, dst)?;
    assert!(!dst.join("a").exists());
    assert!(!dst.join("b").exists());
    assert!(!dst.join("d").join("c").exists());
    Ok(())
}

// ── .dotr config: traverse = "link" ────────────────────────────────

fn write_dotr_config(dir: &Path, content: &str) -> io::Result<()> {
    fs::write(dir.join(".dotr"), content)
}

#[test]
fn dotr_traverse_link_symlinks_directory() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new();

    fs::create_dir_all(src.join("subdir"))?;
    create_file(&src.join("subdir").join("file"))?;
    write_dotr_config(&src.join("subdir"), "traverse = \"link\"")?;

    dotr.link(src, dst)?;

    // subdir should be a symlink to the source directory
    let meta = dst.join("subdir").symlink_metadata()?;
    assert!(meta.file_type().is_symlink());
    assert_is_link(&dst.join("subdir"), &src.join("subdir"));

    // file inside should be accessible through the directory symlink
    assert!(dst.join("subdir").join("file").exists());
    Ok(())
}

#[test]
fn dotr_traverse_link_does_not_link_dotr_file() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new();

    create_file(&src.join("a"))?;
    write_dotr_config(src, "traverse = \"link\"")?;

    // .dotr in root — traverse=link is ignored for root, but .dotr should not be
    // linked
    dotr.link(src, dst)?;

    assert!(!dst.join(".dotr").exists());
    assert_is_link(&dst.join("a"), &src.join("a"));
    Ok(())
}

#[test]
fn dotr_traverse_link_skips_content_linking() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new();

    fs::create_dir_all(src.join("subdir").join("nested"))?;
    create_file(&src.join("subdir").join("file"))?;
    create_file(&src.join("subdir").join("nested").join("deep"))?;
    write_dotr_config(&src.join("subdir"), "traverse = \"link\"")?;

    dotr.link(src, dst)?;

    // subdir itself is a symlink, not a real directory with individual file
    // symlinks
    let meta = dst.join("subdir").symlink_metadata()?;
    assert!(meta.file_type().is_symlink());
    Ok(())
}

#[test]
fn dotr_traverse_link_idempotent() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new();

    fs::create_dir_all(src.join("subdir"))?;
    create_file(&src.join("subdir").join("file"))?;
    write_dotr_config(&src.join("subdir"), "traverse = \"link\"")?;

    dotr.link(src, dst)?;
    dotr.link(src, dst)?;

    assert_is_link(&dst.join("subdir"), &src.join("subdir"));
    Ok(())
}

#[test]
fn dotr_traverse_link_unlink_roundtrip() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new();

    fs::create_dir_all(src.join("subdir"))?;
    create_file(&src.join("subdir").join("file"))?;
    write_dotr_config(&src.join("subdir"), "traverse = \"link\"")?;

    dotr.link(src, dst)?;
    assert!(dst.join("subdir").symlink_metadata().is_ok());

    dotr.unlink(src, dst)?;
    assert!(dst.join("subdir").symlink_metadata().is_err());
    Ok(())
}

#[test]
fn dotr_traverse_link_mixed_with_regular_files() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new();

    create_file(&src.join("regular"))?;
    fs::create_dir_all(src.join("linked_dir"))?;
    create_file(&src.join("linked_dir").join("inside"))?;
    write_dotr_config(&src.join("linked_dir"), "traverse = \"link\"")?;
    fs::create_dir_all(src.join("normal_dir"))?;
    create_file(&src.join("normal_dir").join("inside"))?;

    dotr.link(src, dst)?;

    // regular file linked normally
    assert_is_link(&dst.join("regular"), &src.join("regular"));
    // linked_dir is a directory symlink
    let meta = dst.join("linked_dir").symlink_metadata()?;
    assert!(meta.file_type().is_symlink());
    // normal_dir contents are linked individually
    assert!(dst.join("normal_dir").is_dir());
    assert_is_link(
        &dst.join("normal_dir").join("inside"),
        &src.join("normal_dir").join("inside"),
    );
    Ok(())
}

#[test]
fn dotr_no_config_traverses_normally() -> io::Result<()> {
    let (src, dst) = setup();
    let (src, dst) = (src.path(), dst.path());
    let dotr = super::Dotr::new();

    fs::create_dir_all(src.join("subdir"))?;
    create_file(&src.join("subdir").join("file"))?;

    dotr.link(src, dst)?;

    // subdir should be a real directory, not a symlink
    let meta = dst.join("subdir").symlink_metadata()?;
    assert!(meta.file_type().is_dir());
    assert_is_link(
        &dst.join("subdir").join("file"),
        &src.join("subdir").join("file"),
    );
    Ok(())
}
