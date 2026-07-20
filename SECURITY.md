# Reliability and security model

`dotr` is a convenience tool for dotfiles in directory trees controlled by one
trusted user. It is not a security boundary.

## Path assumptions

- Source and destination bases must not overlap. Equal or nested bases can
  replace or delete source data, especially with `--force`.
- Source and destination trees must remain quiescent for the duration of an
  operation. Filesystem checks and mutations are separate system calls, so
  concurrent changes can alter outcomes.
- Intermediate destination symlinks are followed. They can redirect link and
  unlink operations outside the destination base.
- Do not use paths writable by untrusted users and do not run `dotr` with
  elevated privileges.

## Destructive operations

`--force` may replace or remove a same-name non-directory destination without
proving that `dotr` created it. A normal unlink removes only a matching symlink,
but force unlink also removes unrelated files and symlinks. Real directories
receive limited protection, but that protection does not confine operations
when an intermediate path component is a symlink.

Operations are incremental and non-transactional. An error can leave a
partially updated destination, and `dotr` provides no rollback. Keep
recoverable backups and review both base paths before running destructive
operations.

Walk errors below the source root are logged and skipped. Destination lookup
failures may be treated as missing entries, so a successful operation does not
prove that every requested path was inspected or changed.

## Configuration

Missing, unreadable, or malformed `.dotr` files are silently treated as default
traversal. A root-level `.dotr` file is skipped and does not change traversal.
Treat source configuration as trusted input.
