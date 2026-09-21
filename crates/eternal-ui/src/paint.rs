// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Turning the laid-out tree into drawing calls.

use eternal_styler::Corners;

use crate::render::Renderer;
use crate::ui::{NodeId, Ui};
use crate::widget::PaintCx;

impl<M: 'static> Ui<M> {
    /// Draws the whole tree, as last laid out, through `renderer`.
    ///
    /// For each element, in tree order: the background, the border, the
    /// widget's own content, then its children. Nothing is clipped unless a
    /// widget asks for it, so an element that overflows its parent shows.
    pub fn paint(&self, renderer: &mut dyn Renderer) {
        if let Some(root) = self.root() {
            self.paint_node(root, renderer);
        }
    }

    fn paint_node(&self, id: NodeId, renderer: &mut dyn Renderer) {
        let node = self.node(id);
        let style = &node.style;
        let rect = node.rect;
        if rect.width <= 0.0 || rect.height <= 0.0 {
            return;
        }
        let translucent = style.opacity < 1.0;
        if translucent {
            renderer.push_opacity(style.opacity);
        }

        let radius = Corners::new(
            style.border_radius.top_left.resolve(rect.width),
            style.border_radius.top_right.resolve(rect.width),
            style.border_radius.bottom_right.resolve(rect.width),
            style.border_radius.bottom_left.resolve(rect.width),
        );
        if style.background_color.a > 0.0 {
            renderer.fill_rect(rect, radius, style.background_color);
        }
        let widths = style.border_width;
        if widths.top > 0.0 || widths.right > 0.0 || widths.bottom > 0.0 || widths.left > 0.0 {
            renderer.border(rect, widths, style.border_color, radius);
        }

        let cx = PaintCx {
            style,
            state: self.state_of(id),
        };
        node.widget.paint(renderer, &cx, node.content);

        for &child in &node.children {
            self.paint_node(child, renderer);
        }

        if translucent {
            renderer.pop_opacity();
        }
    }
}
