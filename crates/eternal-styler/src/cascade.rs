// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Turning the winning declarations for an element into a computed style.

use crate::computed::{
    ComputedStyle, LengthPercentage, LengthPercentageOrAuto, LengthPercentageOrNone, LineHeight,
};
use crate::properties::{LONGHAND_COUNT, LonghandId, PropertyDeclaration, Resolution};
use crate::specified::{self, BorderWidth, ColorValue, MaxSize, Size};
use crate::values::{self, Sides};

/// The declaration that won the cascade for each longhand, if any did.
pub(crate) struct Winners<'a>([Option<&'a PropertyDeclaration>; LONGHAND_COUNT]);

impl<'a> Winners<'a> {
    pub(crate) fn new() -> Self {
        Self([None; LONGHAND_COUNT])
    }

    /// Records `declaration` as the current winner for its longhand,
    /// replacing whatever came before.
    pub(crate) fn set(&mut self, declaration: &'a PropertyDeclaration) {
        self.0[declaration.id().index()] = Some(declaration);
    }

    fn get(&self, id: LonghandId) -> Option<&'a PropertyDeclaration> {
        self.0[id.index()]
    }
}

/// Sets one field of `style` from its winning declaration: the computed
/// value if there is one, the parent's or initial value for a CSS-wide
/// keyword, and nothing at all otherwise (the field already holds the
/// inherited or initial value).
macro_rules! apply {
    (
        $winners:ident, $parent:ident, $initial:ident,
        $style:ident . $($field:tt).+ = $id:ident,
        |$value:ident| $compute:expr
    ) => {
        match $winners.get(LonghandId::$id) {
            Some(PropertyDeclaration::$id($value)) => $style.$($field).+ = $compute,
            Some(PropertyDeclaration::CssWide(_, keyword)) => {
                match keyword.resolve(LonghandId::$id.is_inherited()) {
                    Resolution::Inherit => $style.$($field).+ = $parent.$($field).+.clone(),
                    Resolution::Initial => $style.$($field).+ = $initial.$($field).+.clone(),
                }
            }
            _ => {}
        }
    };
}

