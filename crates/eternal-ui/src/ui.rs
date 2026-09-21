// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The element tree: mounting, styling, focus and event dispatch.

use std::fmt;

use eternal_styler::{
    ComputedStyle, DeclarationBlock, ElementState, Error, OpaqueElement, Styler, Stylesheet,
    opaque_element_from_index,
};

use crate::element::Element;
use crate::event::{Event, Key, Modifiers};
use crate::geometry::{Point, Rect, Size};
use crate::layout::{Align, Justify};
use crate::render::Fonts;
use crate::shortcut::Shortcut;
use crate::style::default_theme;
use crate::widget::{EventCx, Handlers, Widget};
use crate::widgets::{Container, parse_bool};

/// A handle to a mounted element. Stays valid until the element is removed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(usize);

pub(crate) struct Node<M> {
    pub(crate) tag: &'static str,
    pub(crate) widget: Box<dyn Widget<M>>,
    pub(crate) id: Option<String>,
    pub(crate) classes: Vec<String>,
    pub(crate) attrs: Vec<(String, String)>,
    pub(crate) custom_states: Vec<String>,
    pub(crate) inline_style: Option<DeclarationBlock>,
    pub(crate) handlers: Handlers<M>,
    pub(crate) grow: f32,
    pub(crate) align: Option<Align>,
    pub(crate) disabled: bool,
    pub(crate) shortcut: Option<Shortcut>,
    /// The bits the framework manages: hover, active, focus.
    pub(crate) state: ElementState,
    pub(crate) parent: Option<NodeId>,
    pub(crate) children: Vec<NodeId>,
    pub(crate) style: ComputedStyle,
    pub(crate) measured: Size,
    pub(crate) rect: Rect,
    pub(crate) content: Rect,
}

impl<M: 'static> Node<M> {
    /// How a container aligns and justifies its children; defaults for
    /// widgets that are not [`Container`]s.
    pub(crate) fn container_alignment(&self) -> (Align, Justify) {
        match self.widget.as_any().downcast_ref::<Container>() {
            Some(container) => (container.align, container.justify),
            None => (Align::Stretch, Justify::Start),
        }
    }
}

/// The element tree, the stylesheets that style it, and the focus.
///
/// `M` is the application's message type. Handlers produce messages, and
/// [`handle`](Self::handle) returns them for the application to act on,
/// which keeps every decision in explicit application code rather than in
/// callbacks scattered through the tree.
///
/// A frame goes: feed events to [`handle`](Self::handle), act on the
/// messages, [`layout`](Self::layout), [`paint`](Self::paint).
pub struct Ui<M> {
    nodes: Vec<Option<Node<M>>>,
    free: Vec<usize>,
    root: Option<NodeId>,
    styler: Styler,
    focus: Option<NodeId>,
    hover: Option<NodeId>,
    active: Option<NodeId>,
    shortcuts: Vec<(Shortcut, Box<dyn FnMut() -> M>)>,
    pub(crate) dirty: bool,
    pub(crate) viewport: Size,
}

impl<M: 'static> Default for Ui<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: 'static> Ui<M> {
    /// An empty tree styled by the [default theme](crate::default_theme).
    pub fn new() -> Self {
        let mut ui = Self::unstyled();
        ui.styler.add_stylesheet(default_theme());
        ui
    }

    /// An empty tree with no stylesheet at all.
    pub fn unstyled() -> Self {
        Self {
            nodes: Vec::new(),
            free: Vec::new(),
            root: None,
            styler: Styler::new(),
            focus: None,
            hover: None,
            active: None,
            shortcuts: Vec::new(),
            dirty: true,
            viewport: Size::ZERO,
        }
    }

    // Stylesheets

    /// Replaces every stylesheet with this one.
    pub fn set_theme(&mut self, theme: Stylesheet) {
        self.styler.clear_stylesheets();
        self.styler.add_stylesheet(theme);
        self.dirty = true;
    }

