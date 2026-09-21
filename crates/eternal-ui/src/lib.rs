// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A small, dense, keyboard-first UI toolkit.
//!
//! Eternal UI is built for tools: information-dense screens, strong
//! hierarchy without decoration, keyboard and mouse as equals. The look is
//! in the spirit of Aseprite and lives entirely in a stylesheet, so it is
//! easy to change.
//!
//! - **Declarative composition.** The [`ui!`] macro describes a tree in
//!   HTML-like markup, inside Rust.
//! - **Imperative behaviour.** Widgets are plain structs; reach them
//!   through [`Ui::widget_mut`] and call methods on them.
//! - **Explicit logic.** Handlers produce messages of your own type;
//!   [`Ui::handle`] returns them and your code decides what happens.
//! - **Data-driven styling.** Every colour, border, padding and font comes
//!   from CSS handled by [eternal-styler](eternal_styler), either the
//!   built-in theme, files you load, or inline `style` attributes built
//!   with [`Style`].
//! - **Shortcuts first.** Any button takes a `shortcut="Ctrl+S"`
//!   attribute; window-wide shortcuts are bound with [`Ui::bind`]; Tab
//!   moves focus; Enter and Space activate.
//! - **Pluggable rendering.** The toolkit draws through the [`Renderer`]
//!   and [`Fonts`] traits. `eternal-ui-skia` implements them with Skia;
//!   a Vello or tiny-skia backend would implement the same two traits.
//!
//! # Example
//!
//! ```
//! use eternal_ui::{ui, Element, Event, Key, Modifiers, MonospaceFonts, PointerButton, Size, Ui};
//! use eternal_ui::widgets::Label;
//!
//! #[derive(Clone, Debug, PartialEq)]
//! enum Msg { Save, Rename(String) }
//!
//! // Describe the tree.
//! let root: Element<Msg> = ui! {
//!     <column class="panel">
//!         <row class="toolbar">
//!             <button id="save" shortcut="Ctrl+S" on:click={|| Msg::Save}>"Save"</button>
//!             <spacer/>
//!             <label id="status" class="dim">"Ready"</label>
//!         </row>
//!         <input placeholder="File name" on:change={Msg::Rename}/>
//!     </column>
//! };
//!
//! let mut ui = Ui::new();
//! ui.set_root(root);
//!
//! // Each frame: events in, messages out, then layout and paint. The fonts
//! // and renderer come from a backend; this one only measures.
//! let fonts = MonospaceFonts::default();
//! ui.layout(Size::new(320.0, 200.0), &fonts);
//!
//! let messages = ui.handle(&Event::KeyDown { key: Key::Char('s'), modifiers: Modifiers::CTRL }, &fonts);
//! assert_eq!(messages, vec![Msg::Save]);
//!
//! // Behaviour is imperative: change what you like, when you like.
//! let status = ui.find("status").unwrap();
//! ui.widget_mut::<Label>(status).unwrap().set_text("Saved");
//! ```
//!
//! # The look
//!
//! The [default theme](default_theme) is dark, flat and dense. Replace it
//! with [`Ui::set_theme`], or add a stylesheet after it with
//! [`Ui::add_stylesheet`] to change parts. Element tags (`button`,
//! `input`, `panel`...), the `id` and `class` attributes, and the usual
//! pseudo-classes (`:hover`, `:active`, `:focus-visible`, `:disabled`,
//! `:checked`, `:state(name)`) are what selectors match on.
//!
//! # Layout
//!
//! Containers lay their children out along one axis. Sizes, spacing and
//! borders come from the stylesheet; `grow` and `align` attributes on
//! elements and `align` and `justify` on containers decide how spare room
//! is used. See the [`layout`] module.

mod element;
mod event;
mod geometry;
pub mod layout;
mod macros;
mod paint;
mod render;
mod shortcut;
mod style;
mod ui;
mod widget;
pub mod widgets;

/// The style engine, re-exported for convenience.
pub use eternal_styler as styler;

pub use element::{Child, Element, IntoChildren, elements, tags};
pub use event::{Event, Key, Modifiers, PointerButton};
pub use geometry::{Axis, Point, Rect, Size};
pub use layout::{Align, Justify};
pub use render::{
    DrawOp, FontSpec, Fonts, MonospaceFonts, RecordingRenderer, Renderer, TextMetrics,
};
pub use shortcut::{Shortcut, ShortcutParseError};
pub use style::{DEFAULT_THEME_CSS, LoadError, Style, default_theme, load_stylesheet};
pub use ui::{NodeId, Ui};
pub use widget::{EventCx, Handler, Handlers, PaintCx, Widget, on};
