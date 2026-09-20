// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Brush colour.

use crate::ffi;

/// The colour space a colour's components are expressed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ColorSpace {
    #[default]
    Srgb,
    DisplayP3,
}

impl ColorSpace {
    pub(crate) fn to_ffi(self) -> ffi::ColorSpace {
        match self {
            Self::Srgb => ffi::ColorSpace::kSrgb,
            Self::DisplayP3 => ffi::ColorSpace::kDisplayP3,
        }
    }

    pub(crate) fn from_ffi(space: ffi::ColorSpace) -> Self {
        if space == ffi::ColorSpace::kDisplayP3 {
            Self::DisplayP3
        } else {
            Self::Srgb
        }
    }
}

/// How a colour's components are encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ColorFormat {
    /// Components are linear light.
    Linear,
    /// Components carry the colour space's transfer function. This is what
    /// hex colours and most colour pickers give you.
    #[default]
    GammaEncoded,
    /// Components are linear and already multiplied by alpha.
    PremultipliedAlpha,
}

impl ColorFormat {
    pub(crate) fn to_ffi(self) -> ffi::ColorFormat {
        match self {
            Self::Linear => ffi::ColorFormat::kLinear,
            Self::GammaEncoded => ffi::ColorFormat::kGammaEncoded,
            Self::PremultipliedAlpha => ffi::ColorFormat::kPremultipliedAlpha,
        }
    }
}

/// A colour, together with how to interpret its components.
///
/// Ink stores colour internally as linear, non-premultiplied sRGB, and converts
/// on the way in and out. The `space` and `format` here say what the `r`, `g`,
/// `b` and `a` fields mean, not how Ink will store them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
    pub space: ColorSpace,
    pub format: ColorFormat,
}

impl Color {
    pub const BLACK: Color = Color::srgb(0.0, 0.0, 0.0, 1.0);
    pub const WHITE: Color = Color::srgb(1.0, 1.0, 1.0, 1.0);
    pub const TRANSPARENT: Color = Color::srgb(0.0, 0.0, 0.0, 0.0);

    /// A gamma-encoded sRGB colour, the usual way to spell a colour by hand.
    pub const fn srgb(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self {
            r,
            g,
            b,
            a,
            space: ColorSpace::Srgb,
            format: ColorFormat::GammaEncoded,
        }
    }

    /// A gamma-encoded sRGB colour from 8-bit components.
    pub fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::srgb(
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
            f32::from(a) / 255.0,
        )
    }

    /// A gamma-encoded sRGB colour from a packed `0xRRGGBBAA` value.
    pub fn from_packed_rgba(rgba: u32) -> Self {
        Self::from_rgba8(
            (rgba >> 24) as u8,
            (rgba >> 16) as u8,
            (rgba >> 8) as u8,
            rgba as u8,
        )
    }

    /// The same colour with a different alpha.
    pub fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    pub(crate) fn to_ffi(self) -> ffi::Rgba {
        ffi::Rgba {
            r: self.r,
            g: self.g,
            b: self.b,
            a: self.a,
        }
    }

    pub(crate) fn from_ffi(rgba: ffi::Rgba, space: ColorSpace, format: ColorFormat) -> Self {
        Self {
            r: rgba.r,
            g: rgba.g,
            b: rgba.b,
            a: rgba.a,
            space,
            format,
        }
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::BLACK
    }
}
