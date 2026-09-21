// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The behaviour side of an element.

use std::any::Any;

use eternal_styler::{ComputedStyle, ElementState};

use crate::event::Event;
use crate::geometry::{Axis, Rect, Size};
use crate::render::{Fonts, Renderer};

/// What an element does: how it measures, reacts to input and paints.
///
/// Composition is declarative (the [`ui!`](crate::ui) macro builds a tree of
/// elements) but behaviour is imperative: a widget is a plain struct with
/// methods, and you can reach it through
/// [`Ui::widget_mut`](crate::Ui::widget_mut) to change it.
///
/// The framework draws backgrounds and borders from the computed style and
/// lays children out; a widget only paints its own content and handles
/// events aimed at it. Every method has a default, so a container is an
/// empty impl plus [`axis`](Self::axis).
pub trait Widget<M>: Any {
    /// Takes an attribute from the element description, returning `false`
    /// for one the widget does not know. `id`, `class`, `style`, `grow`,
    /// `align`, `disabled`, `shortcut` and `state` never reach here.
    fn set_attribute(&mut self, name: &str, value: &str) -> bool {
        let _ = (name, value);
        false
    }

    /// `Some` for a container: the direction it lays its children out in.
    fn axis(&self) -> Option<Axis> {
        None
    }

    /// The natural size of the widget's own content, without padding or
    /// border. Containers are measured from their children instead.
    fn measure(&self, fonts: &dyn Fonts, style: &ComputedStyle) -> Size {
        let _ = (fonts, style);
        Size::ZERO
    }

    /// State the widget owns, such as `CHECKED`, merged into the element's
    /// state for styling.
    fn state(&self) -> ElementState {
        ElementState::empty()
    }

    /// Whether the widget can take keyboard focus.
    fn focusable(&self) -> bool {
        false
    }

    /// Handles an event aimed at this element. Returning `true` stops the
    /// event from bubbling to the parent.
    fn event(&mut self, cx: &mut EventCx<'_, M>, event: &Event) -> bool {
        let _ = (cx, event);
        false
    }

    /// Paints the widget's own content inside `content`, after the
    /// framework has painted the background and border and before it paints
    /// the children.
    fn paint(&self, renderer: &mut dyn Renderer, cx: &PaintCx<'_>, content: Rect) {
        let _ = (renderer, cx, content);
    }

    fn as_any(&self) -> &dyn Any;

    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// The handlers an element was declared with.
///
/// Each kind of event has one slot. Widgets fire them through [`EventCx`].
pub struct Handlers<M> {
    pub(crate) click: Option<Box<dyn FnMut() -> M>>,
    pub(crate) change: Option<Box<dyn FnMut(String) -> M>>,
    pub(crate) toggle: Option<Box<dyn FnMut(bool) -> M>>,
    pub(crate) submit: Option<Box<dyn FnMut(String) -> M>>,
}

impl<M> Default for Handlers<M> {
    fn default() -> Self {
        Self {
            click: None,
            change: None,
            toggle: None,
            submit: None,
        }
    }
}

/// One handler, made by the functions in [`on`](crate::on) and attached with
/// [`Element::on`](crate::Element::on).
pub enum Handler<M> {
    Click(Box<dyn FnMut() -> M>),
    Change(Box<dyn FnMut(String) -> M>),
    Toggle(Box<dyn FnMut(bool) -> M>),
    Submit(Box<dyn FnMut(String) -> M>),
}

/// Constructors for handlers. In the [`ui!`](crate::ui) macro,
/// `on:click={|| Msg::Save}` becomes `.on(on::click(|| Msg::Save))`.
pub mod on {
    use super::Handler;

    /// A button was clicked, or activated from the keyboard, or its
    /// shortcut was pressed.
    pub fn click<M>(f: impl FnMut() -> M + 'static) -> Handler<M> {
        Handler::Click(Box::new(f))
    }

    /// A text field's text changed. Fired on every edit.
    pub fn change<M>(f: impl FnMut(String) -> M + 'static) -> Handler<M> {
        Handler::Change(Box::new(f))
    }

    /// A checkbox was checked or unchecked.
    pub fn toggle<M>(f: impl FnMut(bool) -> M + 'static) -> Handler<M> {
        Handler::Toggle(Box::new(f))
    }

    /// Enter was pressed in a text field.
    pub fn submit<M>(f: impl FnMut(String) -> M + 'static) -> Handler<M> {
        Handler::Submit(Box::new(f))
    }
}

/// What a widget can do while handling an event.
pub struct EventCx<'a, M> {
    pub(crate) handlers: &'a mut Handlers<M>,
    pub(crate) messages: &'a mut Vec<M>,
    pub(crate) fonts: &'a dyn Fonts,
    pub(crate) focus_requested: bool,
    pub(crate) changed: bool,
    /// The element's border box.
    pub rect: Rect,
    /// The element's content box.
    pub content: Rect,
    /// The element's state at the time of the event.
    pub state: ElementState,
    /// The element's computed style.
    pub style: &'a ComputedStyle,
}

impl<M> EventCx<'_, M> {
    /// Sends a message to the application.
    pub fn emit(&mut self, message: M) {
        self.messages.push(message);
    }

    /// Fires the element's `click` handler, if it has one.
    pub fn click(&mut self) {
        if let Some(handler) = &mut self.handlers.click {
            self.messages.push(handler());
        }
    }

    pub fn change(&mut self, text: String) {
        if let Some(handler) = &mut self.handlers.change {
            self.messages.push(handler(text));
        }
    }

    pub fn toggle(&mut self, checked: bool) {
        if let Some(handler) = &mut self.handlers.toggle {
            self.messages.push(handler(checked));
        }
    }

    pub fn submit(&mut self, text: String) {
        if let Some(handler) = &mut self.handlers.submit {
            self.messages.push(handler(text));
        }
    }

    /// Asks for keyboard focus to move to this element.
    pub fn request_focus(&mut self) {
        self.focus_requested = true;
    }

    /// Tells the framework the widget's appearance or size changed, so
    /// styles and layout are recomputed before the next paint.
    pub fn mark_changed(&mut self) {
        self.changed = true;
    }

    /// For measuring text, for example to place a caret under the pointer.
    pub fn fonts(&self) -> &dyn Fonts {
        self.fonts
    }
}

/// What a widget can see while painting.
pub struct PaintCx<'a> {
    pub style: &'a ComputedStyle,
    pub state: ElementState,
}
