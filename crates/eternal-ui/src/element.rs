// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The declarative description of an element, before it is mounted.

use eternal_styler::DeclarationBlock;

use crate::layout::Align;
use crate::shortcut::Shortcut;
use crate::widget::{Handler, Handlers, Widget};
use crate::widgets::{Blank, Button, Checkbox, Container, Label, TextInput, parse_bool};

/// An element waiting to be mounted: a widget, its attributes, handlers
/// and children.
///
/// Built by the [`ui!`](crate::ui) macro or by hand from the constructors
/// in [`elements`], then handed to [`Ui::set_root`](crate::Ui::set_root) or
/// [`Ui::mount`](crate::Ui::mount).
pub struct Element<M> {
    pub(crate) tag: &'static str,
    pub(crate) widget: Box<dyn Widget<M>>,
    pub(crate) id: Option<String>,
    pub(crate) classes: Vec<String>,
    pub(crate) attrs: Vec<(String, String)>,
    pub(crate) custom_states: Vec<String>,
    pub(crate) inline_style: Option<DeclarationBlock>,
    pub(crate) handlers: Handlers<M>,
    pub(crate) children: Vec<Element<M>>,
    pub(crate) grow: f32,
    pub(crate) align: Option<Align>,
    pub(crate) disabled: bool,
    pub(crate) shortcut: Option<Shortcut>,
}

impl<M: 'static> Element<M> {
    /// An element of your own widget type. `tag` is what selectors match it
    /// by, so use something distinct like `color-picker`.
    pub fn new(tag: &'static str, widget: impl Widget<M>) -> Self {
        Self {
            tag,
            widget: Box::new(widget),
            id: None,
            classes: Vec::new(),
            attrs: Vec::new(),
            custom_states: Vec::new(),
            inline_style: None,
            handlers: Handlers::default(),
            children: Vec::new(),
            grow: 0.0,
            align: None,
            disabled: false,
            shortcut: None,
        }
    }

    /// Sets an attribute, the way `name="value"` does in the macro.
    ///
    /// `id`, `class`, `style`, `grow`, `align`, `disabled`, `shortcut` and
    /// `state` are understood by the framework; anything else goes to the
    /// widget. An attribute nobody understands, an inline style that does
    /// not parse or a bad shortcut is a mistake in the template, and panics
    /// with a message saying which.
    ///
    /// # Panics
    ///
    /// On an attribute the element does not accept, or a value it cannot
    /// parse.
    pub fn attr(mut self, name: &str, value: impl ToString) -> Self {
        let value = value.to_string();
        match name {
            "id" => self.id = Some(value),
            "class" => self
                .classes
                .extend(value.split_whitespace().map(str::to_owned)),
            "style" => match DeclarationBlock::parse(&value) {
                Ok(block) => self.inline_style = Some(block),
                Err(error) => panic!("<{}> style=\"{value}\": {error}", self.tag),
            },
            "grow" => {
                self.grow = value
                    .trim()
                    .parse()
                    .unwrap_or_else(|_| panic!("<{}> grow=\"{value}\": not a number", self.tag))
            }
            "align" => {
                self.align = Some(Align::parse(&value).unwrap_or_else(|| {
                    panic!(
                        "<{}> align=\"{value}\": expected start, center, end or stretch",
                        self.tag
                    )
                }))
            }
            "disabled" => self.disabled = parse_bool(&value),
            "shortcut" => match Shortcut::parse(&value) {
                Ok(shortcut) => self.shortcut = Some(shortcut),
                Err(error) => panic!("<{}> shortcut=\"{value}\": {error}", self.tag),
            },
            "state" => self
                .custom_states
                .extend(value.split_whitespace().map(str::to_owned)),
            _ => {
                if !self.widget.set_attribute(name, &value) {
                    panic!("<{}> does not accept the attribute `{name}`", self.tag);
                }
                self.attrs.push((name.to_owned(), value));
            }
        }
        self
    }

    /// Attaches a handler made by the functions in [`on`](crate::on).
    pub fn on(mut self, handler: Handler<M>) -> Self {
        match handler {
            Handler::Click(f) => self.handlers.click = Some(f),
            Handler::Change(f) => self.handlers.change = Some(f),
            Handler::Toggle(f) => self.handlers.toggle = Some(f),
            Handler::Submit(f) => self.handlers.submit = Some(f),
        }
        self
    }

    /// Appends children: an element, a list of them, an option, or text.
    ///
    /// Text goes to the widget if it takes a `text` attribute (a label does)
    /// and otherwise becomes a child label, so `<button>"Save"</button>` is a
    /// button containing a label.
    pub fn child(mut self, child: impl IntoChildren<M>) -> Self {
        for child in child.into_children() {
            match child {
                Child::Element(element) => self.children.push(*element),
                Child::Text(text) => {
                    if !self.widget.set_attribute("text", &text) {
                        self.children.push(elements::label().attr("text", text));
                    }
                }
            }
        }
        self
    }

    pub fn tag(&self) -> &'static str {
        self.tag
    }

    pub fn children(&self) -> &[Element<M>] {
        &self.children
    }

    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    pub fn classes(&self) -> &[String] {
        &self.classes
    }
}

