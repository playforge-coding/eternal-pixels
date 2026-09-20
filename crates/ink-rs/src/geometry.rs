// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Plain geometric values.
//!
//! These are ordinary Rust types rather than the generated ones. The generated
//! structs are `!Send`/`!Sync` because Crubit cannot know what the C++ side
//! does with them, which would spread to everything holding a point. Converting
//! at the boundary costs a couple of float copies and keeps the public API
//! movable between threads.

use crate::ffi;

/// A position in stroke units.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const ORIGIN: Point = Point { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub(crate) fn from_ffi(point: ffi::Point) -> Self {
        Self {
            x: point.x,
            y: point.y,
        }
    }

    pub(crate) fn to_ffi(self) -> ffi::Point {
        ffi::Point {
            x: self.x,
            y: self.y,
        }
    }
}

impl From<(f32, f32)> for Point {
    fn from((x, y): (f32, f32)) -> Self {
        Self::new(x, y)
    }
}

/// A displacement in stroke units.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn magnitude(self) -> f32 {
        self.x.hypot(self.y)
    }
}

impl From<(f32, f32)> for Vec2 {
    fn from((x, y): (f32, f32)) -> Self {
        Self::new(x, y)
    }
}

/// An axis-aligned bounding box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x_min: f32,
    pub y_min: f32,
    pub x_max: f32,
    pub y_max: f32,
}

impl Rect {
    pub const fn new(x_min: f32, y_min: f32, x_max: f32, y_max: f32) -> Self {
        Self {
            x_min,
            y_min,
            x_max,
            y_max,
        }
    }

    pub fn width(&self) -> f32 {
        self.x_max - self.x_min
    }

    pub fn height(&self) -> f32 {
        self.y_max - self.y_min
    }

    pub fn center(&self) -> Point {
        Point::new(
            (self.x_min + self.x_max) * 0.5,
            (self.y_min + self.y_max) * 0.5,
        )
    }

    pub(crate) fn from_ffi(rect: ffi::Rect) -> Self {
        Self {
            x_min: rect.x_min,
            y_min: rect.y_min,
            x_max: rect.x_max,
            y_max: rect.y_max,
        }
    }
}

/// A snapshot of triangle geometry, ready to hand to a renderer.
///
/// Positions and indices are copied out of Ink rather than borrowed, because
/// Ink rebuilds its meshes in place as a stroke grows and any borrow would be
/// invalidated by the next call to
/// [`InProgressStroke::update_shape`](crate::InProgressStroke::update_shape).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mesh {
    /// Vertex positions, one per vertex.
    pub positions: Vec<Point>,
    /// Triangle vertex indices, three per triangle, indexing into `positions`.
    pub indices: Vec<u32>,
}

impl Mesh {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// Number of triangles, which is always `indices.len() / 3`.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// The three positions making up triangle `index`, if it exists.
    pub fn triangle(&self, index: usize) -> Option<[Point; 3]> {
        let base = index.checked_mul(3)?;
        let slice = self.indices.get(base..base + 3)?;
        Some([
            *self.positions.get(slice[0] as usize)?,
            *self.positions.get(slice[1] as usize)?,
            *self.positions.get(slice[2] as usize)?,
        ])
    }
}
