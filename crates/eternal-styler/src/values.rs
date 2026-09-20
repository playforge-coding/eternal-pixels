// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Value types shared between specified and computed styles.

use std::sync::Arc;

/// A colour as gamma-encoded sRGB with straight (not premultiplied) alpha,
/// each component in `0.0..=1.0`.
///
/// Every colour syntax the parser accepts ends up here, so a renderer only
/// ever deals with one representation.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const BLACK: Color = Color::rgb(0.0, 0.0, 0.0);
    pub const WHITE: Color = Color::rgb(1.0, 1.0, 1.0);
    pub const TRANSPARENT: Color = Color::rgba(0.0, 0.0, 0.0, 0.0);

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self::rgba(r, g, b, 1.0)
    }

    /// From 8-bit components, the way hex colours are written.
    pub fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::rgba(
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
            f32::from(a) / 255.0,
        )
    }

    /// From a packed `0xRRGGBBAA` value.
    pub fn from_packed_rgba(rgba: u32) -> Self {
        Self::from_rgba8(
            (rgba >> 24) as u8,
            (rgba >> 16) as u8,
            (rgba >> 8) as u8,
            rgba as u8,
        )
    }

    /// The components as bytes, `[r, g, b, a]`, rounded to nearest.
    pub fn to_rgba8(self) -> [u8; 4] {
        let byte = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
        [byte(self.r), byte(self.g), byte(self.b), byte(self.a)]
    }

    pub fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    /// From linear-light sRGB components, as `color(srgb-linear ...)` gives.
    pub fn from_linear_srgb(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self::rgba(gamma_encode(r), gamma_encode(g), gamma_encode(b), a).clamped()
    }

    /// From HSL. Hue in degrees; saturation and lightness in `0.0..=1.0`.
    pub fn from_hsl(hue: f32, saturation: f32, lightness: f32, a: f32) -> Self {
        let (r, g, b) = cssparser_color::hsl_to_rgb(
            normalize_hue(hue) / 360.0,
            saturation.clamp(0.0, 1.0),
            lightness.clamp(0.0, 1.0),
        );
        Self::rgba(r, g, b, a)
    }

    /// From HWB. Hue in degrees; whiteness and blackness in `0.0..=1.0`.
    pub fn from_hwb(hue: f32, whiteness: f32, blackness: f32, a: f32) -> Self {
        let (r, g, b) = cssparser_color::hwb_to_rgb(
            normalize_hue(hue) / 360.0,
            whiteness.clamp(0.0, 1.0),
            blackness.clamp(0.0, 1.0),
        );
        Self::rgba(r, g, b, a)
    }

    /// From Oklab, with lightness in `0.0..=1.0`. Colours outside the sRGB
    /// gamut are clipped component-wise.
    pub fn from_oklab(lightness: f32, a: f32, b: f32, alpha: f32) -> Self {
        let l_ = lightness + 0.396_337_78 * a + 0.215_803_76 * b;
        let m_ = lightness - 0.105_561_346 * a - 0.063_854_17 * b;
        let s_ = lightness - 0.089_484_18 * a - 1.291_485_5 * b;
        let (l, m, s) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
        Self::from_linear_srgb(
            4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s,
            -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s,
            -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s,
            alpha,
        )
    }

    /// From Oklch, with lightness in `0.0..=1.0` and hue in degrees.
    pub fn from_oklch(lightness: f32, chroma: f32, hue: f32, alpha: f32) -> Self {
        let (a, b) = polar_to_rect(chroma, hue);
        Self::from_oklab(lightness, a, b, alpha)
    }

    /// From CIELAB (D50), with lightness in `0.0..=100.0`. Colours outside
    /// the sRGB gamut are clipped component-wise.
    pub fn from_lab(lightness: f32, a: f32, b: f32, alpha: f32) -> Self {
        // Lab to XYZ, relative to the D50 white point.
        const EPSILON: f32 = 216.0 / 24389.0;
        const KAPPA: f32 = 24389.0 / 27.0;
        const WHITE: [f32; 3] = [0.964_22, 1.0, 0.825_21];
        let fy = (lightness + 16.0) / 116.0;
        let fx = a / 500.0 + fy;
        let fz = fy - b / 200.0;
        let inverse = |f: f32| {
            let cubed = f * f * f;
            if cubed > EPSILON {
                cubed
            } else {
                (116.0 * f - 16.0) / KAPPA
            }
        };
        let x = inverse(fx) * WHITE[0];
        let y = if lightness > KAPPA * EPSILON {
            fy * fy * fy
        } else {
            lightness / KAPPA
        } * WHITE[1];
        let z = inverse(fz) * WHITE[2];

        // Bradford chromatic adaptation from D50 to D65.
        let x65 = 0.955_473_4 * x - 0.023_098_538 * y + 0.063_259_31 * z;
        let y65 = -0.028_369_706 * x + 1.009_995_5 * y + 0.021_041_399 * z;
        let z65 = 0.012_314_002 * x - 0.020_507_697 * y + 1.330_365_9 * z;

        Self::from_xyz_d65(x65, y65, z65, alpha)
    }

    /// From CIELCH (D50), with lightness in `0.0..=100.0` and hue in degrees.
    pub fn from_lch(lightness: f32, chroma: f32, hue: f32, alpha: f32) -> Self {
        let (a, b) = polar_to_rect(chroma, hue);
        Self::from_lab(lightness, a, b, alpha)
    }

    /// From linear Display P3 components, as `color(display-p3 ...)` gives.
    /// Colours outside the sRGB gamut are clipped component-wise.
    pub fn from_display_p3(r: f32, g: f32, b: f32, alpha: f32) -> Self {
        let (r, g, b) = (gamma_decode(r), gamma_decode(g), gamma_decode(b));
        let x = 0.486_570_95 * r + 0.265_667_7 * g + 0.198_217_29 * b;
        let y = 0.228_974_57 * r + 0.691_738_55 * g + 0.079_286_91 * b;
        let z = 0.045_113_38 * g + 1.043_944_4 * b;
        Self::from_xyz_d65(x, y, z, alpha)
    }

    fn from_xyz_d65(x: f32, y: f32, z: f32, alpha: f32) -> Self {
        Self::from_linear_srgb(
            3.240_97 * x - 1.537_383_2 * y - 0.498_610_76 * z,
            -0.969_243_65 * x + 1.875_967_5 * y + 0.041_555_06 * z,
            0.055_630_08 * x - 0.203_976_96 * y + 1.056_971_5 * z,
            alpha,
        )
    }

    fn clamped(self) -> Self {
        Self::rgba(
            self.r.clamp(0.0, 1.0),
            self.g.clamp(0.0, 1.0),
            self.b.clamp(0.0, 1.0),
            self.a.clamp(0.0, 1.0),
        )
    }
}

