# dotr

`dotr` is a very simple dotfile manager.

It supports `link` and `unlink` operations and couple
of basic flags like `force`.

## Installation

* [Install Rust](https://www.rustup.rs/)

```
cargo install dotr
```

## Usage

```
dotr help
```

## Safety

`dotr` is a convenience tool for trusted, quiescent directory trees. It does
not confine operations beneath `--dst-dir`: symlinks in intermediate
destination path components can redirect writes and removals elsewhere.
Source and destination bases must not overlap.

`--force` may replace or delete entries that `dotr` did not create. Operations
are non-transactional and provide no rollback, so review paths and keep
recoverable backups before using force mode. Do not run `dotr` with elevated
privileges or on paths writable by untrusted users. See [`SECURITY.md`](SECURITY.md)
for the complete reliability model.

## `.dotr` directory config

A `.dotr` file (TOML format) can be placed in a non-root directory within the
source tree to control how that directory is handled. Root-level configuration
files are skipped but do not alter traversal.

### `traverse`

Controls how the directory is traversed during link/unlink operations.

- `traverse = "link"` — Instead of traversing the directory and linking its contents individually, create a symlink to the directory itself. This is useful when new files created in the destination should automatically appear in the source (e.g. for revision control).

The `.dotr` file itself is never linked to the destination.
Malformed configuration files are silently ignored.

Example `.dotr` file:

```toml
traverse = "link"
```

## License

dotr is licensed under: MPL-2.0

## AI usage disclosure

[I use LLMs when working on my projects.](https://dpc.pw/posts/personal-ai-usage-disclosure/)