    /// Adds a stylesheet after the existing ones, so it wins ties.
    pub fn add_stylesheet(&mut self, sheet: Stylesheet) {
        self.styler.add_stylesheet(sheet);
        self.dirty = true;
    }

    pub fn styler(&self) -> &Styler {
        &self.styler
    }

    /// The style engine, for changing the root font size and the like.
    pub fn styler_mut(&mut self) -> &mut Styler {
        self.dirty = true;
        &mut self.styler
    }

    // Mounting

    /// Replaces the whole tree with `element`.
    pub fn set_root(&mut self, element: Element<M>) -> NodeId {
        self.clear();
        let id = self.mount_element(None, element);
        self.root = Some(id);
        id
    }

    /// Appends `element` as the last child of `parent`.
    pub fn mount(&mut self, parent: NodeId, element: Element<M>) -> NodeId {
        let id = self.mount_element(Some(parent), element);
        self.node_mut(parent).children.push(id);
        id
    }

    fn mount_element(&mut self, parent: Option<NodeId>, element: Element<M>) -> NodeId {
        let Element {
            tag,
            widget,
            id: element_id,
            classes,
            attrs,
            custom_states,
            inline_style,
            handlers,
            children,
            grow,
            align,
            disabled,
            shortcut,
        } = element;
        let id = self.allocate(Node {
            tag,
            widget,
            id: element_id,
            classes,
            attrs,
            custom_states,
            inline_style,
            handlers,
            grow,
            align,
            disabled,
            shortcut,
            state: ElementState::empty(),
            parent,
            children: Vec::new(),
            style: ComputedStyle::initial(),
            measured: Size::ZERO,
            rect: Rect::ZERO,
            content: Rect::ZERO,
        });
        for child in children {
            let child_id = self.mount_element(Some(id), child);
            self.node_mut(id).children.push(child_id);
        }
        self.dirty = true;
        id
    }

    fn allocate(&mut self, node: Node<M>) -> NodeId {
        match self.free.pop() {
            Some(index) => {
                self.nodes[index] = Some(node);
                NodeId(index)
            }
            None => {
                self.nodes.push(Some(node));
                NodeId(self.nodes.len() - 1)
            }
        }
    }

    /// Removes an element and everything under it.
    pub fn remove(&mut self, id: NodeId) {
        if self.root == Some(id) {
            self.clear();
            return;
        }
        if let Some(parent) = self.node(id).parent {
            self.node_mut(parent).children.retain(|&child| child != id);
        }
        self.free_subtree(id);
        self.dirty = true;
    }