fn gamma_encode(linear: f32) -> f32 {
    if linear <= 0.003_130_8 {
        12.92 * linear
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    }
}

fn gamma_decode(encoded: f32) -> f32 {
    if encoded <= 0.040_45 {
        encoded / 12.92
    } else {
        ((encoded + 0.055) / 1.055).powf(2.4)
    }
}

fn normalize_hue(hue: f32) -> f32 {
    hue - 360.0 * (hue / 360.0).floor()
}

fn polar_to_rect(chroma: f32, hue_degrees: f32) -> (f32, f32) {
    let radians = hue_degrees.to_radians();
    (chroma * radians.cos(), chroma * radians.sin())
}

/// How a border is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum BorderStyle {
    /// No border. The computed border width is zero.
    #[default]
    None,
    /// Like `None` for drawing and layout, kept distinct because CSS does.
    Hidden,
    Solid,
    Dashed,
    Dotted,
    Double,
}

impl BorderStyle {
    /// Whether the border takes up space and is drawn.
    pub fn is_visible(self) -> bool {
        !matches!(self, Self::None | Self::Hidden)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
    Oblique,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum TextAlign {
    /// The start of the line in the writing direction.
    #[default]
    Start,
    End,
    Left,
    Right,
    Center,
    Justify,
}

/// A font weight from 1 to 1000, where 400 is normal and 700 is bold.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct FontWeight(f32);

impl FontWeight {
    pub const NORMAL: FontWeight = FontWeight(400.0);
    pub const BOLD: FontWeight = FontWeight(700.0);

    /// Clamped to `1.0..=1000.0`.
    pub fn new(weight: f32) -> Self {
        Self(weight.clamp(1.0, 1000.0))
    }

    pub fn value(self) -> f32 {
        self.0
    }

    /// `bolder` relative to this weight, per the CSS Fonts table.
    pub fn bolder(self) -> Self {
        Self(if self.0 < 350.0 {
            400.0
        } else if self.0 < 550.0 {
            700.0
        } else if self.0 < 900.0 {
            900.0
        } else {
            self.0
        })
    }

    /// `lighter` relative to this weight, per the CSS Fonts table.
    pub fn lighter(self) -> Self {
        Self(if self.0 < 100.0 {
            self.0
        } else if self.0 < 550.0 {
            100.0
        } else if self.0 < 750.0 {
            400.0
        } else {
            700.0
        })
    }
}

impl Default for FontWeight {
    fn default() -> Self {
        Self::NORMAL
    }
}

/// A prioritised list of font family names.
///
/// Generic families (`sans-serif`, `serif`, `monospace`, `system-ui`, ...)
/// are passed through as plain lower-case names; it is up to the toolkit to
/// map them to real fonts.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FontFamily(Arc<[String]>);

impl FontFamily {
    pub fn new<I, S>(names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self(names.into_iter().map(Into::into).collect())
    }

    /// The names in order of preference.
    pub fn names(&self) -> &[String] {
        &self.0
    }
}

impl Default for FontFamily {
    fn default() -> Self {
        Self::new(["sans-serif"])
    }
}

/// One value per side of a box, in the CSS order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Sides<T> {
    pub top: T,
    pub right: T,
    pub bottom: T,
    pub left: T,
}

impl<T> Sides<T> {
    pub const fn new(top: T, right: T, bottom: T, left: T) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    pub fn all(value: T) -> Self
    where
        T: Clone,
    {
        Self::new(value.clone(), value.clone(), value.clone(), value)
    }
}

/// One value per corner of a box, in the CSS order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Corners<T> {
    pub top_left: T,
    pub top_right: T,
    pub bottom_right: T,
    pub bottom_left: T,
}

