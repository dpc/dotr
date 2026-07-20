//! End-to-end characterization tests for the `dotr` command-line interface.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

struct Sandbox {
    _root: TempDir,
    home: PathBuf,
    src: PathBuf,
    dst: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        let root = TempDir::new().unwrap();
        let root_path = root.path().canonicalize().unwrap();
        let home = root_path.join("home");
        let src = root_path.join("source");
        let dst = root_path.join("destination");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dst).unwrap();

        Self {
            _root: root,
            home,
            src,
            dst,
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_dotr"));
        command
            .env_clear()
            .env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", self.home.join(".config"))
            .current_dir(&self.src);
        command
    }

    fn run(&self, operation: &str, flags: &[&str]) -> Output {
        self.run_with_paths(operation, flags, &self.src, &self.dst)
    }

    fn run_with_paths(&self, operation: &str, flags: &[&str], src: &Path, dst: &Path) -> Output {
        self.operation_command(operation, flags, src, dst)
            .output()
            .unwrap()
    }

    fn operation_command(
        &self,
        operation: &str,
        flags: &[&str],
        src: &Path,
        dst: &Path,
    ) -> Command {
        let mut command = self.command();
        command
            .arg("--src-dir")
            .arg(src)
            .arg("--dst-dir")
            .arg(dst)
            .args(flags)
            .arg(operation);
        command
    }
}

fn write(path: impl AsRef<Path>, content: &str) {
    fs::write(path, content).unwrap();
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed with {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        stderr(output)
    );
}

fn assert_error(output: &Output, message: &str) {
    assert_runtime_error(output);
    assert!(
        stderr(output).contains(message),
        "expected stderr to contain {message:?}, got:\n{}",
        stderr(output)
    );
}

fn assert_runtime_error(output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(255),
        "stderr:\n{}",
        stderr(output)
    );
}

fn assert_symlink(path: impl AsRef<Path>, target: impl AsRef<Path>) {
    assert_eq!(fs::read_link(path).unwrap(), target.as_ref());
}

fn assert_missing(path: impl AsRef<Path>) {
    assert!(fs::symlink_metadata(path).is_err());
}

fn configured_directory_conflicts() -> Sandbox {
    let sandbox = Sandbox::new();
    for name in ["wrong-link", "regular"] {
        fs::create_dir_all(sandbox.src.join(name)).unwrap();
        write(sandbox.src.join(name).join(".dotr"), "traverse = \"link\"");
    }
    symlink(OsStr::new("elsewhere"), sandbox.dst.join("wrong-link")).unwrap();
    write(sandbox.dst.join("regular"), "valuable");
    sandbox
}

#[test]
fn help_describes_the_command_line_interface() {
    let sandbox = Sandbox::new();
    let output = sandbox.command().arg("--help").output().unwrap();

    assert_success(&output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Usage: dotr [OPTIONS] --dst-dir <DST_DIR> <COMMAND>"));
    assert!(stdout.contains("link"));
    assert!(stdout.contains("unlink"));
    assert!(stdout.contains("--dry-run"));
    assert!(stdout.contains("--force"));
    assert!(output.stderr.is_empty());
}

#[test]
fn version_reports_the_package_version() {
    let sandbox = Sandbox::new();
    let output = sandbox.command().arg("--version").output().unwrap();

    assert_success(&output);
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "dotr 0.4.0\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn missing_required_destination_is_a_clap_error() {
    let sandbox = Sandbox::new();
    let output = sandbox.command().arg("link").output().unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("--dst-dir <DST_DIR>"));
    assert!(stderr(&output).contains("required arguments were not provided"));
}

#[test]
fn unknown_subcommand_is_a_clap_error() {
    let sandbox = Sandbox::new();
    let output = sandbox
        .command()
        .arg("--dst-dir")
        .arg(&sandbox.dst)
        .arg("install")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("unrecognized subcommand 'install'"));
}