/// A child as the macro sees it: an element or a run of text.
pub enum Child<M> {
    Element(Box<Element<M>>),
    Text(String),
}

/// Anything that can appear as a child in the [`ui!`](crate::ui) macro.
pub trait IntoChildren<M> {
    fn into_children(self) -> Vec<Child<M>>;
}

impl<M> IntoChildren<M> for Element<M> {
    fn into_children(self) -> Vec<Child<M>> {
        vec![Child::Element(Box::new(self))]
    }
}

impl<M> IntoChildren<M> for Vec<Element<M>> {
    fn into_children(self) -> Vec<Child<M>> {
        self.into_iter()
            .map(|e| Child::Element(Box::new(e)))
            .collect()
    }
}

impl<M> IntoChildren<M> for Option<Element<M>> {
    fn into_children(self) -> Vec<Child<M>> {
        self.into_iter()
            .map(|e| Child::Element(Box::new(e)))
            .collect()
    }
}

impl<M> IntoChildren<M> for Child<M> {
    fn into_children(self) -> Vec<Child<M>> {
        vec![self]
    }
}

impl<M> IntoChildren<M> for &str {
    fn into_children(self) -> Vec<Child<M>> {
        vec![Child::Text(self.to_owned())]
    }
}

impl<M> IntoChildren<M> for String {
    fn into_children(self) -> Vec<Child<M>> {
        vec![Child::Text(self)]
    }
}

impl<M> IntoChildren<M> for &String {
    fn into_children(self) -> Vec<Child<M>> {
        vec![Child::Text(self.clone())]
    }
}

/// Constructors for the built-in elements, one per tag the
/// [`ui!`](crate::ui) macro understands.
pub mod elements {
    use super::Element;
    use super::*;

    /// Lays children out top to bottom.
    pub fn column<M: 'static>() -> Element<M> {
        Element::new("column", Container::column())
    }

    /// Lays children out left to right.
    pub fn row<M: 'static>() -> Element<M> {
        Element::new("row", Container::row())
    }

    /// A column meant to be framed by the theme.
    pub fn panel<M: 'static>() -> Element<M> {
        Element::new("panel", Container::column())
    }

    /// A line of text. Takes a `text` attribute or a text child.
    pub fn label<M: 'static>() -> Element<M> {
        Element::new("label", Label::default())
    }

    /// A push button. Text children become its label.
    pub fn button<M: 'static>() -> Element<M> {
        Element::new("button", Button)
    }

    /// A checkbox: a `check` element followed by any children. Takes
    /// `checked`.
    pub fn checkbox<M: 'static>() -> Element<M> {
        Element::new("checkbox", Checkbox::default()).child(Element::new("check", Blank))
    }

    /// A single-line text field. Takes `value` and `placeholder`.
    pub fn input<M: 'static>() -> Element<M> {
        Element::new("input", TextInput::default())
    }

    /// A thin line between things; the theme gives it a size and colour.
    pub fn separator<M: 'static>() -> Element<M> {
        Element::new("separator", Blank)
    }

    /// An empty element that grows to take up spare room, pushing what
    /// follows it to the far end.
    pub fn spacer<M: 'static>() -> Element<M> {
        Element::new("spacer", Blank).attr("grow", 1)
    }
}

/// Marker types the [`ui!`](crate::ui) macro uses to check that a closing
/// tag matches its opening tag. Not for use directly.
#[doc(hidden)]
#[allow(non_camel_case_types)]
pub mod tags {
    pub struct column;
    pub struct row;
    pub struct panel;
    pub struct label;
    pub struct button;
    pub struct checkbox;
    pub struct input;
    pub struct separator;
    pub struct spacer;
}
