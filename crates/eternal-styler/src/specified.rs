// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Values as written in a stylesheet, before the cascade resolves them.
//!
//! These are what [`PropertyDeclaration`](crate::PropertyDeclaration) carries.
//! Relative units (`em`, `rem`, percentages of the parent's font size) and
//! keywords such as `currentcolor` or `bolder` survive here and are only
//! resolved when a [`ComputedStyle`](crate::ComputedStyle) is built.

use cssparser::{Parser, Token};
use cssparser_color::{ColorParser, FromParsedColor, parse_color_with};

use crate::error::{StyleError, StyleParseErrorKind};
use crate::values::{BorderStyle, Color, FontFamily, FontStyle, TextAlign};

/// Matches an identifier against keywords, ASCII case-insensitively, and
/// fails with an "unexpected token" error at the identifier otherwise.
macro_rules! keyword {
    ($input:expr, { $( $name:literal => $value:expr ),+ $(,)? }) => {{
        $input.skip_whitespace();
        let location = $input.current_source_location();
        let ident = $input.expect_ident()?;
        cssparser::match_ignore_ascii_case! { ident,
            $( $name => Ok($value), )+
            _ => Err(location.new_unexpected_token_error(Token::Ident(ident.clone()))),
        }
    }};
}

/// A length with its unit still attached, so that `em` and `rem` can be
/// resolved against the right font size later. Absolute units other than
/// `px` are converted to pixels when parsed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Length {
    Px(f32),
    /// Relative to the element's font size (the parent's, for `font-size`
    /// itself).
    Em(f32),
    /// Relative to the root font size, see
    /// [`Styler::set_root_font_size`](crate::Styler::set_root_font_size).
    Rem(f32),
}

impl Length {
    pub const ZERO: Length = Length::Px(0.0);

    pub(crate) fn to_px(self, font_size: f32, root_font_size: f32) -> f32 {
        match self {
            Self::Px(px) => px,
            Self::Em(em) => em * font_size,
            Self::Rem(rem) => rem * root_font_size,
        }
    }

    fn from_dimension(value: f32, unit: &str) -> Option<Self> {
        Some(cssparser::match_ignore_ascii_case! { unit,
            "px" => Self::Px(value),
            "em" => Self::Em(value),
            "rem" => Self::Rem(value),
            "pt" => Self::Px(value * 96.0 / 72.0),
            "pc" => Self::Px(value * 16.0),
            "in" => Self::Px(value * 96.0),
            "cm" => Self::Px(value * 96.0 / 2.54),
            "mm" => Self::Px(value * 96.0 / 25.4),
            "q" => Self::Px(value * 96.0 / 101.6),
            _ => return None,
        })
    }

    fn value(self) -> f32 {
        match self {
            Self::Px(v) | Self::Em(v) | Self::Rem(v) => v,
        }
    }

    pub(crate) fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, StyleError<'i>> {
        input.skip_whitespace();
        let location = input.current_source_location();
        let token = input.next()?.clone();
        let length = match &token {
            Token::Dimension { value, unit, .. } => Self::from_dimension(*value, unit),
            Token::Number { value, .. } if *value == 0.0 => Some(Self::ZERO),
            _ => None,
        };
        length.ok_or_else(|| location.new_unexpected_token_error(token))
    }

    pub(crate) fn parse_non_negative<'i>(
        input: &mut Parser<'i, '_>,
    ) -> Result<Self, StyleError<'i>> {
        input.skip_whitespace();
        let location = input.current_source_location();
        let length = Self::parse(input)?;
        if length.value() < 0.0 {
            return Err(location.new_custom_error(StyleParseErrorKind::NegativeValue));
        }
        Ok(length)
    }
}

/// A length or a percentage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LengthPercentage {
    Length(Length),
    /// A fraction, so `50%` is `Percent(0.5)`.
    Percent(f32),
}

impl LengthPercentage {
    fn value(self) -> f32 {
        match self {
            Self::Length(length) => length.value(),
            Self::Percent(p) => p,
        }
    }

