// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The properties this crate understands, and how their values are parsed.

use cssparser::Parser;

use crate::error::StyleError;
use crate::specified::{
    self, BorderWidth, ColorValue, FontSize, LengthPercentage, LetterSpacing, LineHeight, MaxSize,
    Size, parse_opacity,
};
use crate::values::{BorderStyle, FontFamily, FontStyle, TextAlign};

macro_rules! longhands {
    ($( $variant:ident: $name:literal, $ty:ty, inherited: $inherited:literal, parse: $parse:expr; )*) => {
        /// Every longhand property. Shorthands expand to these when parsed,
        /// so the cascade only ever deals with longhands.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum LonghandId {
            $( #[doc = concat!("`", $name, "`")] $variant, )*
        }

        impl LonghandId {
            pub const ALL: &'static [LonghandId] = &[ $( LonghandId::$variant, )* ];

            /// The CSS name, such as `border-top-width`.
            pub fn name(self) -> &'static str {
                match self { $( Self::$variant => $name, )* }
            }

            /// Whether the property inherits from the parent by default.
            pub fn is_inherited(self) -> bool {
                match self { $( Self::$variant => $inherited, )* }
            }

            /// Looks a property up by its CSS name, ignoring ASCII case.
            pub fn from_name(name: &str) -> Option<Self> {
                cssparser::match_ignore_ascii_case! { name,
                    $( $name => Some(Self::$variant), )*
                    _ => None,
                }
            }

            pub(crate) fn index(self) -> usize {
                self as usize
            }

            fn parse_value<'i>(
                self,
                input: &mut Parser<'i, '_>,
            ) -> Result<PropertyDeclaration, StyleError<'i>> {
                match self {
                    $( Self::$variant => $parse(input).map(PropertyDeclaration::$variant), )*
                }
            }
        }

        /// A parsed declaration: one longhand and its specified value.
        #[derive(Clone, Debug, PartialEq)]
        pub enum PropertyDeclaration {
            $( #[doc = concat!("`", $name, "`")] $variant($ty), )*
            /// `inherit`, `initial`, `unset` or `revert` for the given longhand.
            CssWide(LonghandId, CssWideKeyword),
        }

        impl PropertyDeclaration {
            pub fn id(&self) -> LonghandId {
                match self {
                    $( Self::$variant(_) => LonghandId::$variant, )*
                    Self::CssWide(id, _) => *id,
                }
            }
        }
    };
}

