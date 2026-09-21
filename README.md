# Eternal Pixels

A UI suite and pixel art editor.

The editor is not built yet. What exists so far is the Bazel monorepo and
the libraries it will be made of: `ink-rs`, Rust bindings to
[Google Ink](https://github.com/google/ink); `eternal-styler`, a small CSS
engine built on Servo's `cssparser` and `selectors` crates; `eternal-ui`, a
dense, keyboard-first UI toolkit styled with that CSS; and `eternal-ui-skia`,
its Skia rendering backend.

## Layout

```
crates/
  eternal-pixels/   the application            (AGPL-3.0)
  eternal-ui/       UI toolkit                 (MPL-2.0)
    themes/         the built-in stylesheet
  eternal-ui-skia/  Skia backend for the toolkit (MPL-2.0)
  eternal-styler/   CSS engine for the toolkit (MPL-2.0)
  ink-rs/           Google Ink bindings        (MPL-2.0)
    cc/             the C++ facade
    src/            the safe Rust API
patches/            Bazel module patches
third_party/        crate_universe lockfile for crates.io dependencies
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

## Crates from crates.io

Dependencies are declared in each crate's `Cargo.toml`. Bazel reads those
manifests through rules_rust's crate_universe (see `MODULE.bazel`) and makes
every dependency available as `@crates//:<name>`. The resolved graph is pinned
in `third_party/Cargo.lock` and `third_party/cargo-bazel-lock.json`; after
changing a `Cargo.toml`, regenerate both:

```bash
CARGO_BAZEL_REPIN=1 bazel build //...
```

The first build of `eternal-ui-skia` downloads a prebuilt Skia through
skia-safe's build script, which runs inside the sandbox.

## The Cargo path

Every crate also has a `Cargo.toml` so it can be published to crates.io.
`eternal-styler`, `eternal-ui` and `eternal-ui-skia` are pure Rust (Skia comes
prebuilt) and build with plain `cargo` in their own directories. Cargo
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

## Examples

Each crate has an `examples/` directory, built by Bazel as `<name>_example`
binaries and by Cargo as ordinary examples:

```bash
bazel run //crates/ink-rs:stroke_example              # draws a stroke, prints it as ASCII
bazel run //crates/eternal-styler:inspect_example     # a style inspector; pass a .css file to try yours
bazel run //crates/eternal-ui:headless_example        # a whole app loop without a window
bazel run //crates/eternal-ui:custom_widget_example   # a widget of your own
bazel run //crates/eternal-ui-skia:render_png_example # renders the theme to eternal-ui.png
```

## Documentation

The docs are an [mdBook](https://rust-lang.github.io/mdBook/) under `docs/`,
with the rustdoc API reference for every crate published next to it at
`api/`. Both are built and published to GitHub Pages by
`.github/workflows/docs.yml` on every push to `main` that touches `docs/`,
`crates/` or `tools/docs/`. For that to work, the repository's Pages source
must be set to "GitHub Actions" in Settings > Pages.

```bash
mdbook serve docs --open    # live preview of the book while editing
mdbook build docs           # output in docs/book/
tools/docs/build-api.sh     # the API reference, into docs/book/api/
```

Pages live in `docs/src/`; add new ones to `docs/src/SUMMARY.md`. The API
reference is built with Cargo rather than Bazel's `rust_doc` targets so that
all four crates land in one tree with working links between them, and so
that Ink does not need to be built: `ink-rs` renders its docs without it.

- [docs/src/ink-bindings.md](docs/src/ink-bindings.md): how the Ink bindings
  are put together, how to add to them, and why they no longer use Crubit.
- [docs/src/styler.md](docs/src/styler.md): how the CSS engine is layered on
  Servo's crates, how the cascade works, and how to add a property.
- [docs/src/ui.md](docs/src/ui.md): how the toolkit is put together, what a
  frame looks like, the `ui!` macro, layout, and how to add a widget or a
  rendering backend.
