// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! What a rendering backend has to provide.
//!
//! The toolkit never draws anything itself. It measures text through
//! [`Fonts`] and paints through [`Renderer`], and a backend crate implements
//! both for a particular graphics library. `eternal-ui-skia` does so for
//! Skia; the same two traits are all a Vello or tiny-skia backend would need.

use eternal_styler::{Color, ComputedStyle, Corners, FontFamily, FontStyle, FontWeight, Sides};

use crate::geometry::{Point, Rect};

/// The font a piece of text is set in, taken from a computed style.
#[derive(Clone, Debug, PartialEq)]
pub struct FontSpec {
    pub family: FontFamily,
    /// In pixels.
    pub size: f32,
    pub weight: FontWeight,
    pub style: FontStyle,
    /// The line box height in pixels, or `None` for the font's own.
    pub line_height: Option<f32>,
}

impl FontSpec {
    pub fn from_style(style: &ComputedStyle) -> Self {
        Self {
            family: style.font_family.clone(),
            size: style.font_size,
            weight: style.font_weight,
            style: style.font_style,
            line_height: style.line_height.resolve(style.font_size),
        }
    }
}

/// How much room a run of text takes.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct TextMetrics {
    /// The advance width of the whole run.
    pub width: f32,
    /// Distance from the baseline up to the top of the line box.
    pub ascent: f32,
    /// Distance from the baseline down to the bottom of the line box.
    pub descent: f32,
    /// The height of the line box. At least `ascent + descent`, more if the
    /// style asked for a taller line.
    pub line_height: f32,
}

/// Measures text. Implemented by a backend, since only it knows the fonts.
pub trait Fonts {
    fn measure(&self, text: &str, font: &FontSpec) -> TextMetrics;
}

/// Draws. Implemented by a backend for one frame's drawing target.
///
/// Coordinates are window pixels with the origin at the top left. Colours
/// are straight-alpha sRGB. Rectangles are filled and stroked without
/// anti-aliasing where the backend can manage it, so that 1px borders stay
/// crisp.
pub trait Renderer {
    fn fill_rect(&mut self, rect: Rect, radius: Corners<f32>, color: Color);

    /// A border drawn inside `rect`, one width and colour per side.
    fn border(
        &mut self,
        rect: Rect,
        widths: Sides<f32>,
        colors: Sides<Color>,
        radius: Corners<f32>,
    );

    /// A single line of text, its line box starting at `top_left`. The
    /// backend places the baseline inside the line box the same way
    /// [`Fonts::measure`] measured it.
    fn text(&mut self, text: &str, font: &FontSpec, top_left: Point, color: Color);

    /// Restricts drawing to `rect` until the matching [`pop_clip`](Self::pop_clip).
    fn push_clip(&mut self, rect: Rect);

    fn pop_clip(&mut self);

    /// Draws everything until the matching [`pop_opacity`](Self::pop_opacity)
    /// through a layer with the given opacity.
    fn push_opacity(&mut self, opacity: f32);

    fn pop_opacity(&mut self);
}

/// A font source that pretends every glyph is the same width.
///
/// For tests, and for laying out without a real backend. Each character is
/// `advance` times the font size wide; the line box is 1.25 times the font
/// size high unless the style says otherwise.
#[derive(Clone, Debug)]
pub struct MonospaceFonts {
    pub advance: f32,
}

impl Default for MonospaceFonts {
    fn default() -> Self {
        Self { advance: 0.6 }
    }
}

impl Fonts for MonospaceFonts {
    fn measure(&self, text: &str, font: &FontSpec) -> TextMetrics {
        let ascent = font.size * 1.0;
        let descent = font.size * 0.25;
        TextMetrics {
            width: text.chars().count() as f32 * self.advance * font.size,
            ascent,
            descent,
            line_height: font.line_height.unwrap_or(ascent + descent),
        }
    }
}

/// One drawing call, as recorded by [`RecordingRenderer`].
#[derive(Clone, Debug, PartialEq)]
pub enum DrawOp {
    FillRect {
        rect: Rect,
        radius: Corners<f32>,
        color: Color,
    },
    Border {
        rect: Rect,
        widths: Sides<f32>,
        colors: Sides<Color>,
        radius: Corners<f32>,
    },
    Text {
        text: String,
        font: FontSpec,
        top_left: Point,
        color: Color,
    },
    PushClip(Rect),
    PopClip,
    PushOpacity(f32),
    PopOpacity,
}

/// A renderer that records what it was asked to draw, for tests and for
/// inspecting what a frame would contain.
#[derive(Clone, Debug, Default)]
pub struct RecordingRenderer {
    pub ops: Vec<DrawOp>,
}

impl Renderer for RecordingRenderer {
    fn fill_rect(&mut self, rect: Rect, radius: Corners<f32>, color: Color) {
        self.ops.push(DrawOp::FillRect {
            rect,
            radius,
            color,
        });
    }

    fn border(
        &mut self,
        rect: Rect,
        widths: Sides<f32>,
        colors: Sides<Color>,
        radius: Corners<f32>,
    ) {
        self.ops.push(DrawOp::Border {
            rect,
            widths,
            colors,
            radius,
        });
    }

    fn text(&mut self, text: &str, font: &FontSpec, top_left: Point, color: Color) {
        self.ops.push(DrawOp::Text {
            text: text.to_owned(),
            font: font.clone(),
            top_left,
            color,
        });
    }

    fn push_clip(&mut self, rect: Rect) {
        self.ops.push(DrawOp::PushClip(rect));
    }

    fn pop_clip(&mut self) {
        self.ops.push(DrawOp::PopClip);
    }

    fn push_opacity(&mut self, opacity: f32) {
        self.ops.push(DrawOp::PushOpacity(opacity));
    }

    fn pop_opacity(&mut self) {
        self.ops.push(DrawOp::PopOpacity);
    }
}