longhands! {
    // Dimensions
    Width: "width", Size, inherited: false, parse: Size::parse_non_negative;
    Height: "height", Size, inherited: false, parse: Size::parse_non_negative;
    MinWidth: "min-width", Size, inherited: false, parse: Size::parse_non_negative;
    MinHeight: "min-height", Size, inherited: false, parse: Size::parse_non_negative;
    MaxWidth: "max-width", MaxSize, inherited: false, parse: MaxSize::parse;
    MaxHeight: "max-height", MaxSize, inherited: false, parse: MaxSize::parse;
    // Spacing
    MarginTop: "margin-top", Size, inherited: false, parse: Size::parse;
    MarginRight: "margin-right", Size, inherited: false, parse: Size::parse;
    MarginBottom: "margin-bottom", Size, inherited: false, parse: Size::parse;
    MarginLeft: "margin-left", Size, inherited: false, parse: Size::parse;
    PaddingTop: "padding-top", LengthPercentage, inherited: false, parse: LengthPercentage::parse_non_negative;
    PaddingRight: "padding-right", LengthPercentage, inherited: false, parse: LengthPercentage::parse_non_negative;
    PaddingBottom: "padding-bottom", LengthPercentage, inherited: false, parse: LengthPercentage::parse_non_negative;
    PaddingLeft: "padding-left", LengthPercentage, inherited: false, parse: LengthPercentage::parse_non_negative;
    RowGap: "row-gap", LengthPercentage, inherited: false, parse: LengthPercentage::parse_non_negative;
    ColumnGap: "column-gap", LengthPercentage, inherited: false, parse: LengthPercentage::parse_non_negative;
    // Colours
    Color: "color", ColorValue, inherited: true, parse: ColorValue::parse;
    BackgroundColor: "background-color", ColorValue, inherited: false, parse: ColorValue::parse;
    Opacity: "opacity", f32, inherited: false, parse: parse_opacity;
    // Borders
    BorderTopWidth: "border-top-width", BorderWidth, inherited: false, parse: BorderWidth::parse;
    BorderRightWidth: "border-right-width", BorderWidth, inherited: false, parse: BorderWidth::parse;
    BorderBottomWidth: "border-bottom-width", BorderWidth, inherited: false, parse: BorderWidth::parse;
    BorderLeftWidth: "border-left-width", BorderWidth, inherited: false, parse: BorderWidth::parse;
    BorderTopStyle: "border-top-style", BorderStyle, inherited: false, parse: BorderStyle::parse;
    BorderRightStyle: "border-right-style", BorderStyle, inherited: false, parse: BorderStyle::parse;
    BorderBottomStyle: "border-bottom-style", BorderStyle, inherited: false, parse: BorderStyle::parse;
    BorderLeftStyle: "border-left-style", BorderStyle, inherited: false, parse: BorderStyle::parse;
    BorderTopColor: "border-top-color", ColorValue, inherited: false, parse: ColorValue::parse;
    BorderRightColor: "border-right-color", ColorValue, inherited: false, parse: ColorValue::parse;
    BorderBottomColor: "border-bottom-color", ColorValue, inherited: false, parse: ColorValue::parse;
    BorderLeftColor: "border-left-color", ColorValue, inherited: false, parse: ColorValue::parse;
    BorderTopLeftRadius: "border-top-left-radius", LengthPercentage, inherited: false, parse: LengthPercentage::parse_non_negative;
    BorderTopRightRadius: "border-top-right-radius", LengthPercentage, inherited: false, parse: LengthPercentage::parse_non_negative;
    BorderBottomRightRadius: "border-bottom-right-radius", LengthPercentage, inherited: false, parse: LengthPercentage::parse_non_negative;
    BorderBottomLeftRadius: "border-bottom-left-radius", LengthPercentage, inherited: false, parse: LengthPercentage::parse_non_negative;
    // Typography
    FontFamily: "font-family", FontFamily, inherited: true, parse: FontFamily::parse;
    FontSize: "font-size", FontSize, inherited: true, parse: FontSize::parse;
    FontWeight: "font-weight", specified::FontWeight, inherited: true, parse: specified::FontWeight::parse;
    FontStyle: "font-style", FontStyle, inherited: true, parse: FontStyle::parse;
    LineHeight: "line-height", LineHeight, inherited: true, parse: LineHeight::parse;
    LetterSpacing: "letter-spacing", LetterSpacing, inherited: true, parse: LetterSpacing::parse;
    TextAlign: "text-align", TextAlign, inherited: true, parse: TextAlign::parse;
}

/// The number of longhands, for tables indexed by [`LonghandId::index`].
pub(crate) const LONGHAND_COUNT: usize = LonghandId::ALL.len();

/// A shorthand property: a single declaration that sets several longhands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShorthandId {
    /// `margin`: one to four values, top, right, bottom, left.
    Margin,
    /// `padding`: one to four values.
    Padding,
    /// `gap`: row gap, then an optional column gap.
    Gap,
    /// `border`: any of a width, a style and a colour, in any order, for all
    /// four sides.
    Border,
    /// `border-top`: like `border` for one side.
    BorderTop,
    BorderRight,
    BorderBottom,
    BorderLeft,
    /// `border-width`: one to four values.
    BorderWidth,
    /// `border-style`: one to four values.
    BorderStyle,
    /// `border-color`: one to four values.
    BorderColor,
    /// `border-radius`: one to four values, top-left, top-right,
    /// bottom-right, bottom-left. Elliptical corners (`a / b`) are not
    /// supported.
    BorderRadius,
}

