use std::path::Path;
use std::{fs, io};

use tempdir::TempDir;

fn create_file(path: &Path) -> io::Result<()> {
    std::fs::File::create(path)?;
    Ok(())
}

fn assert_is_link(path: &Path, links_to: &Path) {
    let dst_path = fs::read_link(path).unwrap();

    assert_eq!(dst_path, links_to);
}

#[test]
fn simple_file() -> io::Result<()> {
    let dotr = super::Dotr::new();

    let src = TempDir::new("src").unwrap();
    let dst = TempDir::new("dst").unwrap();
    let src = src.path();
    let dst = dst.path();

    let src_path = src.join("a");
    let dst_path = dst.join("a");
    create_file(&src_path)?;

    dotr.link(src, dst)?;
    assert_is_link(&dst_path, &src_path);

    dotr.unlink(src, dst)?;
    assert!(!dst_path.exists());

    Ok(())
}

#[test]
fn simple_nested_file() -> io::Result<()> {
    let dotr = super::Dotr::new();

    let src = TempDir::new("src").unwrap();
    let dst = TempDir::new("dst").unwrap();
    let src = src.path();
    let dst = dst.path();

    let src_path = src.join("foo").join("a");
    let dst_path = dst.join("foo").join("a");
    fs::create_dir_all(src.join("foo"))?;
    create_file(&src_path)?;

    dotr.link(src, dst)?;
    assert_is_link(&dst_path, &src_path);

    dotr.unlink(src, dst)?;
    assert!(!dst_path.exists());

    Ok(())
}

#[test]
fn simple_symlink() -> io::Result<()> {
    let dotr = super::Dotr::new();

    let src = TempDir::new("src").unwrap();
    let dst = TempDir::new("dst").unwrap();
    let src = src.path();
    let dst = dst.path();

    let src_path = src.join("a");
    let src_link_path = src.join("a.lnk");
    let dst_path = dst.join("a");
    let dst_link_path = dst.join("a.lnk");
    create_file(&src_path)?;

    std::os::unix::fs::symlink(&src_path, src_link_path)?;

    dotr.link(src, dst)?;
    assert_is_link(&dst_link_path, &src_path);

    dotr.unlink(src, dst)?;
    assert!(!dst_path.exists());
    assert!(!dst_link_path.exists());

    Ok(())
}
