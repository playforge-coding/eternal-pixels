# ink-rs

Rust bindings for [Google Ink](https://github.com/google/ink), the freehand
stroke library behind Android Jetpack Ink. Give it sampled pointer events and a
brush; get back smoothed, modelled triangle meshes to draw.

```rust
use std::time::Duration;

use ink_rs::{Brush, Color, InProgressStroke, Point, StockBrush, StrokeInput, StrokeInputBatch, ToolType};

let family = StockBrush::PressurePen.family();
let brush = Brush::new(&family, Color::from_packed_rgba(0x1a1a1aff), 4.0, 0.1)?;

let mut stroke = InProgressStroke::new();
stroke.start(&brush);

let inputs = StrokeInputBatch::new(&[
    StrokeInput::new(ToolType::Stylus, Point::new(10.0, 10.0), Duration::ZERO)
        .with_pressure(0.4),
    StrokeInput::new(ToolType::Stylus, Point::new(22.0, 18.0), Duration::from_millis(16))
        .with_pressure(0.7),
])?;
stroke.enqueue_inputs(&inputs, None)?;
stroke.update_shape(Duration::from_millis(16))?;

for coat in 0..stroke.coat_count() {
    let mesh = stroke.coat_mesh(coat);
    println!("{} triangles", mesh.triangle_count());
}
```

## Building

Ink is a Bazel project with no install step and no released binaries, so this
crate cannot fetch or build it for you. Point the build at one you have
already:

```bash
INK_ROOT=/path/to/prefix cargo build
```

where the prefix holds `include/ink/...`, `include/absl/...` and a
`lib/libink.a`. `INK_INCLUDE_DIR`, `INK_LIB_DIR`, `ABSL_INCLUDE_DIR` and
`ABSL_LIB_DIR` set the pieces individually.

Ink builds as a couple of hundred separate static archives, so producing a
single `libink.a` takes a merge step. The
[repository](https://github.com/playforge-coding/eternal-pixels) has
`tools/ink/bundle.sh`, which builds Ink with Bazel and lays out exactly this
prefix.

These bindings are written against Ink at commit
`dd67b9bb6524d002d5a43e1fe9286066a810af1e`.

## How it works

Ink's public API is built on `absl::StatusOr`, `absl::Span` and templates, none
of which cross an FFI boundary directly. So there is a small C++ facade
(`cc/ink_ffi.h`) restating the parts of Ink this crate exposes as plain structs,
enums and pointers with C linkage, and the Rust declarations for it are
generated from that header by bindgen and checked in. Nothing is generated at
build time, so no consumer needs bindgen or libclang.

## What is covered

Stock brush families, `Brush`, `StrokeInput` and `StrokeInputBatch`,
`InProgressStroke`, `Stroke`, and mesh readback from both stroke types.

Not covered yet: custom brush families (only the stock ones are exposed),
protobuf serialisation, Ink's own Skia and Metal renderers, the geometry
algorithms, and vertex attributes other than position.

## Threading

Ink's types are not thread-safe and neither are these. `Brush`, `Stroke` and
friends are deliberately neither `Send` nor `Sync`: build and read a stroke on
one thread, and move the plain `Mesh` data if another thread needs it.

## Licence

MPL-2.0. Ink itself is Apache-2.0.