/// Computes the style of an element from its winning declarations, its
/// parent's computed style (or the initial style for the root) and the root
/// font size `rem` is relative to.
pub(crate) fn compute(
    winners: &Winners<'_>,
    parent: Option<&ComputedStyle>,
    root_font_size: f32,
) -> ComputedStyle {
    let root_parent = ComputedStyle::initial();
    let parent = parent.unwrap_or(&root_parent);
    let mut initial = ComputedStyle::initial();
    let mut style = ComputedStyle::initial();
    style.inherit_from(parent);

    // Font size first: every other `em` depends on it.
    apply! { winners, parent, initial, style.font_size = FontSize, |v| match v {
        specified::FontSize::Length(length) => length.to_px(parent.font_size, root_font_size),
        specified::FontSize::Percent(p) => parent.font_size * p,
        specified::FontSize::Keyword(keyword) => keyword.to_px(),
        specified::FontSize::Larger => parent.font_size * 1.2,
        specified::FontSize::Smaller => parent.font_size / 1.2,
    } }
    let font_size = style.font_size;
    let px = |length: &specified::Length| length.to_px(font_size, root_font_size);

    apply! { winners, parent, initial, style.font_family = FontFamily, |v| v.clone() }
    apply! { winners, parent, initial, style.font_weight = FontWeight, |v| match v {
        specified::FontWeight::Absolute(weight) => values::FontWeight::new(*weight),
        specified::FontWeight::Bolder => parent.font_weight.bolder(),
        specified::FontWeight::Lighter => parent.font_weight.lighter(),
    } }
    apply! { winners, parent, initial, style.font_style = FontStyle, |v| *v }
    apply! { winners, parent, initial, style.line_height = LineHeight, |v| match v {
        specified::LineHeight::Normal => LineHeight::Normal,
        specified::LineHeight::Number(n) => LineHeight::Number(*n),
        specified::LineHeight::Length(length) => LineHeight::Px(px(length)),
        specified::LineHeight::Percent(p) => LineHeight::Px(font_size * p),
    } }
    apply! { winners, parent, initial, style.letter_spacing = LetterSpacing, |v| match v {
        specified::LetterSpacing::Normal => 0.0,
        specified::LetterSpacing::Length(length) => px(length),
    } }
    apply! { winners, parent, initial, style.text_align = TextAlign, |v| *v }

    // Then colour: `currentcolor` elsewhere means this element's colour, but
    // in `color` itself it means the parent's.
    apply! { winners, parent, initial, style.color = Color, |v| match v {
        ColorValue::CurrentColor => parent.color,
        ColorValue::Color(color) => *color,
    } }
    let current_color = style.color;
    let color = |value: &ColorValue| match value {
        ColorValue::CurrentColor => current_color,
        ColorValue::Color(color) => *color,
    };
    // The initial border colour is `currentcolor`, which is only known now.
    // The initial border width is `medium`; sides without a visible style
    // are zeroed at the end.
    initial.border_color = Sides::all(current_color);
    initial.border_width = Sides::all(3.0);

    apply! { winners, parent, initial, style.background_color = BackgroundColor, |v| color(v) }
    apply! { winners, parent, initial, style.opacity = Opacity, |v| *v }

    let length_percentage = |value: &specified::LengthPercentage| match value {
        specified::LengthPercentage::Length(length) => LengthPercentage::Px(px(length)),
        specified::LengthPercentage::Percent(p) => LengthPercentage::Percent(*p),
    };
    let size = |value: &Size| match value {
        Size::Auto => LengthPercentageOrAuto::Auto,
        Size::LengthPercentage(specified::LengthPercentage::Length(length)) => {
            LengthPercentageOrAuto::Px(px(length))
        }
        Size::LengthPercentage(specified::LengthPercentage::Percent(p)) => {
            LengthPercentageOrAuto::Percent(*p)
        }
    };
    let max_size = |value: &MaxSize| match value {
        MaxSize::None => LengthPercentageOrNone::None,
        MaxSize::LengthPercentage(specified::LengthPercentage::Length(length)) => {
            LengthPercentageOrNone::Px(px(length))
        }
        MaxSize::LengthPercentage(specified::LengthPercentage::Percent(p)) => {
            LengthPercentageOrNone::Percent(*p)
        }
    };

    apply! { winners, parent, initial, style.width = Width, |v| size(v) }
    apply! { winners, parent, initial, style.height = Height, |v| size(v) }
    apply! { winners, parent, initial, style.min_width = MinWidth, |v| size(v) }
    apply! { winners, parent, initial, style.min_height = MinHeight, |v| size(v) }
    apply! { winners, parent, initial, style.max_width = MaxWidth, |v| max_size(v) }
    apply! { winners, parent, initial, style.max_height = MaxHeight, |v| max_size(v) }

    apply! { winners, parent, initial, style.margin.top = MarginTop, |v| size(v) }
    apply! { winners, parent, initial, style.margin.right = MarginRight, |v| size(v) }
    apply! { winners, parent, initial, style.margin.bottom = MarginBottom, |v| size(v) }
    apply! { winners, parent, initial, style.margin.left = MarginLeft, |v| size(v) }
    apply! { winners, parent, initial, style.padding.top = PaddingTop, |v| length_percentage(v) }
    apply! { winners, parent, initial, style.padding.right = PaddingRight, |v| length_percentage(v) }
    apply! { winners, parent, initial, style.padding.bottom = PaddingBottom, |v| length_percentage(v) }
    apply! { winners, parent, initial, style.padding.left = PaddingLeft, |v| length_percentage(v) }
    apply! { winners, parent, initial, style.row_gap = RowGap, |v| length_percentage(v) }
    apply! { winners, parent, initial, style.column_gap = ColumnGap, |v| length_percentage(v) }

    apply! { winners, parent, initial, style.border_style.top = BorderTopStyle, |v| *v }
    apply! { winners, parent, initial, style.border_style.right = BorderRightStyle, |v| *v }
    apply! { winners, parent, initial, style.border_style.bottom = BorderBottomStyle, |v| *v }
    apply! { winners, parent, initial, style.border_style.left = BorderLeftStyle, |v| *v }
    apply! { winners, parent, initial, style.border_color.top = BorderTopColor, |v| color(v) }
    apply! { winners, parent, initial, style.border_color.right = BorderRightColor, |v| color(v) }
    apply! { winners, parent, initial, style.border_color.bottom = BorderBottomColor, |v| color(v) }
    apply! { winners, parent, initial, style.border_color.left = BorderLeftColor, |v| color(v) }

    let border_width = |value: &BorderWidth| match value {
        BorderWidth::Thin => 1.0,
        BorderWidth::Medium => 3.0,
        BorderWidth::Thick => 5.0,
        BorderWidth::Length(length) => px(length),
    };
    apply! { winners, parent, initial, style.border_width.top = BorderTopWidth, |v| border_width(v) }
    apply! { winners, parent, initial, style.border_width.right = BorderRightWidth, |v| border_width(v) }
    apply! { winners, parent, initial, style.border_width.bottom = BorderBottomWidth, |v| border_width(v) }
    apply! { winners, parent, initial, style.border_width.left = BorderLeftWidth, |v| border_width(v) }
    // A border without a visible style takes no space.
    if !style.border_style.top.is_visible() {
        style.border_width.top = 0.0;
    }
    if !style.border_style.right.is_visible() {
        style.border_width.right = 0.0;
    }
    if !style.border_style.bottom.is_visible() {
        style.border_width.bottom = 0.0;
    }
    if !style.border_style.left.is_visible() {
        style.border_width.left = 0.0;
    }

    apply! { winners, parent, initial, style.border_radius.top_left = BorderTopLeftRadius, |v| length_percentage(v) }
    apply! { winners, parent, initial, style.border_radius.top_right = BorderTopRightRadius, |v| length_percentage(v) }
    apply! { winners, parent, initial, style.border_radius.bottom_right = BorderBottomRightRadius, |v| length_percentage(v) }
    apply! { winners, parent, initial, style.border_radius.bottom_left = BorderBottomLeftRadius, |v| length_percentage(v) }

    style
}
