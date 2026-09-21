# eternal-ui-skia

Skia rendering for [eternal-ui](https://crates.io/crates/eternal-ui),
through [skia-safe](https://crates.io/crates/skia-safe).

Two types implement the toolkit's backend traits: `SkiaFonts` finds fonts
and measures text, and `SkiaRenderer` draws one frame onto a Skia `Canvas`.
Where the canvas comes from is up to the application: a CPU raster surface
as below, or a GPU surface through skia-safe's `gl`, `metal` or `vulkan`
features.

```rust
use eternal_ui::{ui, Size, Ui};
use eternal_ui_skia::{SkiaFonts, SkiaRenderer};

let mut ui: Ui<()> = Ui::new();
ui.set_root(ui! { <panel><button>"Hello"</button></panel> });

let fonts = SkiaFonts::new();
let mut surface = skia_safe::surfaces::raster_n32_premul((200, 100)).unwrap();
ui.layout(Size::new(200.0, 100.0), &fonts);
let mut renderer = SkiaRenderer::new(surface.canvas(), &fonts);
ui.paint(&mut renderer);
```

Shapes are drawn without anti-aliasing and text without subpixel
positioning, so 1px borders and pixel fonts stay crisp. `SkiaFonts::set_smooth`
turns anti-aliasing on for the ordinary look.

skia-safe downloads a prebuilt Skia for the target on first build. See its
documentation for building Skia from source or targeting other platforms.

## Licence

MPL-2.0. Skia is BSD-3-Clause.
