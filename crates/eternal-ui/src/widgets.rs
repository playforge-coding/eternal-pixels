// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The built-in widgets.
//!
//! Each is a plain struct you can reach through
//! [`Ui::widget`](crate::Ui::widget) and [`Ui::widget_mut`](crate::Ui::widget_mut)
//! to read or change it imperatively. How they look comes entirely from the
//! stylesheet; the widgets only know how to measure, react and paint their
//! own content.

use eternal_styler::{ComputedStyle, ElementState, TextAlign};

use crate::event::{Event, Key, Modifiers};
use crate::geometry::{Axis, Point, Rect, Size};
use crate::layout::{Align, Justify};
use crate::render::{FontSpec, Fonts, Renderer};
use crate::widget::{EventCx, PaintCx, Widget};

/// Implements the two `Any` accessors of [`Widget`] for a type.
#[macro_export]
macro_rules! impl_widget_any {
    () => {
        fn as_any(&self) -> &dyn ::std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn ::std::any::Any {
            self
        }
    };
}

/// Lays its children out in a row or a column. Also what `panel`,
/// `separator`, `spacer` and `check` are made of; they differ only by tag,
/// which is what the stylesheet sees.
#[derive(Clone, Debug)]
pub struct Container {
    axis: Axis,
    /// How children are placed across the axis unless they say otherwise.
    /// The `align_items` attribute.
    pub align: Align,
    /// How children are spread along the axis when there is room left. The
    /// `justify` attribute.
    pub justify: Justify,
}

impl Container {
    pub fn new(axis: Axis) -> Self {
        Self {
            axis,
            align: Align::Stretch,
            justify: Justify::Start,
        }
    }

    pub fn column() -> Self {
        Self::new(Axis::Vertical)
    }

    pub fn row() -> Self {
        Self::new(Axis::Horizontal)
    }
}

impl<M: 'static> Widget<M> for Container {
    fn set_attribute(&mut self, name: &str, value: &str) -> bool {
        match name {
            "align_items" => match Align::parse(value) {
                Some(align) => {
                    self.align = align;
                    true
                }
                None => false,
            },
            "justify" => match Justify::parse(value) {
                Some(justify) => {
                    self.justify = justify;
                    true
                }
                None => false,
            },
            _ => false,
        }
    }

    fn axis(&self) -> Option<Axis> {
        Some(self.axis)
    }

    impl_widget_any!();
}

/// A single line of text.
#[derive(Clone, Debug, Default)]
pub struct Label {
    text: String,
}

impl Label {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }
}

impl<M: 'static> Widget<M> for Label {
    fn set_attribute(&mut self, name: &str, value: &str) -> bool {
        if name == "text" {
            self.text = value.to_owned();
            return true;
        }
        false
    }

    fn measure(&self, fonts: &dyn Fonts, style: &ComputedStyle) -> Size {
        let metrics = fonts.measure(&self.text, &FontSpec::from_style(style));
        Size::new(
            metrics.width + style.letter_spacing * self.text.chars().count() as f32,
            metrics.line_height,
        )
    }

    fn paint(&self, renderer: &mut dyn Renderer, cx: &PaintCx<'_>, content: Rect) {
        paint_line(renderer, cx.style, &self.text, content, cx.style.text_align);
    }

    impl_widget_any!();
}

/// Draws one line of text inside `content`, centred vertically and aligned
/// horizontally per `align`. Needs the font metrics, so measures again.
fn paint_line(
    renderer: &mut dyn Renderer,
    style: &ComputedStyle,
    text: &str,
    content: Rect,
    align: TextAlign,
) {
    let font = FontSpec::from_style(style);
    let line_height = font.line_height.unwrap_or(style.font_size * 1.25);
    let x = match align {
        TextAlign::Start | TextAlign::Left | TextAlign::Justify => content.x,
        TextAlign::Center => content.x + (content.width - text_width(text, style)) / 2.0,
        TextAlign::End | TextAlign::Right => content.right() - text_width(text, style),
    };
    let y = content.y + ((content.height - line_height) / 2.0).max(0.0);
    renderer.text(text, &font, Point::new(x.round(), y.round()), style.color);
}

/// The width a label would have measured, without access to the font
/// source: a label's content box is exactly its text unless it was
/// stretched, so this is only used for alignment inside a stretched box.
fn text_width(text: &str, style: &ComputedStyle) -> f32 {
    // Without fonts at paint time, assume the content box was sized to the
    // text unless the label was stretched; either way the best estimate is
    // the monospace one. Backends that need exact alignment can measure in
    // `Renderer::text` from the same font spec.
    text.chars().count() as f32 * style.font_size * 0.6
}

/// A push button. Its children (usually a text label) are laid out in a
/// row, so an icon element can sit next to the text.
///
/// Fires `click` on a pointer release over the button, on Enter or Space
/// while focused, and when its `shortcut` is pressed.
#[derive(Clone, Debug, Default)]
pub struct Button;