impl ShorthandId {
    pub const ALL: &'static [ShorthandId] = &[
        Self::Margin,
        Self::Padding,
        Self::Gap,
        Self::Border,
        Self::BorderTop,
        Self::BorderRight,
        Self::BorderBottom,
        Self::BorderLeft,
        Self::BorderWidth,
        Self::BorderStyle,
        Self::BorderColor,
        Self::BorderRadius,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Margin => "margin",
            Self::Padding => "padding",
            Self::Gap => "gap",
            Self::Border => "border",
            Self::BorderTop => "border-top",
            Self::BorderRight => "border-right",
            Self::BorderBottom => "border-bottom",
            Self::BorderLeft => "border-left",
            Self::BorderWidth => "border-width",
            Self::BorderStyle => "border-style",
            Self::BorderColor => "border-color",
            Self::BorderRadius => "border-radius",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        cssparser::match_ignore_ascii_case! { name,
            "margin" => Some(Self::Margin),
            "padding" => Some(Self::Padding),
            "gap" => Some(Self::Gap),
            "border" => Some(Self::Border),
            "border-top" => Some(Self::BorderTop),
            "border-right" => Some(Self::BorderRight),
            "border-bottom" => Some(Self::BorderBottom),
            "border-left" => Some(Self::BorderLeft),
            "border-width" => Some(Self::BorderWidth),
            "border-style" => Some(Self::BorderStyle),
            "border-color" => Some(Self::BorderColor),
            "border-radius" => Some(Self::BorderRadius),
            _ => None,
        }
    }

    /// The longhands this shorthand sets.
    pub fn longhands(self) -> &'static [LonghandId] {
        use LonghandId::*;
        match self {
            Self::Margin => &[MarginTop, MarginRight, MarginBottom, MarginLeft],
            Self::Padding => &[PaddingTop, PaddingRight, PaddingBottom, PaddingLeft],
            Self::Gap => &[RowGap, ColumnGap],
            Self::Border => &[
                BorderTopWidth,
                BorderRightWidth,
                BorderBottomWidth,
                BorderLeftWidth,
                BorderTopStyle,
                BorderRightStyle,
                BorderBottomStyle,
                BorderLeftStyle,
                BorderTopColor,
                BorderRightColor,
                BorderBottomColor,
                BorderLeftColor,
            ],
            Self::BorderTop => &[BorderTopWidth, BorderTopStyle, BorderTopColor],
            Self::BorderRight => &[BorderRightWidth, BorderRightStyle, BorderRightColor],
            Self::BorderBottom => &[BorderBottomWidth, BorderBottomStyle, BorderBottomColor],
            Self::BorderLeft => &[BorderLeftWidth, BorderLeftStyle, BorderLeftColor],
            Self::BorderWidth => &[
                BorderTopWidth,
                BorderRightWidth,
                BorderBottomWidth,
                BorderLeftWidth,
            ],
            Self::BorderStyle => &[
                BorderTopStyle,
                BorderRightStyle,
                BorderBottomStyle,
                BorderLeftStyle,
            ],
            Self::BorderColor => &[
                BorderTopColor,
                BorderRightColor,
                BorderBottomColor,
                BorderLeftColor,
            ],
            Self::BorderRadius => &[
                BorderTopLeftRadius,
                BorderTopRightRadius,
                BorderBottomRightRadius,
                BorderBottomLeftRadius,
            ],
        }
    }

    fn parse_into<'i>(
        self,
        input: &mut Parser<'i, '_>,
        out: &mut Vec<PropertyDeclaration>,
    ) -> Result<(), StyleError<'i>> {
        use PropertyDeclaration as D;
        match self {
            Self::Margin => {
                let [top, right, bottom, left] = parse_sides(input, Size::parse)?;
                out.extend([
                    D::MarginTop(top),
                    D::MarginRight(right),
                    D::MarginBottom(bottom),
                    D::MarginLeft(left),
                ]);
            }
            Self::Padding => {
                let [top, right, bottom, left] =
                    parse_sides(input, LengthPercentage::parse_non_negative)?;
                out.extend([
                    D::PaddingTop(top),
                    D::PaddingRight(right),
                    D::PaddingBottom(bottom),
                    D::PaddingLeft(left),
                ]);
            }
            Self::Gap => {
                let row = LengthPercentage::parse_non_negative(input)?;
                let column = input
                    .try_parse(LengthPercentage::parse_non_negative)
                    .unwrap_or(row);
                out.extend([D::RowGap(row), D::ColumnGap(column)]);
            }
            Self::Border => {
                let (width, style, color) = parse_border(input)?;
                out.extend([
                    D::BorderTopWidth(width),
                    D::BorderRightWidth(width),
                    D::BorderBottomWidth(width),
                    D::BorderLeftWidth(width),
                    D::BorderTopStyle(style),
                    D::BorderRightStyle(style),
                    D::BorderBottomStyle(style),
                    D::BorderLeftStyle(style),
                    D::BorderTopColor(color),
                    D::BorderRightColor(color),
                    D::BorderBottomColor(color),
                    D::BorderLeftColor(color),
                ]);
            }
            Self::BorderTop => {
                let (width, style, color) = parse_border(input)?;
                out.extend([
                    D::BorderTopWidth(width),
                    D::BorderTopStyle(style),
                    D::BorderTopColor(color),
                ]);
            }
            Self::BorderRight => {
                let (width, style, color) = parse_border(input)?;
                out.extend([
                    D::BorderRightWidth(width),
                    D::BorderRightStyle(style),
                    D::BorderRightColor(color),
                ]);
            }
            Self::BorderBottom => {
                let (width, style, color) = parse_border(input)?;
                out.extend([
                    D::BorderBottomWidth(width),
                    D::BorderBottomStyle(style),
                    D::BorderBottomColor(color),
                ]);
            }
            Self::BorderLeft => {
                let (width, style, color) = parse_border(input)?;
                out.extend([
                    D::BorderLeftWidth(width),
                    D::BorderLeftStyle(style),
                    D::BorderLeftColor(color),
                ]);
            }
            Self::BorderWidth => {
                let [top, right, bottom, left] = parse_sides(input, BorderWidth::parse)?;
                out.extend([
                    D::BorderTopWidth(top),
                    D::BorderRightWidth(right),
                    D::BorderBottomWidth(bottom),
                    D::BorderLeftWidth(left),
                ]);
            }
            Self::BorderStyle => {
                let [top, right, bottom, left] = parse_sides(input, BorderStyle::parse)?;
                out.extend([
                    D::BorderTopStyle(top),
                    D::BorderRightStyle(right),
                    D::BorderBottomStyle(bottom),
                    D::BorderLeftStyle(left),
                ]);
            }
            Self::BorderColor => {
                let [top, right, bottom, left] = parse_sides(input, ColorValue::parse)?;
                out.extend([
                    D::BorderTopColor(top),
                    D::BorderRightColor(right),
                    D::BorderBottomColor(bottom),
                    D::BorderLeftColor(left),
                ]);
            }
            Self::BorderRadius => {
                let [top_left, top_right, bottom_right, bottom_left] =
                    parse_sides(input, LengthPercentage::parse_non_negative)?;
                out.extend([
                    D::BorderTopLeftRadius(top_left),
                    D::BorderTopRightRadius(top_right),
                    D::BorderBottomRightRadius(bottom_right),
                    D::BorderBottomLeftRadius(bottom_left),
                ]);
            }
        }
        Ok(())
    }
}