    pub(crate) fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, StyleError<'i>> {
        if let Ok(unit_value) = input.try_parse(|input| input.expect_percentage()) {
            return Ok(Self::Percent(unit_value));
        }
        Length::parse(input).map(Self::Length)
    }

    pub(crate) fn parse_non_negative<'i>(
        input: &mut Parser<'i, '_>,
    ) -> Result<Self, StyleError<'i>> {
        input.skip_whitespace();
        let location = input.current_source_location();
        let value = Self::parse(input)?;
        if value.value() < 0.0 {
            return Err(location.new_custom_error(StyleParseErrorKind::NegativeValue));
        }
        Ok(value)
    }
}

/// `auto` or a length or percentage: `width`, `height`, `min-*` and the
/// margins.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Size {
    Auto,
    LengthPercentage(LengthPercentage),
}

impl Size {
    fn parse_with<'i>(
        input: &mut Parser<'i, '_>,
        parse: fn(&mut Parser<'i, '_>) -> Result<LengthPercentage, StyleError<'i>>,
    ) -> Result<Self, StyleError<'i>> {
        if input
            .try_parse(|input| input.expect_ident_matching("auto"))
            .is_ok()
        {
            return Ok(Self::Auto);
        }
        parse(input).map(Self::LengthPercentage)
    }

    /// For margins, which may be negative.
    pub(crate) fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, StyleError<'i>> {
        Self::parse_with(input, LengthPercentage::parse)
    }

    /// For sizes, which may not.
    pub(crate) fn parse_non_negative<'i>(
        input: &mut Parser<'i, '_>,
    ) -> Result<Self, StyleError<'i>> {
        Self::parse_with(input, LengthPercentage::parse_non_negative)
    }
}

/// `none` or a length or percentage: `max-width` and `max-height`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MaxSize {
    None,
    LengthPercentage(LengthPercentage),
}

impl MaxSize {
    pub(crate) fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, StyleError<'i>> {
        if input
            .try_parse(|input| input.expect_ident_matching("none"))
            .is_ok()
        {
            return Ok(Self::None);
        }
        LengthPercentage::parse_non_negative(input).map(Self::LengthPercentage)
    }
}

/// A colour, or `currentcolor`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColorValue {
    CurrentColor,
    Color(Color),
}

impl ColorValue {
    pub(crate) fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, StyleError<'i>> {
        input.skip_whitespace();
        let location = input.current_source_location();
        match parse_color_with(&ColorParserImpl, input)? {
            ParsedColor::Value(value) => Ok(value),
            ParsedColor::Unsupported(space) => {
                Err(location.new_custom_error(StyleParseErrorKind::UnsupportedColorSpace(space)))
            }
        }
    }
}

/// What `cssparser-color` hands back. Colour spaces this crate cannot convert
/// to sRGB are reported rather than silently mangled.
enum ParsedColor {
    Value(ColorValue),
    Unsupported(&'static str),
}

impl ParsedColor {
    fn color(color: Color) -> Self {
        Self::Value(ColorValue::Color(color))
    }
}

impl FromParsedColor for ParsedColor {
    fn from_current_color() -> Self {
        Self::Value(ColorValue::CurrentColor)
    }

    fn from_rgba(red: u8, green: u8, blue: u8, alpha: f32) -> Self {
        Self::color(Color::from_rgba8(red, green, blue, 255).with_alpha(alpha))
    }

    fn from_hsl(
        hue: Option<f32>,
        saturation: Option<f32>,
        lightness: Option<f32>,
        alpha: Option<f32>,
    ) -> Self {
        Self::color(Color::from_hsl(
            hue.unwrap_or(0.0),
            saturation.unwrap_or(0.0),
            lightness.unwrap_or(0.0),
            alpha.unwrap_or(1.0),
        ))
    }

    fn from_hwb(
        hue: Option<f32>,
        whiteness: Option<f32>,
        blackness: Option<f32>,
        alpha: Option<f32>,
    ) -> Self {
        Self::color(Color::from_hwb(
            hue.unwrap_or(0.0),
            whiteness.unwrap_or(0.0),
            blackness.unwrap_or(0.0),
            alpha.unwrap_or(1.0),
        ))
    }

