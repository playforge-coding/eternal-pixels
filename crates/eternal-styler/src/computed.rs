// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Styles after the cascade: what layout and painting read.

use crate::values::{
    BorderStyle, Color, Corners, FontFamily, FontStyle, FontWeight, Sides, TextAlign,
};

/// A length in pixels, or a percentage of something layout knows about.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LengthPercentage {
    Px(f32),
    /// A fraction, so `50%` is `Percent(0.5)`.
    Percent(f32),
}

impl LengthPercentage {
    pub const ZERO: LengthPercentage = LengthPercentage::Px(0.0);

    /// The value in pixels, with percentages taken of `basis`.
    pub fn resolve(self, basis: f32) -> f32 {
        match self {
            Self::Px(px) => px,
            Self::Percent(p) => p * basis,
        }
    }
}

/// `width`, `height`, `min-width`, `min-height` and the margins.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum LengthPercentageOrAuto {
    #[default]
    Auto,
    Px(f32),
    /// A fraction, so `50%` is `Percent(0.5)`.
    Percent(f32),
}

impl LengthPercentageOrAuto {
    /// The value in pixels, or `None` for `auto`.
    pub fn resolve(self, basis: f32) -> Option<f32> {
        match self {
            Self::Auto => None,
            Self::Px(px) => Some(px),
            Self::Percent(p) => Some(p * basis),
        }
    }
}

/// `max-width` and `max-height`.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum LengthPercentageOrNone {
    #[default]
    None,
    Px(f32),
    /// A fraction, so `50%` is `Percent(0.5)`.
    Percent(f32),
}

impl LengthPercentageOrNone {
    /// The value in pixels, or `None` for `none`.
    pub fn resolve(self, basis: f32) -> Option<f32> {
        match self {
            Self::None => None,
            Self::Px(px) => Some(px),
            Self::Percent(p) => Some(p * basis),
        }
    }
}

/// `line-height`.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum LineHeight {
    /// Whatever the font's metrics say. Toolkits usually use around 1.2
    /// times the font size.
    #[default]
    Normal,
    /// A multiple of the font size.
    Number(f32),
    Px(f32),
}

impl LineHeight {
    /// The value in pixels for a given font size, or `None` for `normal`.
    pub fn resolve(self, font_size: f32) -> Option<f32> {
        match self {
            Self::Normal => None,
            Self::Number(n) => Some(n * font_size),
            Self::Px(px) => Some(px),
        }
    }
}

/// Every property's value for one element, after the cascade, inheritance
/// and unit resolution.
///
/// Lengths are in pixels. `em` and `rem` are gone; percentages remain, since
/// only layout knows what they are a percentage of. `currentcolor` has been
/// replaced by the element's colour.
///
/// [`Default`] gives the initial value of every property, which is what a
/// root element with no matching rules gets.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedStyle {
    pub width: LengthPercentageOrAuto,
    pub height: LengthPercentageOrAuto,
    pub min_width: LengthPercentageOrAuto,
    pub min_height: LengthPercentageOrAuto,
    pub max_width: LengthPercentageOrNone,
    pub max_height: LengthPercentageOrNone,

    pub margin: Sides<LengthPercentageOrAuto>,
    pub padding: Sides<LengthPercentage>,
    pub row_gap: LengthPercentage,
    pub column_gap: LengthPercentage,

    pub color: Color,
    pub background_color: Color,
    /// `0.0..=1.0`
    pub opacity: f32,

    /// In pixels. Zero wherever the side's style is `None` or `Hidden`,
    /// whatever width was declared.
    pub border_width: Sides<f32>,
    pub border_style: Sides<BorderStyle>,
    pub border_color: Sides<Color>,
    pub border_radius: Corners<LengthPercentage>,

    pub font_family: FontFamily,
    /// In pixels.
    pub font_size: f32,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,
    pub line_height: LineHeight,
    /// In pixels. `normal` is zero.
    pub letter_spacing: f32,
    pub text_align: TextAlign,
}

impl ComputedStyle {
    /// The initial value of every property.
    pub fn initial() -> Self {
        Self {
            width: LengthPercentageOrAuto::Auto,
            height: LengthPercentageOrAuto::Auto,
            min_width: LengthPercentageOrAuto::Auto,
            min_height: LengthPercentageOrAuto::Auto,
            max_width: LengthPercentageOrNone::None,
            max_height: LengthPercentageOrNone::None,
            margin: Sides::all(LengthPercentageOrAuto::Px(0.0)),
            padding: Sides::all(LengthPercentage::ZERO),
            row_gap: LengthPercentage::ZERO,
            column_gap: LengthPercentage::ZERO,
            color: Color::BLACK,
            background_color: Color::TRANSPARENT,
            opacity: 1.0,
            border_width: Sides::all(0.0),
            border_style: Sides::all(BorderStyle::None),
            border_color: Sides::all(Color::BLACK),
            border_radius: Corners::all(LengthPercentage::ZERO),
            font_family: FontFamily::default(),
            font_size: 16.0,
            font_weight: FontWeight::NORMAL,
            font_style: FontStyle::Normal,
            line_height: LineHeight::Normal,
            letter_spacing: 0.0,
            text_align: TextAlign::Start,
        }
    }

    /// Copies the inherited properties from `parent`.
    pub(crate) fn inherit_from(&mut self, parent: &ComputedStyle) {
        self.color = parent.color;
        self.font_family = parent.font_family.clone();
        self.font_size = parent.font_size;
        self.font_weight = parent.font_weight;
        self.font_style = parent.font_style;
        self.line_height = parent.line_height;
        self.letter_spacing = parent.letter_spacing;
        self.text_align = parent.text_align;
    }
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self::initial()
    }
}
