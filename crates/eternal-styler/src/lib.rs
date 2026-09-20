// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A small CSS engine for UI toolkits.
//!
//! Give it stylesheets and a tree of widgets; get back a
//! [`ComputedStyle`] for each widget with its dimensions, spacing, colours,
//! borders and typography resolved.
//!
//! The selector matching and CSS parsing come from Servo: the
//! [`selectors`](https://crates.io/crates/selectors) and
//! [`cssparser`](https://crates.io/crates/cssparser) crates that Stylo, the
//! style system in Firefox and Servo, is built on. Everything above that
//! (the property set, the cascade, computed values) is this crate's own, and
//! it is deliberately small: it covers what a toolkit needs to style widgets
//! and nothing a web page would.
//!
//! # What is supported
//!
//! - **Selectors**: types, classes, ids, attributes, the descendant, child
//!   and sibling combinators, `:not()`, `:is()`, `:where()`, `:has()`,
//!   `:root`, `:empty`, the `:nth-*` family and the other tree-structural
//!   pseudo-classes, the interactive pseudo-classes listed on
//!   [`ElementState`], and `:state(name)` for a toolkit's own states.
//! - **Properties**: `width`, `height`, `min-*`, `max-*`, `margin`,
//!   `padding`, `gap`, `color`, `background-color`, `opacity`, the border
//!   width, style, colour and radius properties, `font-family`,
//!   `font-size`, `font-weight`, `font-style`, `line-height`,
//!   `letter-spacing` and `text-align`, with their shorthands. See
//!   [`LonghandId`] and [`ShorthandId`] for the full list.
//! - **Values**: `px`, `em`, `rem`, `pt` and the other absolute units,
//!   percentages, every CSS colour syntax up to `oklch()` and
//!   `color(display-p3 ...)`, `!important`, and the `inherit`, `initial`
//!   and `unset` keywords.
//!
//! Not supported: at-rules (`@media`, `@import`, ...), nesting, pseudo-elements,
//! `calc()`, custom properties, transitions, and every layout property.
//! Layout is the toolkit's job; this crate only hands it numbers.
//!
//! # Example
//!
//! A toolkit implements [`Element`] for a handle to its nodes, parses its
//! stylesheets, and asks a [`Styler`] for each node's style, top-down so
//! that each parent's style is available for its children.
//!
//! ```
//! use eternal_styler::{Element, ElementState, OpaqueElement, Styler, Stylesheet, Color};
//! use eternal_styler::LengthPercentage;
//!
//! // A minimal widget tree: an arena of nodes.
//! #[derive(Debug)]
//! struct Node { name: &'static str, class: Option<&'static str>, hovered: bool, parent: Option<usize>, children: Vec<usize> }
//! #[derive(Debug)]
//! struct Tree(Vec<Node>);
//!
//! #[derive(Clone, Debug)]
//! struct Handle<'a> { tree: &'a Tree, index: usize }
//!
//! impl<'a> Handle<'a> {
//!     fn node(&self) -> &'a Node { &self.tree.0[self.index] }
//!     fn at(&self, index: usize) -> Self { Handle { tree: self.tree, index } }
//! }
//!
//! impl Element for Handle<'_> {
//!     fn opaque(&self) -> OpaqueElement { OpaqueElement::new(self.node()) }
//!     fn parent(&self) -> Option<Self> { self.node().parent.map(|i| self.at(i)) }
//!     fn first_child(&self) -> Option<Self> { self.node().children.first().map(|&i| self.at(i)) }
//!     fn prev_sibling(&self) -> Option<Self> { None } // a real toolkit would look these up
//!     fn next_sibling(&self) -> Option<Self> { None }
//!     fn local_name(&self) -> &str { self.node().name }
//!     fn classes(&self) -> impl Iterator<Item = &str> { self.node().class.into_iter() }
//!     fn state(&self) -> ElementState {
//!         if self.node().hovered { ElementState::HOVER } else { ElementState::empty() }
//!     }
//! }
//!
//! let tree = Tree(vec![
//!     Node { name: "panel", class: None, hovered: false, parent: None, children: vec![1] },
//!     Node { name: "button", class: Some("primary"), hovered: true, parent: Some(0), children: vec![] },
//! ]);
//!
//! let mut styler = Styler::new();
//! styler.add_stylesheet(Stylesheet::parse(r#"
//!     panel { font-size: 20px; color: #333; }
//!     button { padding: 0.5em 1em; border: 1px solid; }
//!     button.primary:hover { background-color: #06f; color: white; }
//! "#)?);
//!
//! let panel = Handle { tree: &tree, index: 0 };
//! let panel_style = styler.compute(&panel, None);
//! let button_style = styler.compute(&panel.at(1), Some(&panel_style));
//!
//! assert_eq!(button_style.font_size, 20.0);                       // inherited
//! assert_eq!(button_style.padding.left, LengthPercentage::Px(20.0)); // 1em of 20px
//! assert_eq!(button_style.background_color, Color::from_packed_rgba(0x0066ffff));
//! assert_eq!(button_style.border_color.top, Color::WHITE);        // currentcolor
//! # Ok::<(), eternal_styler::Error>(())
//! ```

mod atom;
mod cascade;
mod computed;
mod element;
mod error;
mod properties;
mod selector;
pub mod specified;
mod styler;
mod stylesheet;
mod values;

pub use selectors::OpaqueElement;

pub use atom::Atom;
pub use computed::{
    ComputedStyle, LengthPercentage, LengthPercentageOrAuto, LengthPercentageOrNone, LineHeight,
};
pub use element::{Element, ElementState, opaque_element_from_index};
pub use error::{Error, ErrorKind};
pub use properties::{CssWideKeyword, LonghandId, PropertyDeclaration, PropertyId, ShorthandId};
pub use selector::{PseudoClass, PseudoElement, Selector, SelectorImpl, SelectorList};
pub use styler::Styler;
pub use stylesheet::{Declaration, DeclarationBlock, StyleRule, Stylesheet};
pub use values::{
    BorderStyle, Color, Corners, FontFamily, FontStyle, FontWeight, Sides, TextAlign,
};
