// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Sizing and placing elements.
//!
//! Containers lay their children out along one axis, like a CSS flex row or
//! column with `flex-wrap: nowrap`. Sizes come from the stylesheet (`width`,
//! `height`, `min-*`, `max-*`, `margin`, `padding`, `border`, `gap`); which
//! way a container goes, whether a child grows, and how children are
//! aligned are attributes on the elements, since they are about structure
//! rather than looks: `grow` and `align` on a child, `align_items` and
//! `justify` on a container.
//!
//! `width` and `height` are border-box sizes, as with
//! `box-sizing: border-box`. Percentages are of the parent's content box.
//! The root always fills the viewport.

use eternal_styler::{ComputedStyle, LengthPercentageOrAuto, LengthPercentageOrNone, Sides};

use crate::geometry::{Axis, Point, Rect, Size, side_start, sides_along};
use crate::render::Fonts;
use crate::ui::{NodeId, Ui};

/// Where a child sits across its container's axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Align {
    Start,
    Center,
    End,
    /// Fill the container across the axis, unless the child has an explicit
    /// size that way.
    #[default]
    Stretch,
}

impl Align {
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text.trim() {
            "start" => Self::Start,
            "center" => Self::Center,
            "end" => Self::End,
            "stretch" => Self::Stretch,
            _ => return None,
        })
    }
}

/// How a container spreads its children along its axis when they do not
/// fill it and none of them grows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    /// The first child at the start, the last at the end, equal gaps
    /// between.
    SpaceBetween,
}

impl Justify {
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text.trim() {
            "start" => Self::Start,
            "center" => Self::Center,
            "end" => Self::End,
            "space-between" => Self::SpaceBetween,
            _ => return None,
        })
    }
}

/// Padding plus border on each side, in pixels.
fn edges(style: &ComputedStyle, basis: f32) -> Sides<f32> {
    Sides::new(
        style.padding.top.resolve(basis) + style.border_width.top,
        style.padding.right.resolve(basis) + style.border_width.right,
        style.padding.bottom.resolve(basis) + style.border_width.bottom,
        style.padding.left.resolve(basis) + style.border_width.left,
    )
}

/// Margins in pixels. `auto` counts as zero.
fn margins(style: &ComputedStyle, basis: f32) -> Sides<f32> {
    let resolve = |m: LengthPercentageOrAuto| m.resolve(basis).unwrap_or(0.0);
    Sides::new(
        resolve(style.margin.top),
        resolve(style.margin.right),
        resolve(style.margin.bottom),
        resolve(style.margin.left),
    )
}

fn clamp(value: f32, min: LengthPercentageOrAuto, max: LengthPercentageOrNone, basis: f32) -> f32 {
    let value = match max.resolve(basis) {
        Some(max) => value.min(max),
        None => value,
    };
    match min.resolve(basis) {
        Some(min) => value.max(min),
        None => value,
    }
}