    /// Removes every element.
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.free.clear();
        self.root = None;
        self.focus = None;
        self.hover = None;
        self.active = None;
        self.dirty = true;
    }

    fn free_subtree(&mut self, id: NodeId) {
        if self.focus == Some(id) {
            self.set_focus(None, false);
        }
        if self.hover == Some(id) {
            self.hover = None;
        }
        if self.active == Some(id) {
            self.active = None;
        }
        let children = std::mem::take(&mut self.node_mut(id).children);
        for child in children {
            self.free_subtree(child);
        }
        self.nodes[id.0] = None;
        self.free.push(id.0);
    }

    // Looking around

    pub fn root(&self) -> Option<NodeId> {
        self.root
    }

    /// The element with this `id` attribute, if any.
    pub fn find(&self, id: &str) -> Option<NodeId> {
        self.preorder()
            .into_iter()
            .find(|&node| self.node(node).id.as_deref() == Some(id))
    }

    pub fn contains(&self, id: NodeId) -> bool {
        self.nodes.get(id.0).is_some_and(Option::is_some)
    }

    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.node(id).parent
    }

    pub fn children(&self, id: NodeId) -> &[NodeId] {
        &self.node(id).children
    }

    pub fn tag(&self, id: NodeId) -> &'static str {
        self.node(id).tag
    }

    /// The element's border box, as of the last [`layout`](Self::layout).
    pub fn rect(&self, id: NodeId) -> Rect {
        self.node(id).rect
    }

    /// The element's content box, as of the last [`layout`](Self::layout).
    pub fn content_rect(&self, id: NodeId) -> Rect {
        self.node(id).content
    }

    /// The element's style, as of the last [`layout`](Self::layout).
    pub fn style(&self, id: NodeId) -> &ComputedStyle {
        &self.node(id).style
    }

    /// The element's full state: what the framework tracks, what the widget
    /// owns, and `DISABLED` if it is disabled.
    pub fn state(&self, id: NodeId) -> ElementState {
        self.state_of(id)
    }

    /// The widget behind an element, if it is a `W`.
    pub fn widget<W: Widget<M>>(&self, id: NodeId) -> Option<&W> {
        self.node(id).widget.as_any().downcast_ref()
    }

    /// The widget behind an element, for changing it. Styles and layout are
    /// recomputed afterwards.
    pub fn widget_mut<W: Widget<M>>(&mut self, id: NodeId) -> Option<&mut W> {
        self.dirty = true;
        self.node_mut(id).widget.as_any_mut().downcast_mut()
    }

    /// The deepest element under `point`, as of the last layout.
    pub fn hit_test(&self, point: Point) -> Option<NodeId> {
        self.root.and_then(|root| self.hit(root, point))
    }

    fn hit(&self, id: NodeId, point: Point) -> Option<NodeId> {
        let node = self.node(id);
        if !node.rect.contains(point) {
            return None;
        }
        node.children
            .iter()
            .rev()
            .find_map(|&child| self.hit(child, point))
            .or(Some(id))
    }

    /// Every element, parents before children, siblings in order.
    pub fn preorder(&self) -> Vec<NodeId> {
        let mut order = Vec::new();
        let mut stack: Vec<NodeId> = self.root.into_iter().collect();
        while let Some(id) = stack.pop() {
            order.push(id);
            stack.extend(self.node(id).children.iter().rev());
        }
        order
    }

    /// Whether something changed since the last [`layout`](Self::layout).
    pub fn needs_layout(&self) -> bool {
        self.dirty
    }

    pub fn viewport(&self) -> Size {
        self.viewport
    }

    // Changing elements

    pub fn has_class(&self, id: NodeId, class: &str) -> bool {
        self.node(id).classes.iter().any(|c| c == class)
    }

    /// Adds or removes a class.
    pub fn set_class(&mut self, id: NodeId, class: &str, on: bool) {
        let node = self.node_mut(id);
        let present = node.classes.iter().position(|c| c == class);
        match (present, on) {
            (None, true) => node.classes.push(class.to_owned()),
            (Some(index), false) => {
                node.classes.remove(index);
            }
            _ => return,
        }
        self.dirty = true;
    }

    pub fn is_disabled(&self, id: NodeId) -> bool {
        self.node(id).disabled
    }

    /// A disabled element matches `:disabled`, gets no events and cannot
    /// take focus.
    pub fn set_disabled(&mut self, id: NodeId, disabled: bool) {
        if self.focus == Some(id) && disabled {
            self.set_focus(None, false);
        }
        self.node_mut(id).disabled = disabled;
        self.dirty = true;
    }

    /// Sets or clears a custom state, matched by `:state(name)`.
    pub fn set_state(&mut self, id: NodeId, name: &str, on: bool) {
        let node = self.node_mut(id);
        let present = node.custom_states.iter().position(|s| s == name);
        match (present, on) {
            (None, true) => node.custom_states.push(name.to_owned()),
            (Some(index), false) => {
                node.custom_states.remove(index);
            }
            _ => return,
        }
        self.dirty = true;
    }

    /// Sets an attribute at run time, the same ones [`Element::attr`] takes.
    /// Returns `false` for an attribute the element does not accept or a
    /// value it cannot parse, rather than panicking, since the value may
    /// come from anywhere.
    pub fn set_attr(&mut self, id: NodeId, name: &str, value: &str) -> bool {
        let node = self.node_mut(id);
        let accepted = match name {
            "id" => {
                node.id = Some(value.to_owned());
                true
            }
            "class" => {
                node.classes = value.split_whitespace().map(str::to_owned).collect();
                true
            }
            "style" => match DeclarationBlock::parse(value) {
                Ok(block) => {
                    node.inline_style = Some(block);
                    true
                }
                Err(_) => false,
            },
            "grow" => match value.trim().parse() {
                Ok(grow) => {
                    node.grow = grow;
                    true
                }
                Err(_) => false,
            },
            "align" => match Align::parse(value) {
                Some(align) => {
                    node.align = Some(align);
                    true
                }
                None => false,
            },
            "disabled" => {
                let disabled = parse_bool(value);
                self.set_disabled(id, disabled);
                return true;
            }
            "shortcut" => match Shortcut::parse(value) {
                Ok(shortcut) => {
                    node.shortcut = Some(shortcut);
                    true
                }
                Err(_) => false,
            },
            "state" => {
                node.custom_states = value.split_whitespace().map(str::to_owned).collect();
                true
            }
            _ => {
                let accepted = node.widget.set_attribute(name, value);
                if accepted {
                    match node.attrs.iter_mut().find(|(n, _)| n == name) {
                        Some(entry) => entry.1 = value.to_owned(),
                        None => node.attrs.push((name.to_owned(), value.to_owned())),
                    }
                }
                accepted
            }
        };
        if accepted {
            self.dirty = true;
        }
        accepted
    }

    /// Replaces the inline style.
    pub fn set_inline_style(&mut self, id: NodeId, css: &str) -> Result<(), Error> {
        let block = DeclarationBlock::parse(css)?;
        self.node_mut(id).inline_style = Some(block);
        self.dirty = true;
        Ok(())
    }

    // Focus

    pub fn focused(&self) -> Option<NodeId> {
        self.focus
    }

    pub fn hovered(&self) -> Option<NodeId> {
        self.hover
    }

    /// Moves keyboard focus to `id`, if it can take it. Focus moved this
    /// way shows as `:focus-visible`, like focus moved with the keyboard.
    pub fn focus(&mut self, id: NodeId) -> bool {
        if !self.is_focusable(id) {
            return false;
        }
        self.set_focus(Some(id), true);
        true
    }

    pub fn blur(&mut self) {
        self.set_focus(None, false);
    }

    fn is_focusable(&self, id: NodeId) -> bool {
        let node = self.node(id);
        node.widget.focusable() && !node.disabled
    }

    fn set_focus(&mut self, target: Option<NodeId>, visible: bool) {
        if let Some(old) = self.focus.take() {
            self.node_mut(old)
                .state
                .remove(ElementState::FOCUS | ElementState::FOCUS_VISIBLE);
            let mut ancestor = self.node(old).parent;
            while let Some(id) = ancestor {
                self.node_mut(id).state.remove(ElementState::FOCUS_WITHIN);
                ancestor = self.node(id).parent;
            }
        }
        if let Some(new) = target {
            let mut state = ElementState::FOCUS;
            if visible {
                state |= ElementState::FOCUS_VISIBLE;
            }
            self.node_mut(new).state.insert(state);
            let mut ancestor = self.node(new).parent;
            while let Some(id) = ancestor {
                self.node_mut(id).state.insert(ElementState::FOCUS_WITHIN);
                ancestor = self.node(id).parent;
            }
        }
        self.focus = target;
        self.dirty = true;
    }

    /// Moves focus to the next (or previous) focusable element in tree
    /// order, wrapping around. What Tab and Shift+Tab do.
    pub fn move_focus(&mut self, forward: bool) {
        let focusable: Vec<NodeId> = self
            .preorder()
            .into_iter()
            .filter(|&id| self.is_focusable(id))
            .collect();
        if focusable.is_empty() {
            return;
        }
        let current = self
            .focus
            .and_then(|focus| focusable.iter().position(|&id| id == focus));
        let next = match (current, forward) {
            (Some(index), true) => (index + 1) % focusable.len(),
            (Some(index), false) => (index + focusable.len() - 1) % focusable.len(),
            (None, true) => 0,
            (None, false) => focusable.len() - 1,
        };
        self.set_focus(Some(focusable[next]), true);
    }

    /// The nearest focusable element at or above `id`.
    fn focusable_from(&self, id: NodeId) -> Option<NodeId> {
        let mut current = Some(id);
        while let Some(id) = current {
            if self.is_focusable(id) {
                return Some(id);
            }
            current = self.node(id).parent;
        }
        None
    }

    // Shortcuts

    /// Binds a shortcut for the whole window. Global shortcuts are checked
    /// before element shortcuts and before the focused element sees the
    /// key.
    pub fn bind(&mut self, shortcut: Shortcut, handler: impl FnMut() -> M + 'static) {
        self.shortcuts.push((shortcut, Box::new(handler)));
    }

    pub fn unbind(&mut self, shortcut: Shortcut) {
        self.shortcuts.retain(|(bound, _)| *bound != shortcut);
    }

    /// Every shortcut in effect: the global ones and those of elements
    /// that are not disabled, with the element's id when it has one. For
    /// help screens and menus.
    pub fn shortcuts(&self) -> Vec<(Shortcut, Option<NodeId>)> {
        let mut all: Vec<(Shortcut, Option<NodeId>)> = self
            .shortcuts
            .iter()
            .map(|(shortcut, _)| (*shortcut, None))
            .collect();
        for id in self.preorder() {
            let node = self.node(id);
            if let Some(shortcut) = node.shortcut
                && !node.disabled
            {
                all.push((shortcut, Some(id)));
            }
        }
        all
    }

    fn run_shortcuts(&mut self, key: Key, modifiers: Modifiers, messages: &mut Vec<M>) -> bool {
        for (shortcut, handler) in &mut self.shortcuts {
            if shortcut.matches(key, modifiers) {
                messages.push(handler());
                return true;
            }
        }
        let target = self.preorder().into_iter().find(|&id| {
            let node = self.node(id);
            !node.disabled && node.shortcut.is_some_and(|s| s.matches(key, modifiers))
        });
        if let Some(id) = target {
            if let Some(handler) = &mut self.node_mut(id).handlers.click {
                messages.push(handler());
            }
            return true;
        }
        false
    }

    // Events

    /// Feeds one event through the tree and returns the messages it
    /// produced.
    ///
    /// Pointer events go to the deepest element under the pointer and
    /// bubble up until a widget handles them; while a button is held, they
    /// go to the element it was pressed on. Key events go first to global
    /// shortcuts, then to element shortcuts, then to the focused element,
    /// and finally Tab and Shift+Tab move focus. Text goes to the focused
    /// element.
    ///
    /// Hit-testing uses the last layout, so call [`layout`](Self::layout)
    /// after changes before feeding more pointer events.
    pub fn handle(&mut self, event: &Event, fonts: &dyn Fonts) -> Vec<M> {
        let mut messages = Vec::new();
        match event {
            Event::PointerMove { position } => {
                self.update_hover(Some(*position));
                if let Some(target) = self.active.or_else(|| self.hit_test(*position)) {
                    self.deliver(target, event, fonts, &mut messages);
                }
            }
            Event::PointerDown { position, .. } => {
                self.update_hover(Some(*position));
                let target = self.hit_test(*position);
                self.set_active(target);
                match target {
                    Some(target) => {
                        let focus = self.focusable_from(target);
                        self.set_focus(focus, false);
                        self.deliver(target, event, fonts, &mut messages);
                    }
                    None => self.set_focus(None, false),
                }
            }
            Event::PointerUp { position, .. } => {
                if let Some(target) = self.active.or_else(|| self.hit_test(*position)) {
                    self.deliver(target, event, fonts, &mut messages);
                }
                self.set_active(None);
                self.update_hover(Some(*position));
            }
            Event::PointerLeave => self.update_hover(None),
            Event::Wheel { position, .. } => {
                if let Some(target) = self.hit_test(*position) {
                    self.deliver(target, event, fonts, &mut messages);
                }
            }
            Event::KeyDown { key, modifiers } => {
                if self.run_shortcuts(*key, *modifiers, &mut messages) {
                    return messages;
                }
                if let Some(focus) = self.focus
                    && self.deliver(focus, event, fonts, &mut messages)
                {
                    return messages;
                }
                if *key == Key::Tab && (modifiers.is_empty() || *modifiers == Modifiers::SHIFT) {
                    self.move_focus(!modifiers.contains(Modifiers::SHIFT));
                }
            }
            Event::KeyUp { .. } | Event::Text(_) => {
                if let Some(focus) = self.focus {
                    self.deliver(focus, event, fonts, &mut messages);
                }
            }
        }
        messages
    }

    /// Offers `event` to `start` and then to each ancestor until one
    /// handles it. Disabled elements are skipped.
    fn deliver(
        &mut self,
        start: NodeId,
        event: &Event,
        fonts: &dyn Fonts,
        messages: &mut Vec<M>,
    ) -> bool {
        let mut current = Some(start);
        while let Some(id) = current {
            let state = self.state_of(id);
            let node = self.node_mut(id);
            if !node.disabled {
                let Node {
                    widget,
                    handlers,
                    style,
                    rect,
                    content,
                    ..
                } = node;
                let mut cx = EventCx {
                    handlers,
                    messages,
                    fonts,
                    focus_requested: false,
                    changed: false,
                    rect: *rect,
                    content: *content,
                    state,
                    style,
                };
                let handled = widget.event(&mut cx, event);
                let (focus_requested, changed) = (cx.focus_requested, cx.changed);
                if changed {
                    self.dirty = true;
                }
                if focus_requested && self.is_focusable(id) {
                    self.set_focus(Some(id), false);
                }
                if handled {
                    return true;
                }
            }
            current = self.node(id).parent;
        }
        false
    }

    /// Marks the element under `position` and its ancestors as hovered,
    /// and nothing else.
    fn update_hover(&mut self, position: Option<Point>) {
        let target = position.and_then(|p| self.hit_test(p));
        if target == self.hover && !self.dirty {
            return;
        }
        self.hover = target;
        self.set_chain_state(target, ElementState::HOVER);
    }

    fn set_active(&mut self, target: Option<NodeId>) {
        self.active = target;
        self.set_chain_state(target, ElementState::ACTIVE);
    }

    /// Sets `flag` on `target` and its ancestors and clears it everywhere
    /// else.
    fn set_chain_state(&mut self, target: Option<NodeId>, flag: ElementState) {
        let mut chain = Vec::new();
        let mut current = target;
        while let Some(id) = current {
            chain.push(id);
            current = self.node(id).parent;
        }
        for id in self.preorder() {
            let node = self.node_mut(id);
            let before = node.state;
            node.state.set(flag, chain.contains(&id));
            if node.state != before {
                self.dirty = true;
            }
        }
    }

    // Styles

    pub(crate) fn recompute_styles(&mut self) {
        for id in self.preorder() {
            let parent_style = self.node(id).parent.map(|p| self.node(p).style.clone());
            let node_ref = NodeRef { ui: self, id };
            let node = self.node(id);
            let style = self.styler.compute_with_inline(
                &node_ref,
                parent_style.as_ref(),
                node.inline_style.as_ref(),
            );
            self.node_mut(id).style = style;
        }
    }

    pub(crate) fn state_of(&self, id: NodeId) -> ElementState {
        let node = self.node(id);
        let mut state = node.state | node.widget.state();
        if node.disabled {
            state |= ElementState::DISABLED;
        }
        state
    }

    pub(crate) fn node(&self, id: NodeId) -> &Node<M> {
        self.nodes
            .get(id.0)
            .and_then(Option::as_ref)
            .unwrap_or_else(|| panic!("{id:?} was removed"))
    }

    pub(crate) fn node_mut(&mut self, id: NodeId) -> &mut Node<M> {
        self.nodes
            .get_mut(id.0)
            .and_then(Option::as_mut)
            .unwrap_or_else(|| panic!("{id:?} was removed"))
    }
}

