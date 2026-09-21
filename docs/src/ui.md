# The UI toolkit

`crates/eternal-ui` is the toolkit Eternal Pixels is built with. It is made
for tools: dense screens, strong hierarchy without decoration, keyboard and
mouse as equals. The look is in the spirit of Aseprite and is nothing but a
stylesheet.

## The pieces

```
eternal-styler             CSS: parsing, selectors, cascade, computed styles
  └─ eternal-ui            elements, layout, focus, events, the ui! macro
       └─ eternal-ui-skia  draws it with Skia
```

The toolkit never draws anything itself. It measures text through the
`Fonts` trait and paints through the `Renderer` trait, and a backend crate
implements both for a particular graphics library. `eternal-ui-skia` does it
for Skia, which is what the application uses. A Vello backend, or a
tiny-skia one with its own text rasteriser, would implement the same two
traits and nothing else. `MonospaceFonts` and `RecordingRenderer` in the
toolkit itself are stand-ins for tests and for laying out without a window.

Windowing is not part of the toolkit either. The application feeds it
`Event`s from winit (or anything else), tells it the window size, and hands
it a canvas to paint on.

## A frame

```rust
// Events in, messages out.
for event in window_events {
    for message in ui.handle(&event, &fonts) {
        app.update(message, &mut ui);
    }
}
// Then layout and paint.
ui.layout(window_size, &fonts);
ui.paint(&mut renderer);
```

`handle` routes pointer events to the deepest element under the pointer
and lets them bubble up; key events go first to window-wide shortcuts
(`Ui::bind`), then to element shortcuts (the `shortcut` attribute), then to
the focused element, and finally Tab and Shift+Tab move focus. Handlers
produce messages of the application's own type and `handle` returns them,
so every decision about what happens lives in `app.update`, in one place.

`layout` recomputes styles if anything changed (an event, a class toggled,
a widget edited), then sizes and places every element. `paint` walks the
tree and for each element draws the background, the border, the widget's
own content, then the children.

## Composition is declarative

The `ui!` macro is HTML inside Rust:

```rust
let root: Element<Msg> = ui! {
    <column class="panel">
        <row class="toolbar">
            <button id="save" shortcut="Ctrl+S" on:click={|| Msg::Save}>"Save"</button>
            <spacer/>
            <label class="dim">{format!("Editing {name}")}</label>
        </row>
        <input value={name} placeholder="File name" on:change={Msg::Rename}/>
        <checkbox checked="true" on:toggle={Msg::Wrap}>"Wrap around"</checkbox>
    </column>
};
ui.set_root(root);
```

A tag is one of the constructors in `elements`; `Element::new` makes an
element from your own `Widget`, and `{expr}` children let you drop it in.
Attributes are `name="literal"` or `name={expr}`; `id`, `class`, `style`,
`grow`, `align`, `disabled`, `shortcut` and `state` are the framework's,
the rest go to the widget. `on:click={...}` attaches a handler; the name is
a function in the `on` module and the closure has the matching signature.
A closing tag that does not match its opening tag is a type error. The
macro is `macro_rules!`, and each token costs one level of recursion, so a
very large template may need `#![recursion_limit = "512"]`.

Text inside a `label` is its text; text anywhere else becomes a child
label, so `<button>"Save"</button>` is a button containing a label, and
`button label { ... }` styles it.

## Behaviour is imperative

Widgets are plain structs. `Ui::widget_mut::<Label>(id)` gives you the one
behind an element and you call `set_text` on it. `Ui::set_class`,
`set_disabled`, `set_state`, `set_attr`, `set_inline_style`, `mount` and
`remove` change the tree in place. Nothing is diffed or re-rendered: change
what you mean to change, and the next `layout` picks it up.

## Styling is data

Everything an element looks like comes from CSS through eternal-styler:
`Ui::new` starts with the built-in theme in
`crates/eternal-ui/themes/pixel-dark.css`; `Ui::set_theme` replaces it and
`Ui::add_stylesheet` layers on top; `load_stylesheet` reads a file. An
element's `style` attribute is inline CSS, and the `Style` builder writes
inline CSS from Rust values for the computed cases.

Selectors match on the tag (`button`, `input`, `panel`), `#id`, `.class`,
attributes, and pseudo-classes: `:hover`, `:active`, `:focus`,
`:focus-visible`, `:focus-within`, `:disabled`, `:enabled`, `:checked`,
`:placeholder-shown`, the tree-structural ones, and `:state(name)` for
states the application sets with `Ui::set_state`. A checkbox's box is a
child element with the tag `check`, so `checkbox:checked > check` styles
the checked look, and a disabled button is `button:disabled`. There is no
styling API in Rust beyond that on purpose: a theme is a file anyone can
edit.

## Layout

Containers lay their children out along one axis, like a CSS flex row or
column that never wraps. Sizes, margins, padding, borders and gaps come from
the stylesheet. Structure comes from attributes: `grow="1"` on a child
takes a share of the spare room along the axis (that is all a `spacer` is),
`align` on a child places it across its parent's axis and `align_items` on
a container sets the default for all its children (`start`, `center`,
`end`, `stretch`), and `justify` on a container spreads them along the axis
when nothing grows (`start`, `center`, `end`, `space-between`).

`width` and `height` are border-box sizes. Percentages are of the parent's
content box. The root always fills the window. There is no wrapping,
scrolling or overflow clipping yet; a text field clips its own content and
that is all.

## Adding a widget

Implement `Widget<M>` for a struct: `measure` for its natural size,
`event` for what it does with input, `paint` for its own content,
`focusable` if it takes focus, `state` for any state it owns that CSS
should see, and `set_attribute` for the attributes it accepts. Backgrounds,
borders, layout and children are the framework's job. Then
`Element::new("my-tag", MyWidget::new())` puts it in a tree, and the tag is
what the stylesheet matches.

## The Skia backend

`SkiaFonts` looks typefaces up through the system font manager by the CSS
family names in a style, with the generic families mapped to common system
fonts. Fonts of your own, such as a pixel font shipped with the
application, are loaded with `SkiaFonts::load_font_file` (or `load_font`
from bytes, for `include_bytes!`), optionally under a family name of your
choosing with the `_as` variants, and are then matched ahead of system
fonts by `font-family`. `SkiaRenderer` draws onto any `skia_safe::Canvas`.
Shapes are drawn without anti-aliasing and text without subpixel
positioning so that 1px borders and pixel fonts stay crisp;
`SkiaFonts::set_smooth` turns anti-aliasing on.

skia-safe fetches a prebuilt Skia for the target the first time it builds,
which crate_universe runs inside Bazel's sandbox; the sandbox allows
network access on macOS, so this just works. The tests render to a raster
surface and read pixels back, so they run headless.

## Examples

- `bazel run //crates/eternal-ui:headless_example` runs a whole
  application loop without a window: markup, a script of events, the
  messages they produce, and the draw calls of a frame.
- `bazel run //crates/eternal-ui:custom_widget_example` adds a widget of
  its own, a colour swatch, styled by its tag.
- `bazel run //crates/eternal-ui-skia:render_png_example` renders a
  showcase of the theme, with widgets hovered, pressed, focused, checked
  and disabled, to `eternal-ui.png` in the current directory. The quickest
  way to see what a theme change looks like. A second argument names a
  font file to set everything in.

The same files are `cargo run --example <name>` in each crate.

## What is left out

Windowing, scrolling, wrapping, text selection and the clipboard, menus,
tooltips, images and icons, animation, and any layout beyond one-axis
containers. Each is meant to be built on what is here rather than to change
it.
