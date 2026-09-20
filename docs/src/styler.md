# The styler

`crates/eternal-styler` is the CSS engine behind Eternal UI. It takes
stylesheets and a tree of widgets and produces a computed style for each
widget: dimensions, spacing, colours, borders and typography, resolved to
numbers a layout and a renderer can use.

## Why not Stylo itself

Stylo is the style system in Firefox and Servo. It is excellent and enormous:
hundreds of properties, every at-rule, animations, transitions, shadow DOM,
custom properties, and the machinery to restyle a document incrementally after
any change. Pulling it into a UI toolkit means pulling in all of that, plus its
build requirements, for a tree of a few hundred widgets that needs a dozen
properties.

So instead of Stylo, the styler uses the two crates Stylo is built on, and
stops there:

```
cssparser        tokenising and the rule/declaration grammar
  └─ selectors   selector parsing, specificity and matching
       └─ eternal-styler   properties, cascade, computed values
```

`cssparser` and `selectors` are maintained by Servo, are the same code Firefox
ships, and are small. Everything above them is this crate's own, and it is kept
deliberately small: what a toolkit needs to style widgets and nothing a web
page would.

## The layers

**Selectors.** `selectors` is generic over a `SelectorImpl` that says which
string types, pseudo-classes and pseudo-elements selectors are made of.
`selector.rs` provides one with a `PseudoClass` enum for the interactive states
(`:hover`, `:disabled`, `:checked` and so on) plus `:state(name)` for
toolkit-defined states, and no pseudo-elements at all. Tree-structural
pseudo-classes (`:first-child`, `:nth-of-type()`, `:empty`, `:root`, `:has()`)
are handled entirely by `selectors`.

**Elements.** `selectors` matches against anything implementing its `Element`
trait, which has thirty-odd methods, most of them about HTML and shadow DOM.
`element.rs` defines this crate's own smaller `Element` trait, the one a
toolkit implements, and an adapter that answers the big trait from the small
one. A toolkit provides identity, parent, siblings, first child, the element
name, and optionally id, classes, attributes and an `ElementState` bitset.

**Properties.** `properties.rs` lists every longhand in one table (name, value
type, whether it inherits, how to parse it) and a macro turns the table into
`LonghandId`, `PropertyDeclaration` and the lookup functions. Shorthands
(`margin`, `border`, `border-radius`, ...) are expanded into longhands at parse
time, so the cascade only ever sees longhands. `specified.rs` holds the value
types as written, with units still attached.

**Stylesheets.** `stylesheet.rs` drives `cssparser`'s rule and declaration
parsers. A stylesheet is a list of rules; a rule is a selector list and a
declaration block; a declaration is a longhand, its specified value and an
`!important` flag. Parse errors carry a line and column. `parse` fails on the
first problem; `parse_lenient` skips bad declarations and rules the way a
browser does and returns the problems alongside.

**The cascade.** `styler.rs` finds the rules matching an element, sorts them by
specificity then source order (a later stylesheet counts as later source), and
walks their declarations so that the last one for each longhand wins. Normal
declarations go first, then `!important` ones, with inline styles above the
stylesheets within each group. `cascade.rs` then turns the winners into a
`ComputedStyle`: inherited properties start from the parent's computed values,
the rest from their initial values; `font-size` is resolved first so `em` has
something to be relative to; `color` next so `currentcolor` does; then
everything else.

**Computed values.** `computed.rs` is what the toolkit reads. Lengths are in
pixels. Percentages remain percentages, since only layout knows what they are a
percentage of. `currentcolor` is gone, replaced by the element's colour. A
border side's width is zero if its style is `none` or `hidden`, as in CSS, so
layout can add border widths without checking styles.

## Using it from a toolkit

Walk the tree top-down, keeping each parent's `ComputedStyle` for its children:

```rust
fn style_subtree(styler: &Styler, node: &Handle, parent: Option<&ComputedStyle>) {
    let style = styler.compute(node, parent);
    for child in node.children() {
        style_subtree(styler, &child, Some(&style));
    }
    node.set_style(style);
}
```

`Styler::compute` takes `&self`, so a styler can be shared across threads once
its stylesheets are loaded. Recompute a subtree whenever an element's state,
classes or attributes change; there is no incremental restyling, and for a UI
tree there does not need to be.

`Styler::matching_rules` returns the rules that matched an element in cascade
order, which is what a style inspector wants.

## Adding a property

1. Add a specified value type to `specified.rs` if none of the existing ones
   fit, with a `parse` function.
2. Add a line to the `longhands!` table in `properties.rs`. If the property has
   a shorthand, add it to `ShorthandId`.
3. Add a field to `ComputedStyle` in `computed.rs`, set its initial value in
   `initial()`, and copy it in `inherit_from` if it inherits.
4. Add an `apply!` line in `cascade.rs` that turns the specified value into the
   computed one.
5. Mention it in the crate's README and in `lib.rs`.

The compiler will point out anything missed: the table is the single source of
truth for names, and `LonghandId` is matched exhaustively.

## Dependencies and Bazel

`cssparser`, `cssparser-color`, `selectors`, `precomputed-hash` and
`bitflags` come from crates.io. Bazel fetches them with rules_rust's
crate_universe, configured in `MODULE.bazel` to read
`crates/eternal-styler/Cargo.toml` and `Cargo.lock`, so the Cargo manifest is
the single place dependencies are declared. The resolved graph is written to
`third_party/cargo-bazel-lock.json` and must be regenerated whenever a
`Cargo.toml` or `Cargo.lock` changes:

```bash
CARGO_BAZEL_REPIN=1 bazel build //...
```

`selectors` pins a particular minor version of `cssparser`, so this crate has
to use the same one or there would be two copies of the parser in the build and
the selector parser would not accept our `Parser`. Check `selectors`'
`Cargo.toml` before bumping `cssparser`.

## What is left out

At-rules (`@media`, `@import`, `@font-face`), nesting, pseudo-elements,
`calc()`, custom properties, transitions and animations, and every layout
property (`display`, `flex-*`, `position`, ...). Layout is Eternal UI's job.
`border-radius` takes one radius per corner; elliptical corners (`a / b`) are
not parsed. Colours in `lab()`, `lch()`, `oklab()`, `oklch()` and
`color(display-p3 ...)` are converted to sRGB and clipped to its gamut; the
other `color()` spaces are rejected.