impl<M: 'static> fmt::Debug for Ui<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn write_node<M: 'static>(
            ui: &Ui<M>,
            id: NodeId,
            depth: usize,
            f: &mut fmt::Formatter<'_>,
        ) -> fmt::Result {
            let node = ui.node(id);
            write!(f, "{:indent$}<{}", "", node.tag, indent = depth * 2)?;
            if let Some(id) = &node.id {
                write!(f, " id=\"{id}\"")?;
            }
            if !node.classes.is_empty() {
                write!(f, " class=\"{}\"", node.classes.join(" "))?;
            }
            writeln!(f, "> {:?}", node.rect)?;
            for &child in &node.children {
                write_node(ui, child, depth + 1, f)?;
            }
            Ok(())
        }
        match self.root {
            Some(root) => write_node(self, root, 0, f),
            None => writeln!(f, "<empty>"),
        }
    }
}

/// What the style engine sees of an element.
struct NodeRef<'a, M> {
    ui: &'a Ui<M>,
    id: NodeId,
}

impl<M> Clone for NodeRef<'_, M> {
    fn clone(&self) -> Self {
        Self {
            ui: self.ui,
            id: self.id,
        }
    }
}

impl<M> fmt::Debug for NodeRef<'_, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeRef({:?})", self.id)
    }
}