    fn from_lab(
        lightness: Option<f32>,
        a: Option<f32>,
        b: Option<f32>,
        alpha: Option<f32>,
    ) -> Self {
        Self::color(Color::from_lab(
            lightness.unwrap_or(0.0),
            a.unwrap_or(0.0),
            b.unwrap_or(0.0),
            alpha.unwrap_or(1.0),
        ))
    }

    fn from_lch(
        lightness: Option<f32>,
        chroma: Option<f32>,
        hue: Option<f32>,
        alpha: Option<f32>,
    ) -> Self {
        Self::color(Color::from_lch(
            lightness.unwrap_or(0.0),
            chroma.unwrap_or(0.0),
            hue.unwrap_or(0.0),
            alpha.unwrap_or(1.0),
        ))
    }

    fn from_oklab(
        lightness: Option<f32>,
        a: Option<f32>,
        b: Option<f32>,
        alpha: Option<f32>,
    ) -> Self {
        Self::color(Color::from_oklab(
            lightness.unwrap_or(0.0),
            a.unwrap_or(0.0),
            b.unwrap_or(0.0),
            alpha.unwrap_or(1.0),
        ))
    }

    fn from_oklch(
        lightness: Option<f32>,
        chroma: Option<f32>,
        hue: Option<f32>,
        alpha: Option<f32>,
    ) -> Self {
        Self::color(Color::from_oklch(
            lightness.unwrap_or(0.0),
            chroma.unwrap_or(0.0),
            hue.unwrap_or(0.0),
            alpha.unwrap_or(1.0),
        ))
    }

    fn from_color_function(
        color_space: cssparser::color::PredefinedColorSpace,
        c1: Option<f32>,
        c2: Option<f32>,
        c3: Option<f32>,
        alpha: Option<f32>,
    ) -> Self {
        use cssparser::color::PredefinedColorSpace::*;
        let (c1, c2, c3) = (c1.unwrap_or(0.0), c2.unwrap_or(0.0), c3.unwrap_or(0.0));
        let alpha = alpha.unwrap_or(1.0);
        match color_space {
            Srgb => Self::color(Color::rgba(
                c1.clamp(0.0, 1.0),
                c2.clamp(0.0, 1.0),
                c3.clamp(0.0, 1.0),
                alpha,
            )),
            SrgbLinear => Self::color(Color::from_linear_srgb(c1, c2, c3, alpha)),
            DisplayP3 => Self::color(Color::from_display_p3(c1, c2, c3, alpha)),
            DisplayP3Linear => Self::Unsupported("display-p3-linear"),
            A98Rgb => Self::Unsupported("a98-rgb"),
            ProphotoRgb => Self::Unsupported("prophoto-rgb"),
            Rec2020 => Self::Unsupported("rec2020"),
            XyzD50 => Self::Unsupported("xyz-d50"),
            XyzD65 => Self::Unsupported("xyz-d65"),
        }
    }
}

struct ColorParserImpl;

impl<'i> ColorParser<'i> for ColorParserImpl {
    type Output = ParsedColor;
    type Error = StyleParseErrorKind<'i>;
}

/// A border width: a keyword or a length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BorderWidth {
    /// 1px
    Thin,
    /// 3px
    Medium,
    /// 5px
    Thick,
    Length(Length),
}

impl BorderWidth {
    pub(crate) fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, StyleError<'i>> {
        if let Ok(keyword) = input.try_parse(|input| -> Result<Self, StyleError<'i>> {
            keyword!(input, {
                "thin" => Self::Thin,
                "medium" => Self::Medium,
                "thick" => Self::Thick,
            })
        }) {
            return Ok(keyword);
        }
        Length::parse_non_negative(input).map(Self::Length)
    }
}

impl BorderStyle {
    pub(crate) fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, StyleError<'i>> {
        keyword!(input, {
            "none" => Self::None,
            "hidden" => Self::Hidden,
            "solid" => Self::Solid,
            "dashed" => Self::Dashed,
            "dotted" => Self::Dotted,
            "double" => Self::Double,
        })
    }
}

