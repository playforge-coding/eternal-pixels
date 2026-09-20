// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The engine: stylesheets in, computed styles out.

use selectors::matching::matches_selector;

use crate::cascade::{self, Winners};
use crate::computed::ComputedStyle;
use crate::element::{Adapter, Element};
use crate::selector::with_matching_context;
use crate::stylesheet::{DeclarationBlock, StyleRule, Stylesheet};

/// Holds stylesheets and computes styles for elements against them.
///
/// Stylesheets are consulted in the order they were added, and a later
/// stylesheet wins over an earlier one when specificity ties, so add the
/// theme first and overrides after.
///
/// Computing a style needs the parent's computed style for inheritance, so
/// walk the tree top-down and keep each parent's result around for its
/// children.
#[derive(Clone, Debug)]
pub struct Styler {
    sheets: Vec<Stylesheet>,
    root_font_size: f32,
}

impl Default for Styler {
    fn default() -> Self {
        Self::new()
    }
}

impl Styler {
    /// An engine with no stylesheets and a root font size of 16px.
    pub fn new() -> Self {
        Self {
            sheets: Vec::new(),
            root_font_size: 16.0,
        }
    }

    pub fn add_stylesheet(&mut self, sheet: Stylesheet) {
        self.sheets.push(sheet);
    }

    pub fn stylesheets(&self) -> &[Stylesheet] {
        &self.sheets
    }

    pub fn clear_stylesheets(&mut self) {
        self.sheets.clear();
    }

    /// The font size `rem` is relative to, in pixels. Tie it to the user's
    /// text scale to make every `rem` in the stylesheets follow it.
    pub fn set_root_font_size(&mut self, px: f32) {
        self.root_font_size = px;
    }

    pub fn root_font_size(&self) -> f32 {
        self.root_font_size
    }

    /// The style of `element` given its parent's computed style, or `None`
    /// for the root.
    pub fn compute<E: Element>(
        &self,
        element: &E,
        parent: Option<&ComputedStyle>,
    ) -> ComputedStyle {
        self.compute_with_inline(element, parent, None)
    }

    /// Like [`compute`](Self::compute), with an inline style that beats every
    /// stylesheet declaration of the same importance.
    pub fn compute_with_inline<E: Element>(
        &self,
        element: &E,
        parent: Option<&ComputedStyle>,
        inline: Option<&DeclarationBlock>,
    ) -> ComputedStyle {
        let matches = self.matches(element);
        let mut winners = Winners::new();
        // Normal declarations first, weakest to strongest, then the important
        // ones in the same order. Inline sits above every stylesheet within
        // its importance.
        for important in [false, true] {
            for m in &matches {
                for declaration in m.rule.declarations.declarations() {
                    if declaration.important == important {
                        winners.set(&declaration.value);
                    }
                }
            }
            if let Some(inline) = inline {
                for declaration in inline.declarations() {
                    if declaration.important == important {
                        winners.set(&declaration.value);
                    }
                }
            }
        }
        cascade::compute(&winners, parent, self.root_font_size)
    }

    /// The rules that match `element`, weakest first: what
    /// [`compute`](Self::compute) applies, in the order it applies them.
    /// Handy for a style inspector.
    pub fn matching_rules<E: Element>(&self, element: &E) -> Vec<&StyleRule> {
        self.matches(element).into_iter().map(|m| m.rule).collect()
    }

    fn matches<E: Element>(&self, element: &E) -> Vec<Match<'_>> {
        let element = Adapter(element.clone());
        let mut found = Vec::new();
        with_matching_context(|context| {
            let rules = self.sheets.iter().flat_map(Stylesheet::rules);
            for (order, rule) in rules.enumerate() {
                // A rule with several selectors is as specific as the most
                // specific selector that matches.
                let specificity = rule
                    .selectors
                    .raw()
                    .slice()
                    .iter()
                    .filter(|selector| matches_selector(selector, 0, None, &element, context))
                    .map(|selector| selector.specificity())
                    .max();
                if let Some(specificity) = specificity {
                    found.push(Match {
                        specificity,
                        order,
                        rule,
                    });
                }
            }
        });
        found.sort_by_key(|m| (m.specificity, m.order));
        found
    }
}

struct Match<'a> {
    specificity: u32,
    order: usize,
    rule: &'a StyleRule,
}