impl<M: 'static> NodeRef<'_, M> {
    fn node(&self) -> &Node<M> {
        self.ui.node(self.id)
    }

    fn at(&self, id: NodeId) -> Self {
        Self { ui: self.ui, id }
    }

    fn sibling(&self, offset: isize) -> Option<Self> {
        let parent = self.node().parent?;
        let siblings = &self.ui.node(parent).children;
        let position = siblings.iter().position(|&id| id == self.id)?;
        let target = position.checked_add_signed(offset)?;
        siblings.get(target).map(|&id| self.at(id))
    }
}

impl<M: 'static> eternal_styler::Element for NodeRef<'_, M> {
    fn opaque(&self) -> OpaqueElement {
        opaque_element_from_index(self.id.0)
    }

    fn parent(&self) -> Option<Self> {
        self.node().parent.map(|id| self.at(id))
    }

    fn prev_sibling(&self) -> Option<Self> {
        self.sibling(-1)
    }

    fn next_sibling(&self) -> Option<Self> {
        self.sibling(1)
    }

    fn first_child(&self) -> Option<Self> {
        self.node().children.first().map(|&id| self.at(id))
    }

    fn local_name(&self) -> &str {
        self.node().tag
    }

    fn id(&self) -> Option<&str> {
        self.node().id.as_deref()
    }

    fn classes(&self) -> impl Iterator<Item = &str> {
        self.node().classes.iter().map(String::as_str)
    }

    fn attribute(&self, name: &str) -> Option<&str> {
        let node = self.node();
        if name == "disabled" {
            return node.disabled.then_some("");
        }
        node.attrs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    fn state(&self) -> ElementState {
        self.ui.state_of(self.id)
    }

    fn has_custom_state(&self, name: &str) -> bool {
        self.node().custom_states.iter().any(|s| s == name)
    }
}