/// The absolute `font-size` keywords, on the usual scale where `medium` is
/// 16px.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontSizeKeyword {
    XxSmall,
    XSmall,
    Small,
    Medium,
    Large,
    XLarge,
    XxLarge,
    XxxLarge,
}

impl FontSizeKeyword {
    pub fn to_px(self) -> f32 {
        match self {
            Self::XxSmall => 9.0,
            Self::XSmall => 10.0,
            Self::Small => 13.0,
            Self::Medium => 16.0,
            Self::Large => 18.0,
            Self::XLarge => 24.0,
            Self::XxLarge => 32.0,
            Self::XxxLarge => 48.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FontSize {
    Length(Length),
    /// A fraction of the parent's font size.
    Percent(f32),
    Keyword(FontSizeKeyword),
    /// 1.2 times the parent's font size.
    Larger,
    /// The parent's font size divided by 1.2.
    Smaller,
}

impl FontSize {
    pub(crate) fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, StyleError<'i>> {
        if let Ok(keyword) = input.try_parse(|input| -> Result<Self, StyleError<'i>> {
            keyword!(input, {
                "xx-small" => Self::Keyword(FontSizeKeyword::XxSmall),
                "x-small" => Self::Keyword(FontSizeKeyword::XSmall),
                "small" => Self::Keyword(FontSizeKeyword::Small),
                "medium" => Self::Keyword(FontSizeKeyword::Medium),
                "large" => Self::Keyword(FontSizeKeyword::Large),
                "x-large" => Self::Keyword(FontSizeKeyword::XLarge),
                "xx-large" => Self::Keyword(FontSizeKeyword::XxLarge),
                "xxx-large" => Self::Keyword(FontSizeKeyword::XxxLarge),
                "larger" => Self::Larger,
                "smaller" => Self::Smaller,
            })
        }) {
            return Ok(keyword);
        }
        Ok(match LengthPercentage::parse_non_negative(input)? {
            LengthPercentage::Length(length) => Self::Length(length),
            LengthPercentage::Percent(p) => Self::Percent(p),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FontWeight {
    /// 1 to 1000. `normal` is 400 and `bold` is 700.
    Absolute(f32),
    Bolder,
    Lighter,
}

impl FontWeight {
    pub(crate) fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, StyleError<'i>> {
        if let Ok(keyword) = input.try_parse(|input| -> Result<Self, StyleError<'i>> {
            keyword!(input, {
                "normal" => Self::Absolute(400.0),
                "bold" => Self::Absolute(700.0),
                "bolder" => Self::Bolder,
                "lighter" => Self::Lighter,
            })
        }) {
            return Ok(keyword);
        }
        input.skip_whitespace();
        let location = input.current_source_location();
        let weight = input.expect_number()?;
        if !(1.0..=1000.0).contains(&weight) {
            return Err(location.new_custom_error(StyleParseErrorKind::OutOfRange));
        }
        Ok(Self::Absolute(weight))
    }
}

impl FontStyle {
    pub(crate) fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, StyleError<'i>> {
        keyword!(input, {
            "normal" => Self::Normal,
            "italic" => Self::Italic,
            "oblique" => Self::Oblique,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineHeight {
    Normal,
    /// A multiple of the font size, inherited as the multiple.
    Number(f32),
    Length(Length),
    /// A fraction of the font size, resolved to a length.
    Percent(f32),
}

impl LineHeight {
    pub(crate) fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, StyleError<'i>> {
        if input
            .try_parse(|input| input.expect_ident_matching("normal"))
            .is_ok()
        {
            return Ok(Self::Normal);
        }
        input.skip_whitespace();
        let location = input.current_source_location();
        if let Ok(number) = input.try_parse(|input| input.expect_number()) {
            if number < 0.0 {
                return Err(location.new_custom_error(StyleParseErrorKind::NegativeValue));
            }
            return Ok(Self::Number(number));
        }
        Ok(match LengthPercentage::parse_non_negative(input)? {
            LengthPercentage::Length(length) => Self::Length(length),
            LengthPercentage::Percent(p) => Self::Percent(p),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LetterSpacing {
    Normal,
    Length(Length),
}

impl LetterSpacing {
    pub(crate) fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, StyleError<'i>> {
        if input
            .try_parse(|input| input.expect_ident_matching("normal"))
            .is_ok()
        {
            return Ok(Self::Normal);
        }
        Length::parse(input).map(Self::Length)
    }
}

impl TextAlign {
    pub(crate) fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, StyleError<'i>> {
        keyword!(input, {
            "start" => Self::Start,
            "end" => Self::End,
            "left" => Self::Left,
            "right" => Self::Right,
            "center" => Self::Center,
            "justify" => Self::Justify,
        })
    }
}

impl FontFamily {
    pub(crate) fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, StyleError<'i>> {
        let names = input.parse_comma_separated(|input| {
            if let Ok(name) = input.try_parse(|input| input.expect_string_cloned()) {
                return Ok(name.to_string());
            }
            // An unquoted family is one or more identifiers: `Segoe UI`.
            let mut name = input.expect_ident()?.to_string();
            while let Ok(word) = input.try_parse(|input| input.expect_ident_cloned()) {
                name.push(' ');
                name.push_str(&word);
            }
            if is_generic_family(&name) {
                name.make_ascii_lowercase();
            }
            Ok(name)
        })?;
        Ok(Self::new(names))
    }
}

fn is_generic_family(name: &str) -> bool {
    cssparser::match_ignore_ascii_case! { name,
        "serif" | "sans-serif" | "monospace" | "cursive" | "fantasy" | "system-ui"
        | "ui-serif" | "ui-sans-serif" | "ui-monospace" | "ui-rounded" | "emoji"
        | "math" | "fangsong" => true,
        _ => false,
    }
}

/// `opacity`: a number or percentage, clamped to `0.0..=1.0`.
pub(crate) fn parse_opacity<'i>(input: &mut Parser<'i, '_>) -> Result<f32, StyleError<'i>> {
    input.skip_whitespace();
    let location = input.current_source_location();
    let value = match input.next()?.clone() {
        Token::Number { value, .. } => value,
        Token::Percentage { unit_value, .. } => unit_value,
        token => return Err(location.new_unexpected_token_error(token)),
    };
    Ok(value.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cssparser::ParserInput;

    fn parse<T>(
        css: &str,
        parse: impl for<'i, 't> FnOnce(&mut Parser<'i, 't>) -> Result<T, StyleError<'i>>,
    ) -> Result<T, String> {
        let mut input = ParserInput::new(css);
        let mut parser = Parser::new(&mut input);
        parser
            .parse_entirely(parse)
            .map_err(|e| crate::Error::from_parse_error(e).to_string())
    }

    #[test]
    fn lengths() {
        assert_eq!(parse("12px", Length::parse), Ok(Length::Px(12.0)));
        assert_eq!(parse("1.5em", Length::parse), Ok(Length::Em(1.5)));
        assert_eq!(parse("2rem", Length::parse), Ok(Length::Rem(2.0)));
        assert_eq!(parse("0", Length::parse), Ok(Length::Px(0.0)));
        assert_eq!(parse("72pt", Length::parse), Ok(Length::Px(96.0)));
        assert_eq!(parse("1in", Length::parse), Ok(Length::Px(96.0)));
        assert!(parse("12", Length::parse).is_err());
        assert!(parse("12vw", Length::parse).is_err());
        assert!(parse("-1px", Length::parse_non_negative).is_err());
        assert_eq!(parse("-1px", Length::parse), Ok(Length::Px(-1.0)));
        assert_eq!(
            parse("50%", LengthPercentage::parse),
            Ok(LengthPercentage::Percent(0.5))
        );
        assert_eq!(parse("auto", Size::parse), Ok(Size::Auto));
        assert_eq!(parse("none", MaxSize::parse), Ok(MaxSize::None));
    }

    #[test]
    fn colors() {
        let rgba8 = |css: &str| match parse(css, ColorValue::parse) {
            Ok(ColorValue::Color(color)) => Ok(color.to_rgba8()),
            Ok(ColorValue::CurrentColor) => Err("currentcolor".to_owned()),
            Err(e) => Err(e),
        };
        assert_eq!(rgba8("#fff"), Ok([255, 255, 255, 255]));
        assert_eq!(rgba8("#1a2b3c80"), Ok([0x1a, 0x2b, 0x3c, 0x80]));
        assert_eq!(rgba8("rebeccapurple"), Ok([102, 51, 153, 255]));
        assert_eq!(rgba8("Transparent"), Ok([0, 0, 0, 0]));
        assert_eq!(rgba8("rgb(10 20 30 / 50%)"), Ok([10, 20, 30, 128]));
        assert_eq!(rgba8("rgba(10, 20, 30, 0.5)"), Ok([10, 20, 30, 128]));
        assert_eq!(rgba8("hsl(120 100% 50%)"), Ok([0, 255, 0, 255]));
        assert_eq!(rgba8("hwb(0 0% 0%)"), Ok([255, 0, 0, 255]));
        assert_eq!(rgba8("oklab(100% 0 0)"), Ok([255, 255, 255, 255]));
        assert_eq!(rgba8("oklch(62.8% 0.2577 29.23)"), Ok([255, 0, 0, 255]));
        assert_eq!(rgba8("lab(100% 0 0)"), Ok([255, 255, 255, 255]));
        assert_eq!(rgba8("color(srgb 1 0 0)"), Ok([255, 0, 0, 255]));
        assert_eq!(
            rgba8("color(srgb-linear 1 0 0 / 0.5)"),
            Ok([255, 0, 0, 128])
        );
        assert_eq!(rgba8("color(display-p3 1 1 1)"), Ok([255, 255, 255, 255]));
        assert_eq!(
            parse("currentColor", ColorValue::parse),
            Ok(ColorValue::CurrentColor)
        );
        assert_eq!(
            rgba8("color(rec2020 1 0 0)"),
            Err("line 1, column 1: unsupported colour space 'rec2020'".to_owned())
        );
        assert!(rgba8("#12345").is_err());
        assert!(rgba8("notacolour").is_err());
    }

    #[test]
    fn typography() {
        assert_eq!(
            parse("bold", FontWeight::parse),
            Ok(FontWeight::Absolute(700.0))
        );
        assert_eq!(
            parse("550", FontWeight::parse),
            Ok(FontWeight::Absolute(550.0))
        );
        assert!(parse("0", FontWeight::parse).is_err());
        assert!(parse("1001", FontWeight::parse).is_err());
        assert_eq!(
            parse("x-large", FontSize::parse),
            Ok(FontSize::Keyword(FontSizeKeyword::XLarge))
        );
        assert_eq!(parse("120%", FontSize::parse), Ok(FontSize::Percent(1.2)));
        assert_eq!(parse("1.4", LineHeight::parse), Ok(LineHeight::Number(1.4)));
        assert_eq!(parse("normal", LineHeight::parse), Ok(LineHeight::Normal));
        assert!(parse("-1", LineHeight::parse).is_err());
        assert_eq!(
            parse("\"Segoe UI\", Inter Display, sans-serif", FontFamily::parse),
            Ok(FontFamily::new(["Segoe UI", "Inter Display", "sans-serif"]))
        );
        assert_eq!(
            parse("Sans-Serif", FontFamily::parse),
            Ok(FontFamily::new(["sans-serif"]))
        );
        assert!(parse("", FontFamily::parse).is_err());
        assert_eq!(parse("center", TextAlign::parse), Ok(TextAlign::Center));
    }

    #[test]
    fn opacity_is_clamped() {
        assert_eq!(parse("0.5", parse_opacity), Ok(0.5));
        assert_eq!(parse("50%", parse_opacity), Ok(0.5));
        assert_eq!(parse("7", parse_opacity), Ok(1.0));
        assert_eq!(parse("-1", parse_opacity), Ok(0.0));
        assert!(parse("1px", parse_opacity).is_err());
    }
}
