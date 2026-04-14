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

## `.dotr` directory config

A `.dotr` file (TOML format) can be placed in any directory within the source tree to control how that directory is handled.

### `traverse`

Controls how the directory is traversed during link/unlink operations.

- `traverse = "link"` — Instead of traversing the directory and linking its contents individually, create a symlink to the directory itself. This is useful when new files created in the destination should automatically appear in the source (e.g. for revision control).

The `.dotr` file itself is never linked to the destination.

Example `.dotr` file:

```toml
traverse = "link"
```

## License

dotr is licensed under: MPL-2.0
