# eternal-ui

A small, dense, keyboard-first UI toolkit for tools, in the spirit of
Aseprite's interface. The look lives entirely in CSS, composition is
declarative, behaviour is imperative, and rendering is pluggable.

```rust
use eternal_ui::{ui, Element, Event, Key, Modifiers, Size, Ui};
use eternal_ui::widgets::Label;

#[derive(Clone, Debug, PartialEq)]
enum Msg { Save, Rename(String) }

let root: Element<Msg> = ui! {
    <column class="panel">
        <row class="toolbar">
            <button id="save" shortcut="Ctrl+S" on:click={|| Msg::Save}>"Save"</button>
            <spacer/>
            <label id="status" class="dim">"Ready"</label>
        </row>
        <input placeholder="File name" on:change={Msg::Rename}/>
    </column>
};

let mut ui = Ui::new();
ui.set_root(root);

// Each frame: events in, messages out, then layout and paint. `fonts` and
// `renderer` come from a backend crate such as eternal-ui-skia.
ui.layout(Size::new(320.0, 200.0), &fonts);
for message in ui.handle(&event, &fonts) {
    match message {
        Msg::Save => { /* ... */ }
        Msg::Rename(name) => { /* ... */ }
    }
}
ui.paint(&mut renderer);

// Behaviour is imperative: reach in and change things.
let status = ui.find("status").unwrap();
ui.widget_mut::<Label>(status).unwrap().set_text("Saved");
```

## What is in the box

- **Elements**: `column`, `row`, `panel`, `label`, `button`, `checkbox`,
  `input`, `separator`, `spacer`, and your own via `Element::new` with a
  `Widget` impl.
- **The `ui!` macro**: HTML-like markup inside Rust. Attributes are
  `name="literal"` or `name={expr}`, handlers are `on:click={closure}`,
  children are tags, string literals or `{expr}`.
- **Styling** through [eternal-styler](https://crates.io/crates/eternal-styler):
  a built-in dark theme, stylesheets from files, `style` attributes, and a
  `Style` builder for computed inline CSS. Selectors match on tag, `id`,
  `class`, attributes and the usual pseudo-classes: `:hover`, `:active`,
  `:focus`, `:focus-visible`, `:disabled`, `:checked`,
  `:placeholder-shown` and `:state(name)` for states you define.
- **Layout**: one-axis containers, like flex rows and columns. Sizes,
  spacing and borders come from CSS; `grow`, `align` and `justify`
  attributes decide how spare room is used.
- **Keyboard**: Tab and Shift+Tab move focus, Enter and Space activate,
  buttons take a `shortcut="Ctrl+S"` attribute, and `Ui::bind` adds
  window-wide shortcuts. `Shortcut` parses and prints the menu spelling.
- **Messages**: handlers return values of your message type; `Ui::handle`
  hands them back, so all logic is in your code.
- **Backends**: the toolkit draws through the `Renderer` and `Fonts`
  traits. `eternal-ui-skia` implements them with Skia. A Vello or
  tiny-skia backend would implement the same two traits; `MonospaceFonts`
  and `RecordingRenderer` are stand-ins for tests.

Windowing is not included. Feed `Event`s from winit or anything else, give
`layout` the window size, and paint into whatever surface your backend
draws on.

## Licence

MPL-2.0.