#[test]
fn options_after_the_subcommand_are_rejected() {
    let sandbox = Sandbox::new();
    let output = sandbox
        .command()
        .arg("link")
        .arg("--dst-dir")
        .arg(&sandbox.dst)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("unexpected argument '--dst-dir'"));
}

#[test]
fn default_source_is_the_current_directory() {
    let sandbox = Sandbox::new();
    write(sandbox.src.join("from-cwd"), "content");
    let output = sandbox
        .command()
        .arg("--dst-dir")
        .arg(&sandbox.dst)
        .arg("link")
        .output()
        .unwrap();

    assert_success(&output);
    assert_symlink(sandbox.dst.join("from-cwd"), sandbox.src.join("from-cwd"));
}

#[test]
fn link_creates_file_links_and_nested_destination_directories() {
    let sandbox = Sandbox::new();
    fs::create_dir_all(sandbox.src.join("nested/deep")).unwrap();
    write(sandbox.src.join("top"), "top");
    write(sandbox.src.join("nested/deep/file"), "deep");

    let output = sandbox.run("link", &[]);

    assert_success(&output);
    assert_symlink(sandbox.dst.join("top"), sandbox.src.join("top"));
    assert!(sandbox.dst.join("nested/deep").is_dir());
    assert!(
        !fs::symlink_metadata(sandbox.dst.join("nested/deep"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_symlink(
        sandbox.dst.join("nested/deep/file"),
        sandbox.src.join("nested/deep/file"),
    );
}

#[test]
fn link_preserves_absolute_source_symlink_target() {
    let sandbox = Sandbox::new();
    write(sandbox.src.join("target"), "target");
    symlink(sandbox.src.join("target"), sandbox.src.join("link")).unwrap();

    let output = sandbox.run("link", &[]);

    assert_success(&output);
    assert_symlink(sandbox.dst.join("link"), sandbox.src.join("target"));
}

#[test]
fn link_preserves_relative_source_symlink_target() {
    let sandbox = Sandbox::new();
    write(sandbox.src.join("target"), "target");
    symlink(OsStr::new("target"), sandbox.src.join("link")).unwrap();

    let output = sandbox.run("link", &[]);

    assert_success(&output);
    assert_symlink(sandbox.dst.join("link"), Path::new("target"));
}

#[test]
fn link_duplicates_dangling_source_symlink() {
    let sandbox = Sandbox::new();
    symlink(OsStr::new("missing-target"), sandbox.src.join("dangling")).unwrap();

    let output = sandbox.run("link", &[]);

    assert_success(&output);
    assert_symlink(sandbox.dst.join("dangling"), Path::new("missing-target"));
    assert!(!sandbox.dst.join("dangling").exists());
}

#[test]
fn link_supports_non_utf8_file_names() {
    let sandbox = Sandbox::new();
    let name = OsString::from_vec(b"non-utf8-\xff".to_vec());
    write(sandbox.src.join(&name), "content");

    let output = sandbox.run("link", &[]);

    assert_success(&output);
    assert_symlink(sandbox.dst.join(&name), sandbox.src.join(&name));
}

#[test]
fn empty_source_is_a_successful_no_op() {
    let sandbox = Sandbox::new();

    let output = sandbox.run("link", &[]);

    assert_success(&output);
    assert_eq!(fs::read_dir(&sandbox.dst).unwrap().count(), 0);
}

#[test]
fn git_directories_are_skipped_at_every_depth() {
    let sandbox = Sandbox::new();
    fs::create_dir_all(sandbox.src.join(".git")).unwrap();
    fs::create_dir_all(sandbox.src.join("nested/.git")).unwrap();
    write(sandbox.src.join(".git/config"), "root");
    write(sandbox.src.join("nested/.git/config"), "nested");
    write(sandbox.src.join("nested/kept"), "kept");

    let output = sandbox.run("link", &[]);

    assert_success(&output);
    assert_missing(sandbox.dst.join(".git"));
    assert_missing(sandbox.dst.join("nested/.git"));
    assert_symlink(
        sandbox.dst.join("nested/kept"),
        sandbox.src.join("nested/kept"),
    );
}

#[test]
fn dotr_control_files_are_never_linked() {
    let sandbox = Sandbox::new();
    fs::create_dir_all(sandbox.src.join("nested")).unwrap();
    write(sandbox.src.join(".dotr"), "traverse = \"link\"");
    write(sandbox.src.join("nested/.dotr"), "");
    write(sandbox.src.join("nested/file"), "content");

    let output = sandbox.run("link", &[]);

    assert_success(&output);
    assert_missing(sandbox.dst.join(".dotr"));
    assert_missing(sandbox.dst.join("nested/.dotr"));
    assert_symlink(
        sandbox.dst.join("nested/file"),
        sandbox.src.join("nested/file"),
    );
}

#[test]
fn malformed_dotr_config_is_silently_ignored() {
    let sandbox = Sandbox::new();
    fs::create_dir_all(sandbox.src.join("configured")).unwrap();
    write(sandbox.src.join("configured/.dotr"), "not valid = [");
    write(sandbox.src.join("configured/file"), "content");

    let output = sandbox.run("link", &[]);

    assert_success(&output);
    assert!(sandbox.dst.join("configured").is_dir());
    assert_symlink(
        sandbox.dst.join("configured/file"),
        sandbox.src.join("configured/file"),
    );
}

#[test]
fn traverse_link_config_links_a_whole_directory() {
    let sandbox = Sandbox::new();
    fs::create_dir_all(sandbox.src.join("configured/nested")).unwrap();
    write(sandbox.src.join("configured/.dotr"), "traverse = \"link\"");
    write(sandbox.src.join("configured/nested/file"), "content");

    let output = sandbox.run("link", &[]);

    assert_success(&output);
    assert_symlink(
        sandbox.dst.join("configured"),
        sandbox.src.join("configured"),
    );
}

#[test]
fn root_traverse_link_config_is_ignored() {
    let sandbox = Sandbox::new();
    write(sandbox.src.join(".dotr"), "traverse = \"link\"");
    write(sandbox.src.join("file"), "content");

    let output = sandbox.run("link", &[]);

    assert_success(&output);
    assert_symlink(sandbox.dst.join("file"), sandbox.src.join("file"));
    assert_missing(sandbox.dst.join(".dotr"));
}

#[test]
fn existing_regular_file_is_preserved_without_force() {
    let sandbox = Sandbox::new();
    write(sandbox.src.join("file"), "source");
    write(sandbox.dst.join("file"), "destination");

    let output = sandbox.run("link", &["-v"]);

    assert_success(&output);
    assert_eq!(
        fs::read_to_string(sandbox.dst.join("file")).unwrap(),
        "destination"
    );
    assert!(
        stderr(&output).contains("Destination already exists and is not a symlink"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn existing_wrong_symlink_is_preserved_without_force() {
    let sandbox = Sandbox::new();
    write(sandbox.src.join("file"), "source");
    write(sandbox.dst.join("other"), "other");
    symlink(sandbox.dst.join("other"), sandbox.dst.join("file")).unwrap();

    let output = sandbox.run("link", &["-v"]);

    assert_success(&output);
    assert_symlink(sandbox.dst.join("file"), sandbox.dst.join("other"));
    assert!(stderr(&output).contains("points elsewhere"));
}

#[test]
fn existing_dangling_destination_symlink_is_preserved_without_force() {
    let sandbox = Sandbox::new();
    write(sandbox.src.join("file"), "source");
    symlink(OsStr::new("missing"), sandbox.dst.join("file")).unwrap();

    let output = sandbox.run("link", &[]);

    assert_success(&output);
    assert_symlink(sandbox.dst.join("file"), Path::new("missing"));
}

#[test]
fn relinking_an_existing_correct_link_is_idempotent() {
    let sandbox = Sandbox::new();
    write(sandbox.src.join("file"), "source");

    assert_success(&sandbox.run("link", &[]));
    let output = sandbox.run("link", &[]);

    assert_success(&output);
    assert_symlink(sandbox.dst.join("file"), sandbox.src.join("file"));
}

#[test]
fn force_replaces_regular_files_and_wrong_symlinks() {
    let sandbox = Sandbox::new();
    write(sandbox.src.join("file"), "source");
    write(sandbox.src.join("link"), "source");
    write(sandbox.dst.join("file"), "destination");
    symlink(OsStr::new("missing"), sandbox.dst.join("link")).unwrap();

    let output = sandbox.run("link", &["--force"]);

    assert_success(&output);
    assert_symlink(sandbox.dst.join("file"), sandbox.src.join("file"));
    assert_symlink(sandbox.dst.join("link"), sandbox.src.join("link"));
}

#[test]
fn force_refuses_to_replace_a_real_directory_with_a_file_link() {
    let sandbox = Sandbox::new();
    write(sandbox.src.join("collision"), "source");
    fs::create_dir(sandbox.dst.join("collision")).unwrap();
    write(sandbox.dst.join("collision/valuable"), "keep");

    let output = sandbox.run("link", &["--force"]);

    assert_runtime_error(&output);
    assert_eq!(
        fs::read_to_string(sandbox.dst.join("collision/valuable")).unwrap(),
        "keep"
    );
}

#[test]
fn force_refuses_to_replace_a_real_directory_with_a_configured_directory_link() {
    let sandbox = Sandbox::new();
    fs::create_dir(sandbox.src.join("configured")).unwrap();
    write(sandbox.src.join("configured/.dotr"), "traverse = \"link\"");
    fs::create_dir(sandbox.dst.join("configured")).unwrap();
    write(sandbox.dst.join("configured/valuable"), "keep");

    let output = sandbox.run("link", &["--force"]);

    assert_error(&output, "Can't safely remove");
    assert_eq!(
        fs::read_to_string(sandbox.dst.join("configured/valuable")).unwrap(),
        "keep"
    );
}

#[test]
fn configured_directory_conflicts_are_preserved_without_force() {
    let sandbox = configured_directory_conflicts();

    let output = sandbox.run("link", &[]);

    assert_success(&output);
    assert_symlink(sandbox.dst.join("wrong-link"), Path::new("elsewhere"));
    assert_eq!(
        fs::read_to_string(sandbox.dst.join("regular")).unwrap(),
        "valuable"
    );
}

#[test]
fn force_replaces_configured_directory_conflicts() {
    let sandbox = configured_directory_conflicts();

    let output = sandbox.run("link", &["--force"]);

    assert_success(&output);
    assert_symlink(
        sandbox.dst.join("wrong-link"),
        sandbox.src.join("wrong-link"),
    );
    assert_symlink(sandbox.dst.join("regular"), sandbox.src.join("regular"));
}

#[test]
fn force_dry_run_preserves_configured_directory_conflicts() {
    let sandbox = configured_directory_conflicts();

    let output = sandbox.run("link", &["--force", "--dry-run"]);

    assert_success(&output);
    assert_symlink(sandbox.dst.join("wrong-link"), Path::new("elsewhere"));
    assert_eq!(
        fs::read_to_string(sandbox.dst.join("regular")).unwrap(),
        "valuable"
    );
}

#[test]
fn dry_run_creates_no_links_or_parent_directories() {
    let sandbox = Sandbox::new();
    fs::create_dir_all(sandbox.src.join("nested/deep")).unwrap();
    write(sandbox.src.join("nested/deep/file"), "content");

    let output = sandbox.run("link", &["--dry-run"]);

    assert_success(&output);
    assert_missing(sandbox.dst.join("nested"));
}

#[test]
fn force_dry_run_preserves_existing_destination() {
    let sandbox = Sandbox::new();
    write(sandbox.src.join("file"), "source");
    write(sandbox.dst.join("file"), "destination");

    let output = sandbox.run("link", &["--force", "--dry-run"]);

    assert_success(&output);
    assert_eq!(
        fs::read_to_string(sandbox.dst.join("file")).unwrap(),
        "destination"
    );
}

#[test]
fn link_rejects_a_missing_destination_base() {
    let sandbox = Sandbox::new();
    fs::remove_dir(&sandbox.dst).unwrap();

    let output = sandbox.run("link", &[]);

    assert_error(&output, "Destination doesn't exist");
}

#[test]
fn link_rejects_a_destination_base_that_is_a_file() {
    let sandbox = Sandbox::new();
    fs::remove_dir(&sandbox.dst).unwrap();
    write(&sandbox.dst, "not a directory");

    let output = sandbox.run("link", &[]);

    assert_error(&output, "Destination is not a directory");
}

#[test]
fn link_reports_a_missing_source() {
    let sandbox = Sandbox::new();
    let missing = sandbox.src.join("missing");

    let output = sandbox.run_with_paths("link", &[], &missing, &sandbox.dst);

    assert_error(&output, "No such file or directory");
}

#[test]
fn destination_base_symlink_is_followed() {
    let sandbox = Sandbox::new();
    let actual = sandbox.home.join("actual-destination");
    fs::create_dir(&actual).unwrap();
    fs::remove_dir(&sandbox.dst).unwrap();
    symlink(&actual, &sandbox.dst).unwrap();
    write(sandbox.src.join("file"), "content");

    let output = sandbox.run("link", &[]);

    assert_success(&output);
    assert_symlink(actual.join("file"), sandbox.src.join("file"));
}

#[test]
fn identical_source_and_destination_preserves_file_without_force() {
    let sandbox = Sandbox::new();
    write(sandbox.src.join("file"), "valuable");

    let output = sandbox.run_with_paths("link", &["-v"], &sandbox.src, &sandbox.src);

    assert_success(&output);
    assert_eq!(
        fs::read_to_string(sandbox.src.join("file")).unwrap(),
        "valuable"
    );
    assert!(stderr(&output).contains("Destination already exists and is not a symlink"));
}

#[test]
fn force_link_with_identical_bases_replaces_source_with_self_symlink() {
    let sandbox = Sandbox::new();
    write(sandbox.src.join("file"), "valuable");

    let output = sandbox.run_with_paths("link", &["--force"], &sandbox.src, &sandbox.src);

    assert_success(&output);
    assert_symlink(sandbox.src.join("file"), sandbox.src.join("file"));
    assert!(!sandbox.src.join("file").exists());
}

#[test]
fn force_unlink_with_identical_bases_deletes_source_file() {
    let sandbox = Sandbox::new();
    write(sandbox.src.join("file"), "valuable");

    let output = sandbox.run_with_paths("unlink", &["--force"], &sandbox.src, &sandbox.src);

    assert_success(&output);
    assert_missing(sandbox.src.join("file"));
}

#[test]
fn source_nested_inside_destination_links_to_destination_sibling() {
    let sandbox = Sandbox::new();
    let nested_source = sandbox.dst.join("source");
    fs::create_dir(&nested_source).unwrap();
    write(nested_source.join("file"), "valuable");

    let output = sandbox.run_with_paths("link", &[], &nested_source, &sandbox.dst);

    assert_success(&output);
    assert_eq!(
        fs::read_to_string(nested_source.join("file")).unwrap(),
        "valuable"
    );
    assert_symlink(sandbox.dst.join("file"), nested_source.join("file"));
}

#[test]
fn intermediate_destination_symlink_redirects_link_outside_destination() {
    let sandbox = Sandbox::new();
    let external = sandbox.home.join("external");
    fs::create_dir(&external).unwrap();
    fs::create_dir(sandbox.src.join("nested")).unwrap();
    write(sandbox.src.join("nested/file"), "source");
    symlink(&external, sandbox.dst.join("nested")).unwrap();

    let output = sandbox.run("link", &[]);

    assert_success(&output);
    assert_symlink(external.join("file"), sandbox.src.join("nested/file"));
}

#[test]
fn intermediate_destination_symlink_redirects_unlink_outside_destination() {
    let sandbox = Sandbox::new();
    let external = sandbox.home.join("external");
    fs::create_dir(&external).unwrap();
    fs::create_dir(sandbox.src.join("nested")).unwrap();
    write(sandbox.src.join("nested/file"), "source");
    symlink(&external, sandbox.dst.join("nested")).unwrap();
    symlink(sandbox.src.join("nested/file"), external.join("file")).unwrap();

    let output = sandbox.run("unlink", &[]);

    assert_success(&output);
    assert_missing(external.join("file"));
}

#[test]
fn intermediate_destination_symlink_redirects_force_unlink_of_regular_file() {
    let sandbox = Sandbox::new();
    let external = sandbox.home.join("external");
    fs::create_dir(&external).unwrap();
    fs::create_dir(sandbox.src.join("nested")).unwrap();
    write(sandbox.src.join("nested/file"), "source");
    symlink(&external, sandbox.dst.join("nested")).unwrap();
    write(external.join("file"), "valuable");

    let output = sandbox.run("unlink", &["--force"]);

    assert_success(&output);
    assert_missing(external.join("file"));
}

#[test]
fn unlink_removes_links_but_leaves_created_parent_directories() {
    let sandbox = Sandbox::new();
    fs::create_dir_all(sandbox.src.join("nested")).unwrap();
    write(sandbox.src.join("top"), "top");
    write(sandbox.src.join("nested/file"), "nested");
    assert_success(&sandbox.run("link", &[]));

    let output = sandbox.run("unlink", &[]);

    assert_success(&output);
    assert_missing(sandbox.dst.join("top"));
    assert_missing(sandbox.dst.join("nested/file"));
    assert!(sandbox.dst.join("nested").is_dir());
}

#[test]
fn unlink_removes_duplicated_source_symlink() {
    let sandbox = Sandbox::new();
    write(sandbox.src.join("target"), "target");
    symlink(OsStr::new("target"), sandbox.src.join("link")).unwrap();
    assert_success(&sandbox.run("link", &[]));

    let output = sandbox.run("unlink", &[]);

    assert_success(&output);
    assert_missing(sandbox.dst.join("target"));
    assert_missing(sandbox.dst.join("link"));
}

#[test]
fn unlink_configured_directory_link_round_trips() {
    let sandbox = Sandbox::new();
    fs::create_dir(sandbox.src.join("configured")).unwrap();
    write(sandbox.src.join("configured/.dotr"), "traverse = \"link\"");
    write(sandbox.src.join("configured/file"), "content");
    assert_success(&sandbox.run("link", &[]));

    let output = sandbox.run("unlink", &[]);

    assert_success(&output);
    assert_missing(sandbox.dst.join("configured"));
}

#[test]
fn unlink_source_symlink_conflict_requires_force() {
    let sandbox = Sandbox::new();
    write(sandbox.src.join("target"), "target");
    symlink(OsStr::new("target"), sandbox.src.join("link")).unwrap();
    symlink(OsStr::new("elsewhere"), sandbox.dst.join("link")).unwrap();

    assert_success(&sandbox.run("unlink", &[]));
    assert_symlink(sandbox.dst.join("link"), Path::new("elsewhere"));

    assert_success(&sandbox.run("unlink", &["--force"]));
    assert_missing(sandbox.dst.join("link"));
}

#[test]
fn unlink_preserves_regular_destination_without_force() {
    let sandbox = Sandbox::new();
    write(sandbox.src.join("file"), "source");
    write(sandbox.dst.join("file"), "valuable");

    let output = sandbox.run("unlink", &["-v"]);

    assert_success(&output);
    assert_eq!(
        fs::read_to_string(sandbox.dst.join("file")).unwrap(),
        "valuable"
    );
    assert!(stderr(&output).contains("Destination already exists and is a file"));
}

#[test]
fn unlink_preserves_wrong_symlink_without_force() {
    let sandbox = Sandbox::new();
    write(sandbox.src.join("file"), "source");
    symlink(OsStr::new("elsewhere"), sandbox.dst.join("file")).unwrap();

    let output = sandbox.run("unlink", &["-v"]);

    assert_success(&output);
    assert_symlink(sandbox.dst.join("file"), Path::new("elsewhere"));
    assert!(stderr(&output).contains("symlink pointing to something else"));
}

#[test]
fn force_unlink_removes_regular_files_and_wrong_symlinks() {
    let sandbox = Sandbox::new();
    write(sandbox.src.join("file"), "source");
    write(sandbox.src.join("link"), "source");
    write(sandbox.dst.join("file"), "destination");
    symlink(OsStr::new("elsewhere"), sandbox.dst.join("link")).unwrap();

    let output = sandbox.run("unlink", &["--force"]);

    assert_success(&output);
    assert_missing(sandbox.dst.join("file"));
    assert_missing(sandbox.dst.join("link"));
}

#[test]
fn force_unlink_refuses_to_remove_a_real_directory_for_a_file() {
    let sandbox = Sandbox::new();
    write(sandbox.src.join("collision"), "source");
    fs::create_dir(sandbox.dst.join("collision")).unwrap();
    write(sandbox.dst.join("collision/valuable"), "keep");

    let output = sandbox.run("unlink", &["--force"]);

    assert_runtime_error(&output);
    assert_eq!(
        fs::read_to_string(sandbox.dst.join("collision/valuable")).unwrap(),
        "keep"
    );
}

#[test]
fn force_unlink_preserves_real_directory_for_configured_directory() {
    let sandbox = Sandbox::new();
    fs::create_dir(sandbox.src.join("configured")).unwrap();
    write(sandbox.src.join("configured/.dotr"), "traverse = \"link\"");
    fs::create_dir(sandbox.dst.join("configured")).unwrap();
    write(sandbox.dst.join("configured/valuable"), "keep");

    let output = sandbox.run("unlink", &["--force", "-v"]);

    assert_success(&output);
    assert_eq!(
        fs::read_to_string(sandbox.dst.join("configured/valuable")).unwrap(),
        "keep"
    );
    assert!(stderr(&output).contains("refusing to remove"));
}

#[test]
fn unlink_configured_directory_conflicts_preserve_force_safety_rules() {
    let sandbox = configured_directory_conflicts();

    assert_success(&sandbox.run("unlink", &["--force", "--dry-run"]));
    assert_symlink(sandbox.dst.join("wrong-link"), Path::new("elsewhere"));
    assert_eq!(
        fs::read_to_string(sandbox.dst.join("regular")).unwrap(),
        "valuable"
    );

    assert_success(&sandbox.run("unlink", &[]));
    assert_symlink(sandbox.dst.join("wrong-link"), Path::new("elsewhere"));
    assert_eq!(
        fs::read_to_string(sandbox.dst.join("regular")).unwrap(),
        "valuable"
    );

    assert_success(&sandbox.run("unlink", &["--force"]));
    assert_missing(sandbox.dst.join("wrong-link"));
    assert_eq!(
        fs::read_to_string(sandbox.dst.join("regular")).unwrap(),
        "valuable"
    );
}

#[test]
fn unlink_dry_run_preserves_correct_link() {
    let sandbox = Sandbox::new();
    write(sandbox.src.join("file"), "source");
    assert_success(&sandbox.run("link", &[]));

    let output = sandbox.run("unlink", &["--dry-run"]);

    assert_success(&output);
    assert_symlink(sandbox.dst.join("file"), sandbox.src.join("file"));
}

#[test]
fn force_unlink_dry_run_preserves_wrong_destination() {
    let sandbox = Sandbox::new();
    write(sandbox.src.join("file"), "source");
    write(sandbox.dst.join("file"), "destination");

    let output = sandbox.run("unlink", &["--force", "--dry-run"]);

    assert_success(&output);
    assert_eq!(
        fs::read_to_string(sandbox.dst.join("file")).unwrap(),
        "destination"
    );
}

#[test]
fn unlink_missing_destination_base_reports_canonicalization_error() {
    let sandbox = Sandbox::new();
    fs::remove_dir(&sandbox.dst).unwrap();

    let output = sandbox.run("unlink", &[]);

    assert_error(&output, "No such file or directory");
}

#[test]
fn unlink_destination_base_file_is_a_successful_no_op() {
    let sandbox = Sandbox::new();
    write(sandbox.src.join("file"), "source");
    fs::remove_dir(&sandbox.dst).unwrap();
    write(&sandbox.dst, "valuable");

    let output = sandbox.run("unlink", &[]);

    assert_success(&output);
    assert_eq!(fs::read_to_string(&sandbox.dst).unwrap(), "valuable");
}

#[test]
fn logs_default_to_info_on_stderr_and_verbosity_increases_detail() {
    let sandbox = Sandbox::new();
    write(sandbox.src.join("file"), "content");
    symlink(sandbox.src.join("file"), sandbox.dst.join("file")).unwrap();

    let default = sandbox.run("link", &[]);
    assert_success(&default);
    assert!(default.stdout.is_empty());
    assert!(stderr(&default).contains("Starting link operation"));
    assert!(!stderr(&default).contains("Destination already points"));
    assert!(!stderr(&default).contains("Walking path"));

    let debug = sandbox.run("link", &["-v"]);
    assert_success(&debug);
    assert!(debug.stdout.is_empty());
    assert!(stderr(&debug).contains("Destination already points"));
    assert!(!stderr(&debug).contains("Walking path"));

    let trace = sandbox.run("link", &["-vv"]);
    assert_success(&trace);
    assert!(trace.stdout.is_empty());
    assert!(stderr(&trace).contains("Walking path"));
}

#[test]
fn dotr_log_environment_controls_filtering() {
    let sandbox = Sandbox::new();
    write(sandbox.src.join("file"), "content");
    let quiet = sandbox
        .operation_command("link", &["-vv"], &sandbox.src, &sandbox.dst)
        .env("DOTR_LOG", "error")
        .output()
        .unwrap();

    assert_success(&quiet);
    assert!(quiet.stdout.is_empty());
    assert!(quiet.stderr.is_empty());

    assert_success(&sandbox.run("unlink", &[]));
    let trace = sandbox
        .operation_command("link", &[], &sandbox.src, &sandbox.dst)
        .env("DOTR_LOG", "trace")
        .output()
        .unwrap();

    assert_success(&trace);
    assert!(trace.stdout.is_empty());
    assert!(stderr(&trace).contains("Walking path"));
}

#[test]
fn empty_dotr_log_uses_the_default_filter() {
    let sandbox = Sandbox::new();
    write(sandbox.src.join("file"), "content");
    let output = sandbox
        .operation_command("link", &[], &sandbox.src, &sandbox.dst)
        .env("DOTR_LOG", "")
        .output()
        .unwrap();

    assert_success(&output);
    assert!(output.stdout.is_empty());
    assert!(stderr(&output).contains("Starting link operation"));
}

#[test]
fn invalid_dotr_log_reports_a_startup_error() {
    let sandbox = Sandbox::new();
    write(sandbox.src.join("file"), "content");
    let output = sandbox
        .operation_command("link", &[], &sandbox.src, &sandbox.dst)
        .env("DOTR_LOG", "[invalid")
        .output()
        .unwrap();

    assert_error(&output, "invalid DOTR_LOG filter");
    assert_missing(sandbox.dst.join("file"));
}

#[test]
fn rust_log_does_not_override_the_default_filter() {
    let sandbox = Sandbox::new();
    write(sandbox.src.join("file"), "content");
    let output = sandbox
        .operation_command("link", &[], &sandbox.src, &sandbox.dst)
        .env("RUST_LOG", "off")
        .output()
        .unwrap();

    assert_success(&output);
    assert!(output.stdout.is_empty());
    assert!(stderr(&output).contains("Starting link operation"));
}