/// Parses one to four values and spreads them over four sides the way
/// `margin` and friends do.
fn parse_sides<'i, T: Copy>(
    input: &mut Parser<'i, '_>,
    parse: fn(&mut Parser<'i, '_>) -> Result<T, StyleError<'i>>,
) -> Result<[T; 4], StyleError<'i>> {
    let first = parse(input)?;
    let Ok(second) = input.try_parse(parse) else {
        return Ok([first; 4]);
    };
    let Ok(third) = input.try_parse(parse) else {
        return Ok([first, second, first, second]);
    };
    let Ok(fourth) = input.try_parse(parse) else {
        return Ok([first, second, third, second]);
    };
    Ok([first, second, third, fourth])
}

/// Parses `<width> || <style> || <color>`, at least one of them, in any
/// order. Whatever is missing takes its initial value.
fn parse_border<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<(BorderWidth, BorderStyle, ColorValue), StyleError<'i>> {
    let mut width = None;
    let mut style = None;
    let mut color = None;
    loop {
        if width.is_none()
            && let Ok(value) = input.try_parse(BorderWidth::parse)
        {
            width = Some(value);
            continue;
        }
        if style.is_none()
            && let Ok(value) = input.try_parse(BorderStyle::parse)
        {
            style = Some(value);
            continue;
        }
        if color.is_none()
            && let Ok(value) = input.try_parse(ColorValue::parse)
        {
            color = Some(value);
            continue;
        }
        break;
    }
    if width.is_none() && style.is_none() && color.is_none() {
        return Err(input.new_error_for_next_token());
    }
    Ok((
        width.unwrap_or(BorderWidth::Medium),
        style.unwrap_or(BorderStyle::None),
        color.unwrap_or(ColorValue::CurrentColor),
    ))
}