impl<T> Corners<T> {
    pub const fn new(top_left: T, top_right: T, bottom_right: T, bottom_left: T) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    pub fn all(value: T) -> Self
    where
        T: Clone,
    {
        Self::new(value.clone(), value.clone(), value.clone(), value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(color: Color, expected: [u8; 4]) {
        let got = color.to_rgba8();
        for (g, e) in got.iter().zip(expected) {
            assert!(
                (i16::from(*g) - i16::from(e)).abs() <= 1,
                "{got:?} vs {expected:?}"
            );
        }
    }

    #[test]
    fn packed_and_bytes_round_trip() {
        let color = Color::from_packed_rgba(0x1a2b3c80);
        assert_eq!(color.to_rgba8(), [0x1a, 0x2b, 0x3c, 0x80]);
    }

    #[test]
    fn hsl_and_hwb() {
        close(Color::from_hsl(120.0, 1.0, 0.5, 1.0), [0, 255, 0, 255]);
        close(Color::from_hsl(-240.0, 1.0, 0.5, 1.0), [0, 255, 0, 255]);
        close(Color::from_hwb(0.0, 0.0, 0.0, 1.0), [255, 0, 0, 255]);
        close(Color::from_hwb(0.0, 0.5, 0.5, 1.0), [128, 128, 128, 255]);
    }

    #[test]
    fn oklab_and_oklch() {
        close(Color::from_oklab(1.0, 0.0, 0.0, 1.0), [255, 255, 255, 255]);
        close(Color::from_oklab(0.0, 0.0, 0.0, 1.0), [0, 0, 0, 255]);
        close(
            Color::from_oklab(0.627_955, 0.224_863, 0.125_846, 1.0),
            [255, 0, 0, 255],
        );
        close(
            Color::from_oklch(0.627_955, 0.257_683, 29.233_9, 1.0),
            [255, 0, 0, 255],
        );
    }

    #[test]
    fn lab_and_lch() {
        close(Color::from_lab(100.0, 0.0, 0.0, 1.0), [255, 255, 255, 255]);
        close(Color::from_lab(0.0, 0.0, 0.0, 1.0), [0, 0, 0, 255]);
        close(Color::from_lab(50.0, 0.0, 0.0, 1.0), [119, 119, 119, 255]);
        close(Color::from_lab(54.29, 80.80, 69.89, 1.0), [255, 0, 0, 255]);
        close(Color::from_lch(54.29, 106.84, 40.86, 1.0), [255, 0, 0, 255]);
    }

    #[test]
    fn display_p3_and_linear() {
        close(
            Color::from_display_p3(1.0, 1.0, 1.0, 1.0),
            [255, 255, 255, 255],
        );
        close(Color::from_display_p3(0.0, 0.0, 0.0, 1.0), [0, 0, 0, 255]);
        close(
            Color::from_linear_srgb(0.0, 0.214_04, 1.0, 0.5),
            [0, 128, 255, 128],
        );
    }

    #[test]
    fn font_weight_relative_keywords() {
        assert_eq!(FontWeight::NORMAL.bolder(), FontWeight::BOLD);
        assert_eq!(FontWeight::BOLD.bolder(), FontWeight::new(900.0));
        assert_eq!(FontWeight::new(900.0).bolder(), FontWeight::new(900.0));
        assert_eq!(FontWeight::BOLD.lighter(), FontWeight::NORMAL);
        assert_eq!(FontWeight::NORMAL.lighter(), FontWeight::new(100.0));
        assert_eq!(FontWeight::new(50.0).lighter(), FontWeight::new(50.0));
    }
}
