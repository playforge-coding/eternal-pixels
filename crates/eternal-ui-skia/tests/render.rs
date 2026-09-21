// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Renders a small tree to a raster surface and reads pixels back.

use eternal_ui::styler::Stylesheet;
use eternal_ui::{FontSpec, Fonts, Size, Ui, ui};
use eternal_ui_skia::{SkiaFonts, SkiaRenderer};

#[test]
fn fonts_measure_something_sensible() {
    let fonts = SkiaFonts::new();
    let spec = FontSpec {
        family: Default::default(),
        size: 12.0,
        weight: Default::default(),
        style: Default::default(),
        line_height: None,
    };
    let empty = fonts.measure("", &spec);
    let hello = fonts.measure("Hello", &spec);
    assert_eq!(empty.width, 0.0);
    assert!(hello.width > 20.0 && hello.width < 60.0, "{hello:?}");
    assert!(hello.ascent > 0.0 && hello.descent > 0.0);
    assert!(hello.line_height >= hello.ascent + hello.descent);
    let taller = fonts.measure(
        "Hello",
        &FontSpec {
            line_height: Some(40.0),
            ..spec
        },
    );
    assert_eq!(taller.line_height, 40.0);
}

#[test]
fn paints_backgrounds_borders_and_text() {
    let mut ui: Ui<()> = Ui::unstyled();
    ui.add_stylesheet(
        Stylesheet::parse(
            "* { font-size: 12px; }
             column { padding: 10px; background-color: #ffffff; }
             button { background-color: #ff0000; border: 2px solid #0000ff; padding: 4px 8px;
                      color: #000000; }",
        )
        .unwrap(),
    );
    ui.set_root(ui! { <column><button>"Hello"</button></column> });

    let fonts = SkiaFonts::new();
    let mut surface = skia_safe::surfaces::raster_n32_premul((160, 80)).unwrap();
    ui.layout(Size::new(160.0, 80.0), &fonts);
    let button = ui.children(ui.root().unwrap())[0];
    let rect = ui.rect(button);
    let label = ui.children(button)[0];
    let text = ui.content_rect(label);
    {
        let mut renderer = SkiaRenderer::new(surface.canvas(), &fonts);
        ui.paint(&mut renderer);
    }

    let pixels = surface.peek_pixels().unwrap();
    let at = |x: f32, y: f32| {
        let color = pixels.get_color((x as i32, y as i32));
        (color.r(), color.g(), color.b())
    };
    assert_eq!(at(2.0, 2.0), (255, 255, 255), "column background");
    assert_eq!(at(rect.x, rect.center().y), (0, 0, 255), "left border");
    assert_eq!(
        at(rect.right() - 1.0, rect.center().y),
        (0, 0, 255),
        "right border"
    );
    assert_eq!(at(rect.center().x, rect.y), (0, 0, 255), "top border");
    assert_eq!(
        at(rect.x + 3.0, rect.y + 3.0),
        (255, 0, 0),
        "button background"
    );

    // Somewhere in the label's box, text darkened the red.
    let mut dark = 0;
    for y in text.y as i32..text.bottom() as i32 {
        for x in text.x as i32..text.right() as i32 {
            if at(x as f32, y as f32) != (255, 0, 0) {
                dark += 1;
            }
        }
    }
    assert!(
        dark > 10,
        "expected glyph pixels inside {text:?}, found {dark}"
    );
}

/// A font file to load in tests, from wherever this machine keeps one.
fn some_font_file() -> Option<&'static str> {
    [
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/System/Library/Fonts/Monaco.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "C:\\Windows\\Fonts\\arial.ttf",
    ]
    .into_iter()
    .find(|path| std::path::Path::new(path).is_file())
}

fn spec(family: &str) -> FontSpec {
    FontSpec {
        family: eternal_ui::styler::FontFamily::new([family]),
        size: 12.0,
        weight: Default::default(),
        style: Default::default(),
        line_height: None,
    }
}

#[test]
fn loaded_fonts_are_used_by_family_name() {
    let Some(path) = some_font_file() else {
        eprintln!("no font file found on this machine, skipping");
        return;
    };
    let mut fonts = SkiaFonts::new();

    // Under the name in the file.
    let family = fonts.load_font_file(path).unwrap();
    assert!(!family.is_empty());
    assert_eq!(
        fonts.font(&spec(&family)).typeface().family_name(),
        family,
        "the loaded font should win for its own family name"
    );
    assert_eq!(
        fonts
            .font(&spec(&family.to_uppercase()))
            .typeface()
            .family_name(),
        family,
        "family names match regardless of case"
    );

    // Under a name of our own, from bytes this time.
    let bytes = std::fs::read(path).unwrap();
    fonts.load_font_as("pixel", &bytes).unwrap();
    assert_eq!(fonts.font(&spec("pixel")).typeface().family_name(), family);
    assert_eq!(
        fonts.loaded_families().collect::<Vec<_>>(),
        [family.to_ascii_lowercase().as_str(), "pixel"]
    );

    // A family nobody provides still falls back to something.
    let fallback = fonts.font(&spec("no-such-family")).typeface();
    assert!(!fallback.family_name().is_empty());
    assert!(fonts.measure("Hello", &spec("pixel")).width > 0.0);
}

#[test]
fn bad_font_data_is_an_error() {
    let mut fonts = SkiaFonts::new();
    let error = fonts.load_font(b"definitely not a font").unwrap_err();
    assert_eq!(error.to_string(), "not a font Skia can read");
    let error = fonts.load_font_file("/no/such/font.ttf").unwrap_err();
    assert!(
        error.to_string().starts_with("/no/such/font.ttf: "),
        "{error}"
    );
    assert!(fonts.loaded_families().next().is_none());
}
