# eternal-styler

A small CSS engine for UI toolkits. Give it stylesheets and a tree of widgets;
get back a computed style for each widget with its dimensions, spacing,
colours, borders and typography resolved.

The selector matching and CSS parsing are Servo's: the
[`selectors`](https://crates.io/crates/selectors) and
[`cssparser`](https://crates.io/crates/cssparser) crates that Stylo, the style
system in Firefox and Servo, is built on. Everything above that (the property
set, the cascade, computed values) is this crate's own and deliberately small.
It covers what a toolkit needs to style widgets and nothing a web page would.

```rust
use eternal_styler::{Styler, Stylesheet};

let mut styler = Styler::new();
styler.add_stylesheet(Stylesheet::parse(r#"
    panel { font-size: 20px; color: #333; }
    button { padding: 0.5em 1em; border: 1px solid; }
    button.primary:hover { background-color: #06f; color: white; }
"#)?);

// `panel` and `button` are handles into your widget tree that implement
// `eternal_styler::Element`. Walk the tree top-down so each parent's style is
// available for its children.
let panel_style = styler.compute(&panel, None);
let button_style = styler.compute(&button, Some(&panel_style));

assert_eq!(button_style.font_size, 20.0);                  // inherited
assert_eq!(button_style.padding.left.resolve(0.0), 20.0);  // 1em of 20px
```

## What is supported

- **Selectors**: types, classes, ids, attributes, the descendant, child and
  sibling combinators, `:not()`, `:is()`, `:where()`, `:has()`, `:root`,
  `:empty`, the `:nth-*` family and the other tree-structural pseudo-classes,
  `:hover`, `:active`, `:focus`, `:focus-visible`, `:focus-within`,
  `:enabled`, `:disabled`, `:checked`, `:indeterminate`, `:read-only`,
  `:read-write`, `:required`, `:optional`, `:valid`, `:invalid`,
  `:placeholder-shown`, and `:state(name)` for a toolkit's own states.
- **Properties**: `width`, `height`, `min-width`, `min-height`, `max-width`,
  `max-height`, `margin`, `padding`, `gap`, `color`, `background-color`,
  `opacity`, the border width, style, colour and radius properties,
  `font-family`, `font-size`, `font-weight`, `font-style`, `line-height`,
  `letter-spacing` and `text-align`, with their shorthands.
- **Values**: `px`, `em`, `rem`, `pt` and the other absolute units,
  percentages, every CSS colour syntax up to `oklch()` and
  `color(display-p3 ...)`, `!important`, and the `inherit`, `initial` and
  `unset` keywords.

Not supported: at-rules (`@media`, `@import`, ...), nesting, pseudo-elements,
`calc()`, custom properties, transitions, and every layout property. Layout is
the toolkit's job; this crate only hands it numbers.

## How it fits together

1. Implement `Element` for a handle to your widgets: identity, parent,
   siblings, first child, name, and optionally id, classes, attributes and
   an `ElementState` for the interactive pseudo-classes.
2. Parse your stylesheets with `Stylesheet::parse`. Errors carry a line and
   column; `parse_lenient` skips bad parts the way a browser would and hands
   you the errors alongside.
3. Add them to a `Styler` and call `compute` for each widget, passing the
   parent's computed style. Inline styles go through `compute_with_inline`.

Percentages are left as percentages in the computed style, because only layout
knows what they are a percentage of. `em` and `rem` are resolved to pixels.
`currentcolor` is resolved to the element's colour.

## Licence

MPL-2.0, like the Servo crates it is built on.
