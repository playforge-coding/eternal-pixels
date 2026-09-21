// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Renders a showcase of the default theme to a PNG, with widgets in their
//! various states, so the look can be checked without a window.
//!
//! Writes `eternal-ui.png` in the current directory, or to the path given
//! as the first argument. Run with
//! `bazel run //crates/eternal-ui-skia:render_png_example` or
//! `cargo run --example render_png`.

use std::path::PathBuf;
use std::process::ExitCode;

use eternal_ui::{Event, Key, Modifiers, PointerButton, Size, Ui, ui};
use eternal_ui_skia::{SkiaFonts, SkiaRenderer};
use skia_safe::EncodedImageFormat;

fn main() -> ExitCode {
    let path = match std::env::args().nth(1) {
        Some(path) => PathBuf::from(path),
        None => {
            // `bazel run` starts in the runfiles tree; write where the user is.
            let dir = std::env::var_os("BUILD_WORKING_DIRECTORY")
                .map(PathBuf::from)
                .unwrap_or_default();
            dir.join("eternal-ui.png")
        }
    };

    let mut ui: Ui<()> = Ui::new();
    ui.set_root(ui! {
        <column class="panel">
            <row class="toolbar">
                <button id="new">"New"</button>
                <button id="open">"Open"</button>
                <button id="save" class="primary" shortcut="Ctrl+S">"Save"</button>
                <button id="export" disabled="true">"Export"</button>
                <spacer/>
                <label class="dim">"sketch.png"</label>
            </row>
            <row>
                <column grow="1">
                    <label class="title">"Canvas"</label>
                    <checkbox checked="true">"Show grid"</checkbox>
                    <checkbox>"Snap to pixels"</checkbox>
                    <checkbox checked="true" disabled="true">"Locked"</checkbox>
                </column>
                <separator/>
                <column grow="1">
                    <label class="title">"Layer"</label>
                    <input id="name" value="Background"/>
                    <input placeholder="Opacity"/>
                </column>
            </row>
        </column>
    });

    let fonts = SkiaFonts::new();
    let size = Size::new(360.0, 150.0);
    ui.layout(size, &fonts);

    // Put some widgets into interactive states so the theme's hover, active
    // and focus styles show: hover Open, hold New down, focus the field.
    let open = ui.rect(ui.find("open").unwrap()).center();
    let new = ui.rect(ui.find("new").unwrap()).center();
    let _ = ui.handle(&Event::PointerMove { position: open }, &fonts);
    let _ = ui.handle(
        &Event::PointerDown {
            position: new,
            button: PointerButton::Primary,
        },
        &fonts,
    );
    let _ = ui.handle(&Event::PointerMove { position: open }, &fonts);
    let _ = ui.handle(
        &Event::KeyDown {
            key: Key::Tab,
            modifiers: Modifiers::empty(),
        },
        &fonts,
    );
    let _ = ui.handle(
        &Event::KeyDown {
            key: Key::Tab,
            modifiers: Modifiers::SHIFT,
        },
        &fonts,
    );
    ui.layout(size, &fonts);

    let mut surface =
        skia_safe::surfaces::raster_n32_premul((size.width as i32, size.height as i32))
            .expect("a raster surface");
    {
        let mut renderer = SkiaRenderer::new(surface.canvas(), &fonts);
        ui.paint(&mut renderer);
    }
    let image = surface.image_snapshot();
    let Some(png) = image.encode(None, EncodedImageFormat::PNG, 100) else {
        eprintln!("could not encode the image as PNG");
        return ExitCode::FAILURE;
    };
    if let Err(error) = std::fs::write(&path, png.as_bytes()) {
        eprintln!("{}: {error}", path.display());
        return ExitCode::FAILURE;
    }
    println!("wrote {} ({}x{})", path.display(), size.width, size.height);
    ExitCode::SUCCESS
}
