// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Points, sizes and rectangles, in pixels.

use eternal_styler::Sides;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const ZERO: Point = Point { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const ZERO: Size = Size {
        width: 0.0,
        height: 0.0,
    };

    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    /// The extent along `axis`.
    pub fn main(self, axis: Axis) -> f32 {
        match axis {
            Axis::Horizontal => self.width,
            Axis::Vertical => self.height,
        }
    }

    /// The extent across `axis`.
    pub fn cross(self, axis: Axis) -> f32 {
        match axis {
            Axis::Horizontal => self.height,
            Axis::Vertical => self.width,
        }
    }

    /// A size from its extents along and across `axis`.
    pub fn from_axis(axis: Axis, main: f32, cross: f32) -> Self {
        match axis {
            Axis::Horizontal => Self::new(main, cross),
            Axis::Vertical => Self::new(cross, main),
        }
    }

    /// Both extents clamped to be at least zero.
    pub fn non_negative(self) -> Self {
        Self::new(self.width.max(0.0), self.height.max(0.0))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const ZERO: Rect = Rect {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };

    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn from_origin_size(origin: Point, size: Size) -> Self {
        Self::new(origin.x, origin.y, size.width, size.height)
    }

    pub fn origin(self) -> Point {
        Point::new(self.x, self.y)
    }

    pub fn size(self) -> Size {
        Size::new(self.width, self.height)
    }

    pub fn right(self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(self) -> f32 {
        self.y + self.height
    }

    pub fn center(self) -> Point {
        Point::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    pub fn contains(self, point: Point) -> bool {
        point.x >= self.x && point.x < self.right() && point.y >= self.y && point.y < self.bottom()
    }

    /// The rectangle shrunk by `edges` on each side, never below zero size.
    pub fn inset(self, edges: Sides<f32>) -> Self {
        Self::new(
            self.x + edges.left,
            self.y + edges.top,
            (self.width - edges.left - edges.right).max(0.0),
            (self.height - edges.top - edges.bottom).max(0.0),
        )
    }

    pub fn intersection(self, other: Rect) -> Rect {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        Rect::new(x, y, (right - x).max(0.0), (bottom - y).max(0.0))
    }
}

/// The direction a container lays its children out in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Axis {
    Horizontal,
    Vertical,
}

impl Axis {
    pub fn cross(self) -> Axis {
        match self {
            Self::Horizontal => Self::Vertical,
            Self::Vertical => Self::Horizontal,
        }
    }
}

/// The total of two opposite sides, along an axis.
pub(crate) fn sides_along(sides: Sides<f32>, axis: Axis) -> f32 {
    match axis {
        Axis::Horizontal => sides.left + sides.right,
        Axis::Vertical => sides.top + sides.bottom,
    }
}

/// The leading side (left or top) along an axis.
pub(crate) fn side_start(sides: Sides<f32>, axis: Axis) -> f32 {
    match axis {
        Axis::Horizontal => sides.left,
        Axis::Vertical => sides.top,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_helpers() {
        let rect = Rect::new(10.0, 20.0, 30.0, 40.0);
        assert!(rect.contains(Point::new(10.0, 20.0)));
        assert!(!rect.contains(Point::new(40.0, 20.0)));
        assert_eq!(rect.center(), Point::new(25.0, 40.0));
        assert_eq!(
            rect.inset(Sides::new(1.0, 2.0, 3.0, 4.0)),
            Rect::new(14.0, 21.0, 24.0, 36.0)
        );
        assert_eq!(
            rect.inset(Sides::all(100.0)),
            Rect::new(110.0, 120.0, 0.0, 0.0)
        );
        assert_eq!(
            rect.intersection(Rect::new(0.0, 0.0, 20.0, 30.0)),
            Rect::new(10.0, 20.0, 10.0, 10.0)
        );
    }

    #[test]
    fn axis_helpers() {
        let size = Size::new(3.0, 7.0);
        assert_eq!(size.main(Axis::Horizontal), 3.0);
        assert_eq!(size.cross(Axis::Horizontal), 7.0);
        assert_eq!(Size::from_axis(Axis::Vertical, 7.0, 3.0), size);
        assert_eq!(
            sides_along(Sides::new(1.0, 2.0, 3.0, 4.0), Axis::Vertical),
            4.0
        );
    }
}
