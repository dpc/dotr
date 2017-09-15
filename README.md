<!-- README.md is auto-generated from README.tpl with `cargo readme` -->

<p align="center">
  <a href="https://travis-ci.org/dpc/dotr">
      <img src="https://img.shields.io/travis/dpc/dotr/master.svg?style=flat-square" alt="Travis CI Build Status">
  </a>
  <a href="https://crates.io/crates/dotr">
      <img src="http://meritbadge.herokuapp.com/dotr?style=flat-square" alt="crates.io">
  </a>
  <a href="https://gitter.im/dpc/dotr">
      <img src="https://img.shields.io/badge/GITTER-join%20chat-green.svg?style=flat-square" alt="Gitter Chat">
  </a>
  <br>
</p>

# dotr

See [wiki](https://github.com/dpc/dotr/wiki) for current project status.

`dotr` is the simplest dotfile manager

It supports `link` and `unlink` operations and couple
of basic flags like `force`.

I wrote it for myself, so it's in Rust and does exactly what I want, so I
can fix/customize if I need something. But hey, maybe it also does
exactly what you want too!

#### Installation:

* [Install Rust](https://www.rustup.rs/)

```norust
cargo install dotr
```

#### Usage:

```norust
dotr --help
```

#### TODO:

* Make it a separate library + binary

# License

dotr is licensed under: MPL-2.0