impl<M: 'static> Widget<M> for Button {
    fn axis(&self) -> Option<Axis> {
        Some(Axis::Horizontal)
    }

    fn focusable(&self) -> bool {
        true
    }

    fn event(&mut self, cx: &mut EventCx<'_, M>, event: &Event) -> bool {
        match event {
            Event::PointerUp { position, .. }
                if cx.state.contains(ElementState::ACTIVE) && cx.rect.contains(*position) =>
            {
                cx.click();
                true
            }
            Event::KeyDown {
                key: Key::Enter | Key::Space,
                modifiers,
            } if modifiers.is_empty() => {
                cx.click();
                true
            }
            _ => false,
        }
    }

    impl_widget_any!();
}

/// A box that can be checked, followed by its children in a row.
///
/// The box itself is a child element with the tag `check`, so it is styled
/// like anything else: `checkbox:checked > check { background-color: ... }`.
/// Fires `toggle` with the new state on click or Space.
#[derive(Clone, Debug, Default)]
pub struct Checkbox {
    checked: bool,
}

impl Checkbox {
    pub fn new(checked: bool) -> Self {
        Self { checked }
    }

    pub fn is_checked(&self) -> bool {
        self.checked
    }

    pub fn set_checked(&mut self, checked: bool) {
        self.checked = checked;
    }
}

impl<M: 'static> Widget<M> for Checkbox {
    fn set_attribute(&mut self, name: &str, value: &str) -> bool {
        if name == "checked" {
            self.checked = parse_bool(value);
            return true;
        }
        false
    }

    fn axis(&self) -> Option<Axis> {
        Some(Axis::Horizontal)
    }

    fn state(&self) -> ElementState {
        if self.checked {
            ElementState::CHECKED
        } else {
            ElementState::empty()
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    fn event(&mut self, cx: &mut EventCx<'_, M>, event: &Event) -> bool {
        let toggle = match event {
            Event::PointerUp { position, .. } => {
                cx.state.contains(ElementState::ACTIVE) && cx.rect.contains(*position)
            }
            Event::KeyDown {
                key: Key::Space,
                modifiers,
            } => modifiers.is_empty(),
            _ => false,
        };
        if toggle {
            self.checked = !self.checked;
            cx.toggle(self.checked);
            cx.mark_changed();
        }
        toggle
    }

    impl_widget_any!();
}

/// `true`, `false`, or the attribute's bare presence (`checked=""`).
pub(crate) fn parse_bool(value: &str) -> bool {
    !matches!(value.trim(), "false" | "0" | "no" | "off")
}

/// A single-line text field.
///
/// Fires `change` on every edit and `submit` on Enter. Left, Right, Home,
/// End, Backspace and Delete move and edit; a click places the caret.
#[derive(Clone, Debug, Default)]
pub struct TextInput {
    text: String,
    /// In characters from the start.
    cursor: usize,
    placeholder: String,
}

impl TextInput {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            cursor: text.chars().count(),
            text,
            placeholder: String::new(),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replaces the text and puts the caret at the end.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.cursor = self.text.chars().count();
    }

    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    pub fn set_placeholder(&mut self, text: impl Into<String>) {
        self.placeholder = text.into();
    }

    /// The caret position in characters from the start.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    fn byte_offset(&self, chars: usize) -> usize {
        self.text
            .char_indices()
            .nth(chars)
            .map_or(self.text.len(), |(offset, _)| offset)
    }

    fn insert(&mut self, inserted: &str) {
        let offset = self.byte_offset(self.cursor);
        self.text.insert_str(offset, inserted);
        self.cursor += inserted.chars().count();
    }

    fn delete_before(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        let start = self.byte_offset(self.cursor - 1);
        let end = self.byte_offset(self.cursor);
        self.text.replace_range(start..end, "");
        self.cursor -= 1;
        true
    }

    fn delete_after(&mut self) -> bool {
        if self.cursor >= self.text.chars().count() {
            return false;
        }
        let start = self.byte_offset(self.cursor);
        let end = self.byte_offset(self.cursor + 1);
        self.text.replace_range(start..end, "");
        true
    }

    /// The caret position closest to `x` pixels into the text.
    fn cursor_at(&self, x: f32, fonts: &dyn Fonts, font: &FontSpec) -> usize {
        let mut best = 0;
        let mut best_distance = f32::INFINITY;
        for (index, offset) in self
            .text
            .char_indices()
            .map(|(offset, _)| offset)
            .chain(std::iter::once(self.text.len()))
            .enumerate()
        {
            let width = fonts.measure(&self.text[..offset], font).width;
            let distance = (width - x).abs();
            if distance < best_distance {
                best = index;
                best_distance = distance;
            }
        }
        best
    }
}

