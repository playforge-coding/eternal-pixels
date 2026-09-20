// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Brushes: what a stroke's path is drawn with.

use crate::color::{Color, ColorFormat, ColorSpace};
use crate::error::{Result, check};
use crate::ffi;

/// Panics if a C++ constructor handed back nothing.
///
/// The facade only returns a null handle when `new` failed, which in practice
/// means the process is already out of memory.
fn expect_handle<T>(handle: T, is_null: bool, what: &str) -> T {
    assert!(!is_null, "ink: failed to allocate {what}");
    handle
}

/// One of Ink's built-in brush families.
///
/// Stock brushes are versioned so that strokes stored as input points plus a
/// brush spec still look the way they did when drawn, even after Ink is
/// upgraded. [`StockBrush::family`] takes the latest version;
/// [`StockBrush::family_version`] pins one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StockBrush {
    /// A simple circular fixed-width brush.
    Marker,
    /// Pressure- and speed-sensitive, tuned for handwriting with a stylus.
    PressurePen,
    /// A chisel tip, meant to be used with a translucent colour.
    Highlighter,
    /// Rounded rectangles with gaps between them, for decoration or for
    /// showing a lasso selection.
    DashedLine,
}

impl StockBrush {
    /// The latest version of this stock brush.
    pub fn family(self) -> BrushFamily {
        self.family_version(0)
    }

    /// A specific version of this stock brush. Version `0` means the latest.
    pub fn family_version(self, version: u32) -> BrushFamily {
        let version = i32::try_from(version).unwrap_or(0);
        let handle = unsafe {
            match self {
                Self::Marker => ffi::StockBrushMarker(version),
                Self::PressurePen => ffi::StockBrushPressurePen(version),
                Self::Highlighter => ffi::StockBrushHighlighter(version),
                Self::DashedLine => ffi::StockBrushDashedLine(version),
            }
        };
        BrushFamily {
            handle: expect_handle(handle, handle.handle.is_null(), "brush family"),
        }
    }
}

/// The shape and behaviour half of a brush, without colour or size.
pub struct BrushFamily {
    handle: ffi::BrushFamilyHandle,
}

impl BrushFamily {
    pub(crate) fn as_ffi(&self) -> ffi::BrushFamilyHandle {
        self.handle
    }

    pub(crate) fn from_ffi(handle: ffi::BrushFamilyHandle) -> Self {
        Self {
            handle: expect_handle(handle, handle.handle.is_null(), "brush family"),
        }
    }
}

impl Clone for BrushFamily {
    fn clone(&self) -> Self {
        Self::from_ffi(unsafe { ffi::BrushFamilyClone(self.handle) })
    }
}

impl Drop for BrushFamily {
    fn drop(&mut self) {
        unsafe { ffi::BrushFamilyDestroy(self.handle) };
    }
}

impl core::fmt::Debug for BrushFamily {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("BrushFamily").finish_non_exhaustive()
    }
}

/// A brush family together with a colour, a size and an epsilon.
///
/// `size` is the stroke width in stroke units. `epsilon` is the smallest
/// distance Ink bothers to distinguish, also in stroke units; it controls how
/// aggressively the generated mesh is simplified, and must be positive and no
/// larger than `size`.
pub struct Brush {
    handle: ffi::BrushHandle,
}

impl Brush {
    /// Builds a brush.
    ///
    /// Fails with [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument)
    /// unless `size` and `epsilon` are both finite and positive and
    /// `epsilon <= size`.
    pub fn new(family: &BrushFamily, color: Color, size: f32, epsilon: f32) -> Result<Self> {
        let mut out = ffi::BrushHandle {
            handle: core::ptr::null_mut(),
        };
        let status = unsafe {
            ffi::BrushCreate(
                family.as_ffi(),
                color.to_ffi(),
                color.space.to_ffi(),
                color.format.to_ffi(),
                size,
                epsilon,
                &mut out,
            )
        };
        check(status)?;
        Ok(Self {
            handle: expect_handle(out, out.handle.is_null(), "brush"),
        })
    }

    pub(crate) fn as_ffi(&self) -> ffi::BrushHandle {
        self.handle
    }

    pub(crate) fn from_ffi(handle: ffi::BrushHandle) -> Self {
        Self {
            handle: expect_handle(handle, handle.handle.is_null(), "brush"),
        }
    }

    /// Stroke width in stroke units.
    pub fn size(&self) -> f32 {
        unsafe { ffi::BrushSize(self.handle) }
    }

    /// Smallest distance Ink distinguishes, in stroke units.
    pub fn epsilon(&self) -> f32 {
        unsafe { ffi::BrushEpsilon(self.handle) }
    }

    /// The brush colour, gamma-encoded in the colour space Ink stored it in.
    pub fn color(&self) -> Color {
        self.color_in(ColorFormat::GammaEncoded)
    }

    /// The brush colour in a chosen encoding.
    pub fn color_in(&self, format: ColorFormat) -> Color {
        let rgba = unsafe { ffi::BrushColor(self.handle, format.to_ffi()) };
        let space = ColorSpace::from_ffi(unsafe { ffi::BrushColorSpace(self.handle) });
        Color::from_ffi(rgba, space, format)
    }

    /// How many coats this brush paints. Each coat becomes its own render
    /// group in the resulting geometry.
    pub fn coat_count(&self) -> u32 {
        unsafe { ffi::BrushCoatCount(self.handle) }
    }

    /// A copy of the brush's family.
    pub fn family(&self) -> BrushFamily {
        BrushFamily::from_ffi(unsafe { ffi::BrushGetFamily(self.handle) })
    }

    /// Changes the stroke width. Same constraints as [`Brush::new`].
    pub fn set_size(&mut self, size: f32) -> Result<()> {
        check(unsafe { ffi::BrushSetSize(self.handle, size) })
    }

    /// Changes epsilon. Same constraints as [`Brush::new`].
    pub fn set_epsilon(&mut self, epsilon: f32) -> Result<()> {
        check(unsafe { ffi::BrushSetEpsilon(self.handle, epsilon) })
    }

    /// Changes the colour. Never invalidates geometry.
    pub fn set_color(&mut self, color: Color) {
        unsafe {
            ffi::BrushSetColor(
                self.handle,
                color.to_ffi(),
                color.space.to_ffi(),
                color.format.to_ffi(),
            );
        }
    }

    /// Changes the brush family.
    pub fn set_family(&mut self, family: &BrushFamily) {
        unsafe { ffi::BrushSetFamily(self.handle, family.as_ffi()) };
    }
}

impl Clone for Brush {
    fn clone(&self) -> Self {
        Self::from_ffi(unsafe { ffi::BrushClone(self.handle) })
    }
}

impl Drop for Brush {
    fn drop(&mut self) {
        unsafe { ffi::BrushDestroy(self.handle) };
    }
}

impl core::fmt::Debug for Brush {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Brush")
            .field("size", &self.size())
            .field("epsilon", &self.epsilon())
            .field("color", &self.color())
            .field("coat_count", &self.coat_count())
            .finish()
    }
}
