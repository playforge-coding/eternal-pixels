// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! End to end: markup in, layout, events, messages and draw calls out.

use eternal_styler::{Color, ElementState, Stylesheet};
use eternal_ui::widgets::{Checkbox, Label, TextInput};
use eternal_ui::{
    DrawOp, Element, Event, Key, Modifiers, MonospaceFonts, NodeId, Point, PointerButton,
    RecordingRenderer, Rect, Shortcut, Size, Style, Ui, ui,
};

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Save,
    Quit,
    Wrap(bool),
    Named(String),
    Submitted(String),
}

/// A predictable stylesheet: 10px monospace text, so a glyph is 6px wide
/// and a line is 12.5px tall with `MonospaceFonts`.
const CSS: &str = "
    * { font-size: 10px; }
    column { padding: 4px; gap: 4px; }
    button { padding: 2px 6px; border: 1px solid #000000; background-color: #333333; }
    button:hover { background-color: #444444; }
    button:active { background-color: #222222; }
    button:focus-visible { border-color: #5b9bd5; }
    button:disabled { background-color: #111111; }
    checkbox { gap: 4px; }
    check { width: 10px; height: 10px; border: 1px solid #000000; }
    checkbox:checked > check { background-color: #5b9bd5; }
    input { min-width: 60px; border: 1px solid #000000; padding: 2px; }
    input:focus { border-color: #5b9bd5; }
    .accent { color: #ff0000; }
";

fn tree() -> Element<Msg> {
    ui! {
        <column id="root">
            <row id="toolbar">
                <button id="save" shortcut="Ctrl+S" on:click={|| Msg::Save}>"Save"</button>
                <spacer/>
                <button id="quit" on:click={|| Msg::Quit}>"Quit"</button>
            </row>
            <checkbox id="wrap" checked="true" on:toggle={Msg::Wrap}>"Wrap"</checkbox>
            <input id="name" value="a" on:change={Msg::Named} on:submit={Msg::Submitted}/>
            <label id="status" class="dim">"Ready"</label>
        </column>
    }
}

fn setup() -> (Ui<Msg>, MonospaceFonts) {
    let mut ui = Ui::unstyled();
    ui.add_stylesheet(Stylesheet::parse(CSS).unwrap());
    ui.set_root(tree());
    let fonts = MonospaceFonts::default();
    ui.layout(Size::new(200.0, 100.0), &fonts);
    (ui, fonts)
}

fn hex(rgba: u32) -> Color {
    Color::from_packed_rgba(rgba)
}

fn press(ui: &mut Ui<Msg>, fonts: &MonospaceFonts, key: Key, modifiers: Modifiers) -> Vec<Msg> {
    ui.handle(&Event::KeyDown { key, modifiers }, fonts)
}

fn click(ui: &mut Ui<Msg>, fonts: &MonospaceFonts, at: Point) -> Vec<Msg> {
    let mut messages = ui.handle(
        &Event::PointerDown {
            position: at,
            button: PointerButton::Primary,
        },
        fonts,
    );
    messages.extend(ui.handle(
        &Event::PointerUp {
            position: at,
            button: PointerButton::Primary,
        },
        fonts,
    ));
    messages
}

#[test]
fn markup_builds_the_tree() {
    let (ui, _) = setup();
    let root = ui.root().unwrap();
    assert_eq!(ui.tag(root), "column");
    assert_eq!(ui.find("root"), Some(root));
    let toolbar = ui.find("toolbar").unwrap();
    assert_eq!(ui.children(root)[0], toolbar);
    let tags: Vec<&str> = ui.children(toolbar).iter().map(|&c| ui.tag(c)).collect();
    assert_eq!(tags, ["button", "spacer", "button"]);

    // Text inside a button becomes a label; inside a label it is the text.
    let save = ui.find("save").unwrap();
    let save_label = ui.children(save)[0];
    assert_eq!(ui.tag(save_label), "label");
    assert_eq!(ui.widget::<Label>(save_label).unwrap().text(), "Save");
    let status = ui.find("status").unwrap();
    assert!(ui.children(status).is_empty());
    assert_eq!(ui.widget::<Label>(status).unwrap().text(), "Ready");
    assert!(ui.has_class(status, "dim"));

    // A checkbox is a check box plus its label.
    let wrap = ui.find("wrap").unwrap();
    assert_eq!(ui.tag(ui.children(wrap)[0]), "check");
    assert!(ui.widget::<Checkbox>(wrap).unwrap().is_checked());
    assert_eq!(
        ui.widget::<TextInput>(ui.find("name").unwrap())
            .unwrap()
            .text(),
        "a"
    );
}

#[test]
fn layout_follows_the_stylesheet() {
    let (ui, _) = setup();
    let toolbar = ui.find("toolbar").unwrap();
    let save = ui.find("save").unwrap();
    let quit = ui.find("quit").unwrap();

    // The column pads by 4 and stretches the row to its width.
    assert_eq!(ui.rect(toolbar).x, 4.0);
    assert_eq!(ui.rect(toolbar).y, 4.0);
    assert_eq!(ui.rect(toolbar).width, 192.0);

    // "Save" is 4 glyphs of 6px plus 2 * (6 + 1) of padding and border.
    let save_rect = ui.rect(save);
    assert_eq!(save_rect, Rect::new(4.0, 4.0, 38.0, 18.5));
    assert_eq!(ui.content_rect(save), Rect::new(11.0, 7.0, 24.0, 12.5));

    // The spacer pushes Quit to the right edge.
    assert_eq!(ui.rect(quit).right(), 196.0);
    assert_eq!(ui.rect(quit).width, 38.0);

    // The next row starts after the toolbar plus the gap.
    let wrap = ui.find("wrap").unwrap();
    assert_eq!(ui.rect(wrap).y, 4.0 + 18.5 + 4.0);
    let check = ui.children(wrap)[0];
    assert_eq!(ui.rect(check).size(), Size::new(10.0, 10.0));

    // min-width applies to the input.
    assert_eq!(ui.rect(ui.find("name").unwrap()).width, 192.0); // stretched
    let mut narrow: Ui<Msg> = Ui::unstyled();
    narrow.add_stylesheet(Stylesheet::parse(CSS).unwrap());
    narrow.set_root(ui! { <row><input/></row> });
    narrow.layout(Size::new(200.0, 100.0), &MonospaceFonts::default());
    let input = narrow.children(narrow.root().unwrap())[0];
    assert_eq!(narrow.rect(input).width, 60.0);
}

#[test]
fn pointer_clicks_hover_and_press() {
    let (mut ui, fonts) = setup();
    let save = ui.find("save").unwrap();
    let center = ui.rect(save).center();

    assert_eq!(ui.style(save).background_color, hex(0x333333ff));
    assert!(
        ui.handle(&Event::PointerMove { position: center }, &fonts)
            .is_empty()
    );
    ui.layout(Size::new(200.0, 100.0), &fonts);
    assert_eq!(ui.hovered(), Some(ui.children(save)[0])); // the label is on top
    assert!(ui.state(save).contains(ElementState::HOVER)); // and hover bubbles
    assert_eq!(ui.style(save).background_color, hex(0x444444ff));

    let down = Event::PointerDown {
        position: center,
        button: PointerButton::Primary,
    };
    assert!(ui.handle(&down, &fonts).is_empty());
    ui.layout(Size::new(200.0, 100.0), &fonts);
    assert_eq!(ui.style(save).background_color, hex(0x222222ff));
    assert_eq!(ui.focused(), Some(save)); // clicking focuses
    assert!(!ui.state(save).contains(ElementState::FOCUS_VISIBLE)); // quietly

    let up = Event::PointerUp {
        position: center,
        button: PointerButton::Primary,
    };
    assert_eq!(ui.handle(&up, &fonts), vec![Msg::Save]);

    // Releasing elsewhere is not a click.
    assert!(ui.handle(&down, &fonts).is_empty());
    let away = Event::PointerUp {
        position: Point::new(199.0, 99.0),
        button: PointerButton::Primary,
    };
    assert!(ui.handle(&away, &fonts).is_empty());

    // Clicking empty space blurs.
    click(&mut ui, &fonts, Point::new(199.0, 99.0));
    assert_eq!(ui.focused(), None);
}

#[test]
fn keyboard_is_first_class() {
    let (mut ui, fonts) = setup();
    let save = ui.find("save").unwrap();
    let quit = ui.find("quit").unwrap();
    let wrap = ui.find("wrap").unwrap();
    let name = ui.find("name").unwrap();

    // The button's own shortcut.
    assert_eq!(
        press(&mut ui, &fonts, Key::Char('s'), Modifiers::CTRL),
        vec![Msg::Save]
    );
    assert!(press(&mut ui, &fonts, Key::Char('s'), Modifiers::empty()).is_empty());

    // A window-wide shortcut.
    ui.bind(Shortcut::parse("Ctrl+Q").unwrap(), || Msg::Quit);
    assert_eq!(
        press(&mut ui, &fonts, Key::Char('q'), Modifiers::CTRL),
        vec![Msg::Quit]
    );
    assert_eq!(ui.shortcuts().len(), 2);

    // Tab walks the focusable elements in order and shows the focus ring.
    assert_eq!(ui.focused(), None);
    press(&mut ui, &fonts, Key::Tab, Modifiers::empty());
    assert_eq!(ui.focused(), Some(save));
    assert!(ui.state(save).contains(ElementState::FOCUS_VISIBLE));
    ui.layout(Size::new(200.0, 100.0), &fonts);
    assert_eq!(ui.style(save).border_color.top, hex(0x5b9bd5ff));
    press(&mut ui, &fonts, Key::Tab, Modifiers::empty());
    assert_eq!(ui.focused(), Some(quit));
    press(&mut ui, &fonts, Key::Tab, Modifiers::empty());
    assert_eq!(ui.focused(), Some(wrap));
    press(&mut ui, &fonts, Key::Tab, Modifiers::empty());
    assert_eq!(ui.focused(), Some(name));
    press(&mut ui, &fonts, Key::Tab, Modifiers::empty());
    assert_eq!(ui.focused(), Some(save)); // wraps
    press(&mut ui, &fonts, Key::Tab, Modifiers::SHIFT);
    assert_eq!(ui.focused(), Some(name));

    // Enter and Space activate buttons.
    ui.focus(quit);
    assert_eq!(
        press(&mut ui, &fonts, Key::Enter, Modifiers::empty()),
        vec![Msg::Quit]
    );
    assert_eq!(
        press(&mut ui, &fonts, Key::Space, Modifiers::empty()),
        vec![Msg::Quit]
    );

    // Space toggles a checkbox and the style follows.
    ui.focus(wrap);
    assert_eq!(
        press(&mut ui, &fonts, Key::Space, Modifiers::empty()),
        vec![Msg::Wrap(false)]
    );
    assert!(!ui.widget::<Checkbox>(wrap).unwrap().is_checked());
    ui.layout(Size::new(200.0, 100.0), &fonts);
    let check = ui.children(wrap)[0];
    assert_eq!(ui.style(check).background_color, Color::TRANSPARENT);
    let wrap_center = ui.rect(wrap).center();
    assert_eq!(click(&mut ui, &fonts, wrap_center), vec![Msg::Wrap(true)]);
    ui.layout(Size::new(200.0, 100.0), &fonts);
    assert_eq!(ui.style(check).background_color, hex(0x5b9bd5ff));

    // Typing goes to the focused text field.
    ui.focus(name);
    assert_eq!(
        ui.handle(&Event::Text("bc".to_owned()), &fonts),
        vec![Msg::Named("abc".to_owned())]
    );
    assert_eq!(
        press(&mut ui, &fonts, Key::ArrowLeft, Modifiers::empty()),
        vec![]
    );
    assert_eq!(
        press(&mut ui, &fonts, Key::Backspace, Modifiers::empty()),
        vec![Msg::Named("ac".to_owned())]
    );
    assert_eq!(
        press(&mut ui, &fonts, Key::Enter, Modifiers::empty()),
        vec![Msg::Submitted("ac".to_owned())]
    );
    // Shortcuts still win while typing.
    assert_eq!(
        press(&mut ui, &fonts, Key::Char('s'), Modifiers::CTRL),
        vec![Msg::Save]
    );
    ui.layout(Size::new(200.0, 100.0), &fonts);
    assert_eq!(ui.style(name).border_color.top, hex(0x5b9bd5ff));
}

#[test]
fn disabled_elements_are_inert() {
    let (mut ui, fonts) = setup();
    let save = ui.find("save").unwrap();
    ui.set_disabled(save, true);
    ui.layout(Size::new(200.0, 100.0), &fonts);
    assert_eq!(ui.style(save).background_color, hex(0x111111ff));
    let save_center = ui.rect(save).center();
    assert!(click(&mut ui, &fonts, save_center).is_empty());
    assert!(press(&mut ui, &fonts, Key::Char('s'), Modifiers::CTRL).is_empty());
    assert!(!ui.focus(save));
    press(&mut ui, &fonts, Key::Tab, Modifiers::empty());
    assert_eq!(ui.focused(), Some(ui.find("quit").unwrap()));
    assert_eq!(ui.shortcuts().len(), 0);
}

#[test]
fn imperative_changes_show_up() {
    let (mut ui, fonts) = setup();
    let status = ui.find("status").unwrap();
    ui.widget_mut::<Label>(status).unwrap().set_text("Saved!");
    assert!(ui.needs_layout());
    ui.set_class(status, "accent", true);
    ui.layout(Size::new(200.0, 100.0), &fonts);
    assert_eq!(ui.style(status).color, hex(0xff0000ff));
    assert_eq!(ui.rect(status).height, 12.5);
    ui.set_class(status, "accent", false);
    assert!(ui.set_attr(status, "style", &Style::new().color("#00ff00").to_string()));
    assert!(!ui.set_attr(status, "style", "color: nope"));
    assert!(!ui.set_attr(status, "nonsense", "1"));
    ui.layout(Size::new(200.0, 100.0), &fonts);
    assert_eq!(ui.style(status).color, hex(0x00ff00ff));

    // Custom states are matched by :state().
    ui.add_stylesheet(Stylesheet::parse("label:state(busy) { opacity: 0.5 }").unwrap());
    ui.set_state(status, "busy", true);
    ui.layout(Size::new(200.0, 100.0), &fonts);
    assert_eq!(ui.style(status).opacity, 0.5);

    // Mounting and removing.
    let root = ui.root().unwrap();
    let extra: NodeId = ui.mount(root, ui! { <label id="extra">"More"</label> });
    ui.layout(Size::new(200.0, 100.0), &fonts);
    assert_eq!(ui.children(root).len(), 5);
    assert!(ui.rect(extra).y > ui.rect(status).y);
    ui.remove(extra);
    assert!(!ui.contains(extra));
    assert_eq!(ui.children(root).len(), 4);
    assert_eq!(ui.find("extra"), None);
}

#[test]
fn painting_records_the_expected_calls() {
    let (mut ui, fonts) = setup();
    ui.layout(Size::new(200.0, 100.0), &fonts);
    let mut renderer = RecordingRenderer::default();
    ui.paint(&mut renderer);

    let save = ui.find("save").unwrap();
    let save_rect = ui.rect(save);
    let background = renderer.ops.iter().position(|op| {
        matches!(op, DrawOp::FillRect { rect, color, .. } if *rect == save_rect && *color == hex(0x333333ff))
    });
    let border = renderer.ops.iter().position(|op| {
        matches!(op, DrawOp::Border { rect, widths, .. } if *rect == save_rect && widths.top == 1.0)
    });
    let text = renderer
        .ops
        .iter()
        .position(|op| matches!(op, DrawOp::Text { text, .. } if text == "Save"));
    assert!(background.unwrap() < border.unwrap());
    assert!(border.unwrap() < text.unwrap());
    if let DrawOp::Text { top_left, font, .. } = &renderer.ops[text.unwrap()] {
        assert_eq!(*top_left, Point::new(11.0, 7.0));
        assert_eq!(font.size, 10.0);
    }

    // The text field clips its content.
    assert!(
        renderer
            .ops
            .iter()
            .any(|op| matches!(op, DrawOp::PushClip(_)))
    );
}

#[test]
fn default_theme_styles_the_built_ins() {
    let mut ui: Ui<Msg> = Ui::new();
    ui.set_root(ui! {
        <panel>
            <row class="toolbar"><button class="primary">"Go"</button></row>
            <checkbox>"Check"</checkbox>
            <separator/>
            <input placeholder="Type"/>
        </panel>
    });
    let fonts = MonospaceFonts::default();
    ui.layout(Size::new(300.0, 200.0), &fonts);
    let root = ui.root().unwrap();
    assert!(ui.style(root).border_width.top > 0.0);
    let toolbar = ui.children(root)[0];
    let button = ui.children(toolbar)[0];
    assert_eq!(ui.style(button).font_size, 11.0);
    assert!(ui.style(button).background_color != ui.style(root).background_color);
    let separator = ui.children(root)[2];
    assert_eq!(ui.rect(separator).height, 1.0);
    let input = ui.children(root)[3];
    assert!(ui.state(input).contains(ElementState::PLACEHOLDER_SHOWN));
}
