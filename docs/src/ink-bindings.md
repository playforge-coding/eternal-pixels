# The Ink bindings

`crates/ink-rs` is a Rust wrapper around
[Google Ink](https://github.com/google/ink).

## Why there is a C++ facade in the middle

Ink's public API is built on `absl::StatusOr`, `absl::Span` and templates, none
of which cross an FFI boundary directly. Counted across Ink's 72 public
headers:

| construct | occurrences |
| --- | --- |
| `absl::Span<...>` | 182 |
| `std::vector<...>` | 144 |
| `std::string` | 99 |
| `std::optional<...>` | 72 |
| `absl::StatusOr<...>` | 71 |
| templates | 90 |

`Brush::Create`, `StrokeInputBatch::Create`, `InProgressStroke::EnqueueInputs`
and most other entry points return `absl::StatusOr` or `absl::Status` and take
spans. No binding generator turns that into usable Rust on its own.

So `crates/ink-rs/cc/ink_ffi.h` restates the parts of Ink we use as plain
structs, scoped enums and pointers, with C linkage. The header includes nothing
but `<cstdint>`, and no Ink type or standard library type appears in any
signature. `ink_ffi.cc` is where the real Ink headers get included, and it is
ordinary C++ with no constraints on it.

## The layers

```
ink/...                        Google Ink, built from source by Bazel
  └─ //crates/ink-rs/cc:ink_ffi           the C++ facade (ink_ffi.h + .cc)
       └─ src/ffi/generated.rs            bindgen output, checked in
            └─ //crates/ink-rs            the safe Rust API
```

Nothing is generated during the build. `src/ffi/generated.rs` is produced by
`tools/bindgen/generate.sh` and committed, so neither Bazel nor Cargo needs
bindgen or libclang. It carries compile-time assertions on every struct size
and field offset, so a header change that is not reflected in it fails to
build rather than corrupting memory at run time.

Re-run the script after editing `ink_ffi.h`.

## Conventions across the boundary

The facade follows a few rules so the Rust side can stay safe:

- **Ownership.** Ink objects are handed out as one-word handle structs
  (`BrushHandle`, `StrokeHandle`, ...). Every `*Create`/`*Clone` has a matching
  `*Destroy`. On the Rust side each handle lives in a type with a `Drop` impl,
  and nothing else touches it.
- **Errors.** Fallible calls return a `StatusCode` mirroring `absl::StatusCode`,
  and record the message in thread-local storage that `LastErrorMessage` reads
  back. `error::check` turns the pair into a `Result` and must be called before
  anything else crosses the boundary on that thread.
- **Absent values.** Ink uses sentinels: `0` for a missing physical length, `-1`
  for missing pressure and stylus angles. The facade passes them through
  unchanged and `input.rs` converts to and from `Option<f32>`.
- **Null handles.** Every entry point tolerates a null handle and returns a
  harmless default. This matters because several Ink methods `CHECK`-fail on a
  bad index; the facade range-checks first so a mistake in Rust cannot abort the
  process.
- **Geometry.** Meshes are copied out into `Vec`s rather than borrowed. Ink
  rebuilds its meshes in place as a stroke grows, so any borrow would be
  invalidated by the next `update_shape`.
- **Floats.** Ink stores times as `f32` seconds and colours as linear floats,
  so neither round-trips exactly. Compare with a tolerance.

## Adding to the bindings

To expose more of Ink, work outwards:

1. Add the entry point to `cc/ink_ffi.h`, keeping to plain structs, scoped
   enums and pointers. Nothing from Ink or the standard library in a signature.
2. Implement it in `cc/ink_ffi.cc`, converting at the edges. Add whatever Ink
   target it needs to the `deps` in `cc/BUILD.bazel`.
3. Run `tools/bindgen/generate.sh`.
4. Wrap it in the matching `src/*.rs` module. The generated declarations are
   only ever named from `src/ffi/mod.rs`, so anything new goes through there.
5. Add a test to `tests/strokes.rs`.

## What is bound so far

The core stroke pipeline: geometry and colour values, stock brush families,
`Brush`, `StrokeInput`/`StrokeInputBatch`, `InProgressStroke`, `Stroke`, and
mesh readback from both stroke types.

Not yet bound, and needing facade work when they are wanted:

- **Custom brush families.** Only the stock brushes are exposed.
  `BrushFamily::Create` takes spans of `BrushCoat`, each a tree of `BrushTip`,
  `BrushPaint`, `BrushBehavior` and `EasingFunction`. It is the largest
  remaining piece.
- **Storage.** `ink/storage` serialises strokes through protobuf. Worth doing
  as its own facade rather than bolting onto this one.
- **Rendering.** `ink/rendering` targets Skia, Metal and `android.graphics.Mesh`
  directly. Reading meshes back and drawing them ourselves is likely to suit a
  pixel editor better.
- **Geometry algorithms.** Intersection, convex hull, tessellation and affine
  transforms.
- **Vertex attributes beyond position.** `coat_mesh` returns positions and
  indices. Brushes that vary colour or texture along the stroke also write other
  vertex attributes, which the facade does not read yet.

## Two build paths

Bazel is the development build: it fetches Ink and builds it from source, so
`bazel test //...` needs nothing installed beyond Bazel itself.

Cargo exists so the crate can be published. It cannot build Ink, which is a
Bazel project with no install step, so it links against one you point it at
with `INK_ROOT`. `tools/ink/bundle.sh` produces a suitable prefix from the
Bazel build: Ink compiles to a couple of hundred separate static archives and
no single `libink.a`, so the script merges them and lays the headers out
beside the result.

```bash
./tools/ink/bundle.sh
INK_ROOT="$PWD/.ink-prefix" cargo test --manifest-path crates/ink-rs/Cargo.toml
```

Both paths compile the same facade and the same checked-in declarations, and
both run the same tests.

## Why not Crubit

The first version of these bindings used [Crubit](https://github.com/google/crubit),
and it did work: it generated 1916 lines covering all 63 facade functions, and
the tests passed against them. It was removed anyway, for two reasons.

**Its output cannot be published.** The generated code calls
`::ctor::MoveAndAssignViaCopy` from Crubit's own `ctor` crate, which is not on
crates.io; the `ctor` name there belongs to an unrelated crate. `ffi_11` and
`forward_declare` are reserved placeholder versions. The generated code also
requires nightly. So publishing would have meant a second, non-Crubit FFI layer
alongside the first.

**The facade makes it redundant.** Crubit exists to bind rich C++ directly, but
it does not handle `absl::StatusOr` or `absl::Span`, which is why the facade
exists at all. Once the facade reduced the surface to plain C, Crubit was
generating something bindgen generates too, without needing sixteen patches to
its Bazel rules, a separately built 70 MB generator, a pinned LLVM 23, static
Abseil and Protobuf builds, and a hand-written stub for a symbol missing from
Homebrew's clang.

Dropping it removed about 1500 lines of patches and tooling and let the Rust
toolchain go back to stable. What was kept, the facade and the safe API, was
always the real work.

## Known risks

- **Replicated overrides.** Bazel only honours module overrides from the root
  module, so every override Ink declares for itself is silently dropped here
  and `//MODULE.bazel` has to repeat it. That covers Ink's `git_override` for
  Dawn; without it, resolution fails outright with "module
  dawn@v20260731.171941 not found in registries". Repo rules called directly
  from a `MODULE.bazel`, which is how Ink pulls in Skia and libtess2, are not
  affected.
- **Ink patched to be depend-able.** `toolchains_llvm` refuses to run its
  `llvm` extension for any module that is not the root, and Ink uses it. Ink is
  patched to drop it (`patches/ink/`), and the root module registers the Apple
  toolchain instead.
- **Pinned commits.** Neither Ink nor Dawn is on the Bazel Central Registry, so
  both come in via `git_override` at a pinned commit. The Ink commit is also
  recorded in `crates/ink-rs/Cargo.toml` and in `build.rs`, since a Cargo build
  links against whatever Ink the user supplies and nothing checks the version
  for them.
- **No version check on the Cargo path.** If `INK_ROOT` points at a different
  Ink than the facade was written against, the mismatch shows up as a link
  error at best and wrong behaviour at worst.
