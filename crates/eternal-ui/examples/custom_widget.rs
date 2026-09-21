// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A widget of your own: a colour swatch that cycles through a palette when
//! clicked or when Space is pressed, and reports the chosen colour.
//!
//! Shows the three parts of a custom widget: a struct implementing
//! [`Widget`], an `Element::new` call that gives it a tag for the
//! stylesheet, and stylesheet rules for that tag. The built-in handlers
//! (`click`, `change`, ...) carry fixed payloads, so a widget with its own
//! kind of event takes a callback and emits the message itself. Run with
//! `bazel run //crates/eternal-ui:custom_widget_example` or
//! `cargo run --example custom_widget`.

use eternal_ui::styler::{Color, ComputedStyle, ElementState, Stylesheet};
use eternal_ui::{
    Element, Event, EventCx, Fonts, Key, MonospaceFonts, PaintCx, PointerButton, RecordingRenderer,
    Rect, Renderer, Size, Ui, Widget, impl_widget_any, ui,
};

#[derive(Clone, Debug)]
enum Msg {
    Picked(Color),
}

/// A square of colour. Clicking it moves to the next colour in the palette
/// and reports the new one through `on_pick`.
struct Swatch<M> {
    palette: Vec<Color>,
    current: usize,
    on_pick: Box<dyn Fn(Color) -> M>,
}

impl<M> Swatch<M> {
    fn new(palette: Vec<Color>, on_pick: impl Fn(Color) -> M + 'static) -> Self {
        Self {
            palette,
            current: 0,
            on_pick: Box::new(on_pick),
        }
    }

    fn color(&self) -> Color {
        self.palette[self.current]
    }
}

impl<M: 'static> Widget<M> for Swatch<M> {
    /// `<swatch index="2"/>` picks the starting colour.
    fn set_attribute(&mut self, name: &str, value: &str) -> bool {
        if name == "index"
            && let Ok(index) = value.parse::<usize>()
        {
            self.current = index % self.palette.len();
            return true;
        }
        false
    }

    /// A square as tall as the line, so it lines up with text next to it.
    fn measure(&self, _fonts: &dyn Fonts, style: &ComputedStyle) -> Size {
        let side = style
            .line_height
            .resolve(style.font_size)
            .unwrap_or(style.font_size * 1.25);
        Size::new(side, side)
    }

    fn focusable(&self) -> bool {
        true
    }

    fn event(&mut self, cx: &mut EventCx<'_, M>, event: &Event) -> bool {
        let advance = match event {
            Event::PointerUp { position, .. } => {
                cx.state.contains(ElementState::ACTIVE) && cx.rect.contains(*position)
            }
            Event::KeyDown {
                key: Key::Space, ..
            } => true,
            _ => false,
        };
        if advance {
            self.current = (self.current + 1) % self.palette.len();
            cx.emit((self.on_pick)(self.color()));
            cx.mark_changed();
        }
        advance
    }

    /// The colour itself; the border and padding come from the stylesheet.
    fn paint(&self, renderer: &mut dyn Renderer, _cx: &PaintCx<'_>, content: Rect) {
        renderer.fill_rect(content, Default::default(), self.color());
    }

    impl_widget_any!();
}

/// A swatch element, ready for the `ui!` macro as `{swatch()}`.
fn swatch() -> Element<Msg> {
    let palette = vec![
        Color::from_packed_rgba(0xe63946ff),
        Color::from_packed_rgba(0xf1faeeff),
        Color::from_packed_rgba(0xa8dadcff),
        Color::from_packed_rgba(0x457b9dff),
        Color::from_packed_rgba(0x1d3557ff),
    ];
    Element::new("swatch", Swatch::new(palette, Msg::Picked))
}

fn hex(color: Color) -> String {
    let [r, g, b, _] = color.to_rgba8();
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn main() {
    let mut ui: Ui<Msg> = Ui::new();
    // The tag is what the stylesheet sees, like any built-in element.
    ui.add_stylesheet(
        Stylesheet::parse(
            "swatch { padding: 1px; border: 1px solid #1c1c1c; }
             swatch:hover { border-color: #5b9bd5; }
             swatch:focus-visible { border-color: #ffffff; }",
        )
        .unwrap(),
    );
    ui.set_root(ui! {
        <row class="panel" align_items="center">
            <label>"Colour"</label>
            {swatch().attr("id", "picker").attr("index", 3)}
            <label class="dim">"click or press Space"</label>
        </row>
    });

    let picker = ui.find("picker").unwrap();
    let fonts = MonospaceFonts::default();
    let viewport = Size::new(240.0, 40.0);
    ui.layout(viewport, &fonts);
    println!(
        "swatch starts at {} in box {:?}",
        hex(ui.widget::<Swatch<Msg>>(picker).unwrap().color()),
        ui.rect(picker)
    );

    let center = ui.rect(picker).center();
    let script = [
        (
            "click",
            Event::PointerDown {
                position: center,
                button: PointerButton::Primary,
            },
        ),
        (
            "",
            Event::PointerUp {
                position: center,
                button: PointerButton::Primary,
            },
        ),
        (
            "click",
            Event::PointerDown {
                position: center,
                button: PointerButton::Primary,
            },
        ),
        (
            "",
            Event::PointerUp {
                position: center,
                button: PointerButton::Primary,
            },
        ),
        (
            "Space",
            Event::KeyDown {
                key: Key::Space,
                modifiers: Default::default(),
            },
        ),
    ];
    for (what, event) in script {
        if !what.is_empty() {
            println!("{what}");
        }
        for message in ui.handle(&event, &fonts) {
            match message {
                Msg::Picked(color) => println!("  -> picked {}", hex(color)),
            }
        }
        ui.layout(viewport, &fonts);
    }

    // The border colour follows the stylesheet: the pointer is still over
    // the swatch, so the hover rule applies.
    println!("border now {}", hex(ui.style(picker).border_color.top));

    let mut renderer = RecordingRenderer::default();
    ui.paint(&mut renderer);
    println!("{} draw calls", renderer.ops.len());
}