impl<M: 'static> Ui<M> {
    /// Recomputes styles if anything changed, then sizes and places every
    /// element for a window of `viewport` pixels. Call before painting and
    /// before hit-testing events after a change.
    pub fn layout(&mut self, viewport: Size, fonts: &dyn Fonts) {
        self.viewport = viewport;
        if self.dirty {
            self.recompute_styles();
        }
        let Some(root) = self.root() else {
            return;
        };
        self.measure(root, viewport, fonts);
        self.arrange(root, Rect::from_origin_size(Point::ZERO, viewport));
        self.dirty = false;
    }

    /// The natural border-box size of `id` given the room its parent has,
    /// stored on the node and returned.
    fn measure(&mut self, id: NodeId, available: Size, fonts: &dyn Fonts) -> Size {
        let node = self.node(id);
        let style = node.style.clone();
        let axis = node.widget.axis();
        let children = node.children.clone();
        let edges = edges(&style, available.width);
        let inner = Size::new(
            available.width - edges.left - edges.right,
            available.height - edges.top - edges.bottom,
        )
        .non_negative();

        let content = match axis {
            Some(axis) => {
                let gap = match axis {
                    Axis::Horizontal => style.column_gap,
                    Axis::Vertical => style.row_gap,
                }
                .resolve(inner.main(axis));
                let mut main = 0.0;
                let mut cross: f32 = 0.0;
                for (index, &child) in children.iter().enumerate() {
                    let margin = margins(&self.node(child).style, inner.width);
                    let size = self.measure(child, inner, fonts);
                    main += size.main(axis) + sides_along(margin, axis);
                    cross = cross.max(size.cross(axis) + sides_along(margin, axis.cross()));
                    if index > 0 {
                        main += gap;
                    }
                }
                Size::from_axis(axis, main, cross)
            }
            None => node.widget.measure(fonts, &style),
        };

        let width = match style.width {
            LengthPercentageOrAuto::Auto => content.width + edges.left + edges.right,
            LengthPercentageOrAuto::Px(px) => px,
            LengthPercentageOrAuto::Percent(p) => p * available.width,
        };
        let height = match style.height {
            LengthPercentageOrAuto::Auto => content.height + edges.top + edges.bottom,
            LengthPercentageOrAuto::Px(px) => px,
            LengthPercentageOrAuto::Percent(p) => p * available.height,
        };
        let size = Size::new(
            clamp(width, style.min_width, style.max_width, available.width),
            clamp(height, style.min_height, style.max_height, available.height),
        )
        .non_negative();
        self.node_mut(id).measured = size;
        size
    }

    /// Gives `id` its final border box and places its children inside.
    fn arrange(&mut self, id: NodeId, rect: Rect) {
        let node = self.node(id);
        let style = node.style.clone();
        let Some(axis) = node.widget.axis() else {
            let content = rect.inset(edges(&style, rect.width));
            let node = self.node_mut(id);
            node.rect = rect;
            node.content = content;
            return;
        };
        let children = node.children.clone();
        let (container_align, justify) = node.container_alignment();
        let content = rect.inset(edges(&style, rect.width));
        {
            let node = self.node_mut(id);
            node.rect = rect;
            node.content = content;
        }

        let gap = match axis {
            Axis::Horizontal => style.column_gap,
            Axis::Vertical => style.row_gap,
        }
        .resolve(content.size().main(axis));

        // What each child asked for.
        struct Item {
            id: NodeId,
            measured: Size,
            margin: Sides<f32>,
            grow: f32,
            align: Align,
            cross_is_auto: bool,
        }
        let items: Vec<Item> = children
            .iter()
            .map(|&child| {
                let node = self.node(child);
                let cross_is_auto = match axis {
                    Axis::Horizontal => node.style.height == LengthPercentageOrAuto::Auto,
                    Axis::Vertical => node.style.width == LengthPercentageOrAuto::Auto,
                };
                Item {
                    id: child,
                    measured: node.measured,
                    margin: margins(&node.style, content.width),
                    grow: node.grow,
                    align: node.align.unwrap_or(container_align),
                    cross_is_auto,
                }
            })
            .collect();

        let used: f32 = items
            .iter()
            .map(|item| item.measured.main(axis) + sides_along(item.margin, axis))
            .sum::<f32>()
            + gap * items.len().saturating_sub(1) as f32;
        let free = (content.size().main(axis) - used).max(0.0);
        let grow_total: f32 = items.iter().map(|item| item.grow).sum();

        let (mut offset, extra_gap) = if grow_total > 0.0 || items.is_empty() {
            (0.0, 0.0)
        } else {
            match justify {
                Justify::Start => (0.0, 0.0),
                Justify::Center => (free / 2.0, 0.0),
                Justify::End => (free, 0.0),
                Justify::SpaceBetween if items.len() > 1 => (0.0, free / (items.len() - 1) as f32),
                Justify::SpaceBetween => (0.0, 0.0),
            }
        };

        let content_cross = content.size().cross(axis);
        for (index, item) in items.iter().enumerate() {
            if index > 0 {
                offset += gap + extra_gap;
            }
            let main = item.measured.main(axis)
                + if grow_total > 0.0 {
                    free * item.grow / grow_total
                } else {
                    0.0
                };
            let cross_room = content_cross - sides_along(item.margin, axis.cross());
            let cross = if item.align == Align::Stretch && item.cross_is_auto {
                cross_room.max(item.measured.cross(axis))
            } else {
                item.measured.cross(axis)
            };
            let cross_offset = match item.align {
                Align::Start | Align::Stretch => 0.0,
                Align::Center => ((cross_room - cross) / 2.0).max(0.0),
                Align::End => (cross_room - cross).max(0.0),
            };
            let main_start = offset + side_start(item.margin, axis);
            let cross_start = cross_offset + side_start(item.margin, axis.cross());
            let child_rect = match axis {
                Axis::Horizontal => {
                    Rect::new(content.x + main_start, content.y + cross_start, main, cross)
                }
                Axis::Vertical => {
                    Rect::new(content.x + cross_start, content.y + main_start, cross, main)
                }
            };
            self.arrange(item.id, child_rect);
            offset += main + sides_along(item.margin, axis);
        }
    }
}
