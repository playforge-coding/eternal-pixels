# Eternal Pixels

A UI suite and pixel art editor.

Not built yet. What exists so far is the Bazel monorepo and `ink-rs`, Rust
bindings to [Google Ink](https://github.com/google/ink).

## Layout

```
crates/
  eternal-pixels/   the application            (AGPL-3.0)
  eternal-ui/       UI toolkit                 (MPL-2.0)
  eternal-styler/   styling                    (MPL-2.0)
  ink-rs/           Google Ink bindings        (MPL-2.0)
    cc/             the C++ facade
    src/            the safe Rust API
patches/            Bazel module patches
tools/bindgen/      regenerates the FFI declarations
tools/ink/          builds an Ink prefix for the Cargo build
licenses/           licence texts, symlinked into each crate
docs/               this book
```

## Building

Bazel is the development build. It fetches Ink and builds it from source, so
nothing needs installing beyond Bazel.

```bash
bazel build //...
bazel test //...
```

The first build is long: it compiles Ink and Abseil.

## This book

The book is built with [mdBook](https://rust-lang.github.io/mdBook/). To work
on it locally:

```bash
mdbook serve docs --open
```

Pages live under `docs/src/`, and `docs/src/SUMMARY.md` is the table of
contents. Every push to `main` that touches `docs/` rebuilds the book and
publishes it to GitHub Pages.