impl<M: 'static> Widget<M> for TextInput {
    fn set_attribute(&mut self, name: &str, value: &str) -> bool {
        match name {
            "value" => {
                self.set_text(value);
                true
            }
            "placeholder" => {
                self.placeholder = value.to_owned();
                true
            }
            _ => false,
        }
    }

    fn measure(&self, fonts: &dyn Fonts, style: &ComputedStyle) -> Size {
        let font = FontSpec::from_style(style);
        let shown = if self.text.is_empty() {
            &self.placeholder
        } else {
            &self.text
        };
        let metrics = fonts.measure(shown, &font);
        Size::new(metrics.width, metrics.line_height)
    }

    fn state(&self) -> ElementState {
        if self.text.is_empty() {
            ElementState::PLACEHOLDER_SHOWN
        } else {
            ElementState::empty()
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    fn event(&mut self, cx: &mut EventCx<'_, M>, event: &Event) -> bool {
        match event {
            Event::Text(text) => {
                let clean: String = text.chars().filter(|c| !c.is_control()).collect();
                if clean.is_empty() {
                    return false;
                }
                self.insert(&clean);
                cx.change(self.text.clone());
                cx.mark_changed();
                true
            }
            Event::KeyDown { key, modifiers }
                if !modifiers.intersects(Modifiers::CTRL | Modifiers::ALT | Modifiers::META) =>
            {
                let edited = match key {
                    Key::Backspace => self.delete_before(),
                    Key::Delete => self.delete_after(),
                    Key::ArrowLeft => {
                        self.cursor = self.cursor.saturating_sub(1);
                        cx.mark_changed();
                        return true;
                    }
                    Key::ArrowRight => {
                        self.cursor = (self.cursor + 1).min(self.text.chars().count());
                        cx.mark_changed();
                        return true;
                    }
                    Key::Home => {
                        self.cursor = 0;
                        cx.mark_changed();
                        return true;
                    }
                    Key::End => {
                        self.cursor = self.text.chars().count();
                        cx.mark_changed();
                        return true;
                    }
                    Key::Enter => {
                        cx.submit(self.text.clone());
                        return true;
                    }
                    _ => return false,
                };
                if edited {
                    cx.change(self.text.clone());
                    cx.mark_changed();
                }
                true
            }
            Event::PointerDown { position, .. } => {
                let font = FontSpec::from_style(cx.style);
                self.cursor = self.cursor_at(position.x - cx.content.x, cx.fonts(), &font);
                cx.mark_changed();
                true
            }
            _ => false,
        }
    }

    fn paint(&self, renderer: &mut dyn Renderer, cx: &PaintCx<'_>, content: Rect) {
        let font = FontSpec::from_style(cx.style);
        let line_height = font.line_height.unwrap_or(cx.style.font_size * 1.25);
        renderer.push_clip(content);
        let shown = if self.text.is_empty() {
            &self.placeholder
        } else {
            &self.text
        };
        let y = content.y + ((content.height - line_height) / 2.0).max(0.0);
        renderer.text(
            shown,
            &font,
            Point::new(content.x, y.round()),
            cx.style.color,
        );
        if cx.state.contains(ElementState::FOCUS) {
            // The caret sits after the character it follows. Its x position
            // is measured by the renderer's own fonts in a real backend; the
            // framework does not have them here, so it is estimated the same
            // way `text_width` does and corrected by the backend if it can.
            let prefix: String = self.text.chars().take(self.cursor).collect();
            let x = content.x + text_width(&prefix, cx.style);
            renderer.fill_rect(
                Rect::new(x.round(), y.round(), 1.0, line_height),
                Default::default(),
                cx.style.color,
            );
        }
        renderer.pop_clip();
    }

    impl_widget_any!();
}

/// A widget with nothing of its own: an empty box for the stylesheet to
/// paint. Used for `check`, `separator` and `spacer`.
#[derive(Clone, Debug, Default)]
pub struct Blank;

impl<M: 'static> Widget<M> for Blank {
    impl_widget_any!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_input_editing() {
        let mut input = TextInput::new("héllo");
        assert_eq!(input.cursor(), 5);
        assert!(input.delete_before());
        assert_eq!(input.text(), "héll");
        input.cursor = 1;
        input.insert("XY");
        assert_eq!(input.text(), "hXYéll");
        assert_eq!(input.cursor(), 3);
        assert!(input.delete_after());
        assert_eq!(input.text(), "hXYll");
        input.cursor = 0;
        assert!(!input.delete_before());
        input.cursor = 5;
        assert!(!input.delete_after());
    }

    #[test]
    fn caret_placement_from_x() {
        let input = TextInput::new("abcd");
        let fonts = crate::render::MonospaceFonts { advance: 0.5 };
        let font = FontSpec {
            family: Default::default(),
            size: 10.0,
            weight: Default::default(),
            style: Default::default(),
            line_height: None,
        };
        // Each glyph is 5px wide.
        assert_eq!(input.cursor_at(0.0, &fonts, &font), 0);
        assert_eq!(input.cursor_at(6.0, &fonts, &font), 1);
        assert_eq!(input.cursor_at(13.0, &fonts, &font), 3);
        assert_eq!(input.cursor_at(100.0, &fonts, &font), 4);
    }

    #[test]
    fn bools() {
        assert!(parse_bool("true"));
        assert!(parse_bool(""));
        assert!(!parse_bool("false"));
        assert!(!parse_bool("0"));
    }
}