/// Any property name this crate accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PropertyId {
    Longhand(LonghandId),
    Shorthand(ShorthandId),
}

impl PropertyId {
    pub fn from_name(name: &str) -> Option<Self> {
        LonghandId::from_name(name)
            .map(Self::Longhand)
            .or_else(|| ShorthandId::from_name(name).map(Self::Shorthand))
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Longhand(id) => id.name(),
            Self::Shorthand(id) => id.name(),
        }
    }

    /// The longhands a declaration of this property sets.
    pub fn longhands(self) -> &'static [LonghandId] {
        match self {
            Self::Longhand(id) => std::slice::from_ref(longhand_ref(id)),
            Self::Shorthand(id) => id.longhands(),
        }
    }
}

fn longhand_ref(id: LonghandId) -> &'static LonghandId {
    &LonghandId::ALL[id.index()]
}

/// The keywords every property accepts in place of a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CssWideKeyword {
    /// The property's initial value.
    Initial,
    /// The parent's computed value, whether or not the property inherits by
    /// default.
    Inherit,
    /// `inherit` for inherited properties, `initial` for the rest.
    Unset,
    /// Treated as `unset`: there is no user-agent stylesheet to revert to.
    Revert,
}

impl CssWideKeyword {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, StyleError<'i>> {
        input.skip_whitespace();
        let location = input.current_source_location();
        let ident = input.expect_ident()?;
        cssparser::match_ignore_ascii_case! { ident,
            "initial" => Ok(Self::Initial),
            "inherit" => Ok(Self::Inherit),
            "unset" => Ok(Self::Unset),
            "revert" => Ok(Self::Revert),
            _ => Err(location.new_unexpected_token_error(cssparser::Token::Ident(ident.clone()))),
        }
    }

    /// What the keyword means for a property that does or does not inherit.
    pub(crate) fn resolve(self, inherited: bool) -> Resolution {
        match self {
            Self::Initial => Resolution::Initial,
            Self::Inherit => Resolution::Inherit,
            Self::Unset | Self::Revert if inherited => Resolution::Inherit,
            Self::Unset | Self::Revert => Resolution::Initial,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Resolution {
    Initial,
    Inherit,
}

/// Parses the value of `id` and appends the resulting longhand declarations
/// to `out`. Does not check that the input is exhausted afterwards.
pub(crate) fn parse_property_value<'i>(
    id: PropertyId,
    input: &mut Parser<'i, '_>,
    out: &mut Vec<PropertyDeclaration>,
) -> Result<(), StyleError<'i>> {
    if let Ok(keyword) = input.try_parse(CssWideKeyword::parse) {
        out.extend(
            id.longhands()
                .iter()
                .map(|&longhand| PropertyDeclaration::CssWide(longhand, keyword)),
        );
        return Ok(());
    }
    match id {
        PropertyId::Longhand(id) => out.push(id.parse_value(input)?),
        PropertyId::Shorthand(id) => id.parse_into(input, out)?,
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::specified::Length;
    use cssparser::ParserInput;

    fn parse(property: &str, css: &str) -> Result<Vec<PropertyDeclaration>, String> {
        let id = PropertyId::from_name(property).expect("known property");
        let mut input = ParserInput::new(css);
        let mut parser = Parser::new(&mut input);
        let mut out = Vec::new();
        parser
            .parse_entirely(|input| parse_property_value(id, input, &mut out))
            .map_err(|e| crate::Error::from_parse_error(e).to_string())?;
        Ok(out)
    }

    fn px(v: f32) -> LengthPercentage {
        LengthPercentage::Length(Length::Px(v))
    }

    #[test]
    fn every_name_round_trips() {
        for &id in LonghandId::ALL {
            assert_eq!(LonghandId::from_name(id.name()), Some(id));
            assert_eq!(
                LonghandId::from_name(&id.name().to_ascii_uppercase()),
                Some(id)
            );
            assert!(ShorthandId::from_name(id.name()).is_none());
        }
        for &id in ShorthandId::ALL {
            assert_eq!(ShorthandId::from_name(id.name()), Some(id));
            assert!(LonghandId::from_name(id.name()).is_none());
            assert!(!id.longhands().is_empty());
        }
        assert_eq!(PropertyId::from_name("colour"), None);
        assert_eq!(LONGHAND_COUNT, LonghandId::ALL.len());
        for (index, &id) in LonghandId::ALL.iter().enumerate() {
            assert_eq!(id.index(), index);
        }
    }

    #[test]
    fn sides_spread_like_css() {
        use PropertyDeclaration as D;
        assert_eq!(
            parse("padding", "1px").unwrap(),
            [
                D::PaddingTop(px(1.0)),
                D::PaddingRight(px(1.0)),
                D::PaddingBottom(px(1.0)),
                D::PaddingLeft(px(1.0)),
            ]
        );
        assert_eq!(
            parse("padding", "1px 2px").unwrap(),
            [
                D::PaddingTop(px(1.0)),
                D::PaddingRight(px(2.0)),
                D::PaddingBottom(px(1.0)),
                D::PaddingLeft(px(2.0)),
            ]
        );
        assert_eq!(
            parse("padding", "1px 2px 3px").unwrap(),
            [
                D::PaddingTop(px(1.0)),
                D::PaddingRight(px(2.0)),
                D::PaddingBottom(px(3.0)),
                D::PaddingLeft(px(2.0)),
            ]
        );
        assert_eq!(
            parse("padding", "1px 2px 3px 4px").unwrap(),
            [
                D::PaddingTop(px(1.0)),
                D::PaddingRight(px(2.0)),
                D::PaddingBottom(px(3.0)),
                D::PaddingLeft(px(4.0)),
            ]
        );
        assert!(parse("padding", "1px 2px 3px 4px 5px").is_err());
        assert!(parse("padding", "-1px").is_err());
        assert_eq!(
            parse("margin", "-1px auto").unwrap()[1],
            D::MarginRight(Size::Auto)
        );
        assert_eq!(
            parse("gap", "4px").unwrap(),
            [D::RowGap(px(4.0)), D::ColumnGap(px(4.0))]
        );
        assert_eq!(
            parse("gap", "4px 8px").unwrap(),
            [D::RowGap(px(4.0)), D::ColumnGap(px(8.0))]
        );
    }

    #[test]
    fn border_takes_its_parts_in_any_order() {
        use PropertyDeclaration as D;
        let red = ColorValue::Color(crate::Color::from_rgba8(255, 0, 0, 255));
        let decls = parse("border-top", "red 2px solid").unwrap();
        assert_eq!(
            decls,
            [
                D::BorderTopWidth(BorderWidth::Length(Length::Px(2.0))),
                D::BorderTopStyle(BorderStyle::Solid),
                D::BorderTopColor(red),
            ]
        );
        let decls = parse("border-left", "dashed").unwrap();
        assert_eq!(
            decls,
            [
                D::BorderLeftWidth(BorderWidth::Medium),
                D::BorderLeftStyle(BorderStyle::Dashed),
                D::BorderLeftColor(ColorValue::CurrentColor),
            ]
        );
        assert_eq!(parse("border", "thin solid").unwrap().len(), 12);
        assert!(parse("border", "").is_err());
        assert!(parse("border", "solid solid").is_err());
        assert!(parse("border", "2px 3px").is_err());
    }

    #[test]
    fn css_wide_keywords_expand_to_every_longhand() {
        let decls = parse("border", "inherit").unwrap();
        assert_eq!(decls.len(), 12);
        assert!(
            decls
                .iter()
                .all(|d| matches!(d, PropertyDeclaration::CssWide(_, CssWideKeyword::Inherit)))
        );
        assert_eq!(
            parse("color", "UNSET").unwrap(),
            [PropertyDeclaration::CssWide(
                LonghandId::Color,
                CssWideKeyword::Unset
            )]
        );
        assert!(parse("color", "inherit red").is_err());
    }
}
