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
```

Each crate carries a `LICENSE` symlink pointing at the text in `licenses/`.

## Building

Bazel is the development build. It fetches Ink and builds it from source, so
nothing needs installing beyond Bazel.

```bash
bazel build //...
bazel test //...
```

The first build is long: it compiles Ink and Abseil.

## Day to day

```bash
bazel build --config=clippy //...    # instead of cargo clippy
bazel test  --config=rustfmt //...   # instead of cargo fmt --check
bazel run @rules_rust//tools/rust_analyzer:gen_rust_project  # IDE support
```

## The Cargo path

`ink-rs` also has a `Cargo.toml` so it can be published to crates.io. Cargo
cannot build Ink, which is a Bazel project with no install step, so it links
against a prefix you point it at. `tools/ink/bundle.sh` builds one from the
Bazel outputs:

```bash
./tools/ink/bundle.sh
INK_ROOT="$PWD/.ink-prefix" cargo test --manifest-path crates/ink-rs/Cargo.toml
```

Both paths compile the same C++ facade and the same checked-in FFI
declarations, and run the same tests. Bazel is what you want for development;
the Cargo path exists for publishing and for anyone consuming the crate from
crates.io.

After changing `crates/ink-rs/cc/ink_ffi.h`, run `tools/bindgen/generate.sh` to
regenerate the declarations. That needs bindgen (`cargo install bindgen-cli`),
but only for whoever changes the header: the output is checked in, so building
the crate never needs bindgen or libclang.

## Documentation

The docs are an [mdBook](https://rust-lang.github.io/mdBook/) under `docs/`,
published to GitHub Pages by `.github/workflows/docs.yml` on every push to
`main` that touches `docs/`. For that to work, the repository's Pages source
must be set to "GitHub Actions" in Settings > Pages.

```bash
mdbook serve docs --open    # live preview while editing
mdbook build docs           # output in docs/book/
```

Pages live in `docs/src/`; add new ones to `docs/src/SUMMARY.md`.

- [docs/src/ink-bindings.md](docs/src/ink-bindings.md): how the Ink bindings
  are put together, how to add to them, and why they no longer use Crubit.
