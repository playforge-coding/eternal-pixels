// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A whole application loop without a window: build a tree with the `ui!`
//! macro, feed it a script of events, act on the messages, and show what a
//! frame would draw.
//!
//! Run with `bazel run //crates/eternal-ui:headless_example` or
//! `cargo run --example headless`.

use eternal_ui::widgets::{Label, TextInput};
use eternal_ui::{
    DrawOp, Element, Event, Key, Modifiers, MonospaceFonts, NodeId, PointerButton,
    RecordingRenderer, Shortcut, Size, Ui, ui,
};

/// Everything the interface can ask the application to do.
#[derive(Clone, Debug)]
enum Msg {
    Save,
    Quit,
    Rename(String),
    Wrap(bool),
}

/// The application: some state and an `update` that acts on messages.
struct App {
    file_name: String,
    wrap: bool,
    saves: u32,
    quit: bool,
}

impl App {
    fn view(&self) -> Element<Msg> {
        ui! {
            <column class="panel" id="root">
                <row class="toolbar">
                    <button id="save" shortcut="Ctrl+S" on:click={|| Msg::Save}>"Save"</button>
                    <button id="quit" on:click={|| Msg::Quit}>"Quit"</button>
                    <spacer/>
                    <label id="status" class="dim">"Ready"</label>
                </row>
                <row>
                    <label>"File name"</label>
                    <input id="name" value={&self.file_name} placeholder="untitled"
                           on:change={Msg::Rename}/>
                </row>
                <checkbox id="wrap" checked={self.wrap} on:toggle={Msg::Wrap}>"Wrap around"</checkbox>
            </column>
        }
    }

    /// Acts on a message and updates the interface in place.
    fn update(&mut self, message: Msg, ui: &mut Ui<Msg>, status: NodeId) {
        let text = match message {
            Msg::Save => {
                self.saves += 1;
                format!(
                    "Saved {} as {} ({} so far)",
                    "sketch", self.file_name, self.saves
                )
            }
            Msg::Quit => {
                self.quit = true;
                "Quitting".to_owned()
            }
            Msg::Rename(name) => {
                self.file_name = name;
                format!("File name is now {:?}", self.file_name)
            }
            Msg::Wrap(wrap) => {
                self.wrap = wrap;
                format!("Wrap around: {wrap}")
            }
        };
        ui.widget_mut::<Label>(status).unwrap().set_text(text);
    }
}

fn main() {
    let mut app = App {
        file_name: "sketch.png".to_owned(),
        wrap: true,
        saves: 0,
        quit: false,
    };
    let mut ui = Ui::new();
    ui.set_root(app.view());
    ui.bind(Shortcut::parse("Ctrl+Q").unwrap(), || Msg::Quit);

    // A backend would supply fonts and a renderer; these stand-ins measure
    // every glyph the same and record draw calls instead of drawing.
    let fonts = MonospaceFonts::default();
    let viewport = Size::new(320.0, 120.0);
    ui.layout(viewport, &fonts);

    let status = ui.find("status").unwrap();
    let save = ui.find("save").unwrap();
    let name = ui.find("name").unwrap();
    println!("== the tree, as laid out ==");
    print!("{ui:?}");

    // A script of user input, as a windowing layer would deliver it.
    let save_center = ui.rect(save).center();
    let name_center = ui.rect(name).center();
    let script: Vec<(&str, Event)> = vec![
        (
            "hover the Save button",
            Event::PointerMove {
                position: save_center,
            },
        ),
        (
            "press",
            Event::PointerDown {
                position: save_center,
                button: PointerButton::Primary,
            },
        ),
        (
            "release",
            Event::PointerUp {
                position: save_center,
                button: PointerButton::Primary,
            },
        ),
        (
            "click the file name field",
            Event::PointerDown {
                position: name_center,
                button: PointerButton::Primary,
            },
        ),
        (
            "",
            Event::PointerUp {
                position: name_center,
                button: PointerButton::Primary,
            },
        ),
        (
            "Home moves the caret to the start",
            Event::KeyDown {
                key: Key::Home,
                modifiers: Modifiers::empty(),
            },
        ),
        ("type", Event::Text("new-".to_owned())),
        (
            "Tab to the checkbox",
            Event::KeyDown {
                key: Key::Tab,
                modifiers: Modifiers::empty(),
            },
        ),
        (
            "Space toggles it",
            Event::KeyDown {
                key: Key::Space,
                modifiers: Modifiers::empty(),
            },
        ),
        (
            "Ctrl+S, the button's shortcut",
            Event::KeyDown {
                key: Key::Char('s'),
                modifiers: Modifiers::CTRL,
            },
        ),
        (
            "Ctrl+Q, a window-wide shortcut",
            Event::KeyDown {
                key: Key::Char('q'),
                modifiers: Modifiers::CTRL,
            },
        ),
    ];

    println!("\n== events in, messages out ==");
    for (what, event) in script {
        if !what.is_empty() {
            println!("{what}");
        }
        for message in ui.handle(&event, &fonts) {
            println!("  -> {message:?}");
            app.update(message, &mut ui, status);
        }
        ui.layout(viewport, &fonts);
    }

    println!("\n== state afterwards ==");
    println!(
        "status label: {:?}",
        ui.widget::<Label>(status).unwrap().text()
    );
    println!(
        "name field:   {:?}",
        ui.widget::<TextInput>(name).unwrap().text()
    );
    println!("focused:      {:?}", ui.focused().map(|id| ui.tag(id)));
    println!("app quit:     {}", app.quit);

    println!("\n== what a frame would draw ==");
    let mut renderer = RecordingRenderer::default();
    ui.paint(&mut renderer);
    let texts = renderer
        .ops
        .iter()
        .filter(|op| matches!(op, DrawOp::Text { .. }))
        .count();
    println!("{} draw calls, {} of them text", renderer.ops.len(), texts);
    for op in renderer.ops.iter().take(6) {
        println!("  {op:?}");
    }
    println!("  ...");
}
