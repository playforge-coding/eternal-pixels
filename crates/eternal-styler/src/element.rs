// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! What a UI toolkit has to provide for its widgets to be styled.

use std::fmt;
use std::ptr::NonNull;

use bitflags::bitflags;
use selectors::OpaqueElement;
use selectors::attr::{AttrSelectorOperation, CaseSensitivity, NamespaceConstraint};
use selectors::bloom::BloomFilter;
use selectors::context::MatchingContext;
use selectors::matching::ElementSelectorFlags;

use crate::atom::{Atom, AttrValue};
use crate::selector::{PseudoClass, PseudoElement, SelectorImpl};

bitflags! {
    /// The interactive state of an element, matched by pseudo-classes.
    ///
    /// The toolkit keeps these up to date; the styler only reads them. Each
    /// flag has a pseudo-class, and a few pseudo-classes are the absence of a
    /// flag: `:enabled` is not `DISABLED`, `:read-write` is not `READ_ONLY`,
    /// `:optional` is not `REQUIRED` and `:valid` is not `INVALID`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct ElementState: u16 {
        /// `:hover`
        const HOVER = 1 << 0;
        /// `:active`
        const ACTIVE = 1 << 1;
        /// `:focus`
        const FOCUS = 1 << 2;
        /// `:focus-visible`
        const FOCUS_VISIBLE = 1 << 3;
        /// `:focus-within`
        const FOCUS_WITHIN = 1 << 4;
        /// `:disabled`
        const DISABLED = 1 << 5;
        /// `:checked`
        const CHECKED = 1 << 6;
        /// `:indeterminate`
        const INDETERMINATE = 1 << 7;
        /// `:read-only`
        const READ_ONLY = 1 << 8;
        /// `:required`
        const REQUIRED = 1 << 9;
        /// `:invalid`
        const INVALID = 1 << 10;
        /// `:placeholder-shown`
        const PLACEHOLDER_SHOWN = 1 << 11;
    }
}

/// A handle to a node in the toolkit's widget tree.
///
/// Implement this for a cheap handle type, such as `&Node` or an arena index
/// paired with a reference to the arena, and the styler can match selectors
/// against your tree. Everything except identity, tree navigation and the
/// element name has a default that reports nothing.
///
/// Tree-structural pseudo-classes (`:first-child`, `:nth-of-type()`, `:empty`
/// and friends) are answered from the navigation methods. The interactive
/// ones come from [`state`](Self::state), and `:state(name)` from
/// [`has_custom_state`](Self::has_custom_state).
pub trait Element: Clone + fmt::Debug {
    /// A value that identifies this element and no other for as long as the
    /// tree is alive.
    ///
    /// For a handle that is a reference, `OpaqueElement::new(node)` is the
    /// natural choice. For an arena index, use
    /// [`opaque_element_from_index`].
    fn opaque(&self) -> OpaqueElement;

    fn parent(&self) -> Option<Self>;

    fn prev_sibling(&self) -> Option<Self>;

    fn next_sibling(&self) -> Option<Self>;

    fn first_child(&self) -> Option<Self>;

    /// The name matched by a type selector, such as `button`.
    fn local_name(&self) -> &str;

    /// The name matched by an id selector, such as `#save`.
    fn id(&self) -> Option<&str> {
        None
    }

    /// The names matched by class selectors, such as `.primary`.
    fn classes(&self) -> impl Iterator<Item = &str> {
        std::iter::empty()
    }

    /// The value of an attribute, for `[name]` and `[name="value"]` selectors.
    fn attribute(&self, name: &str) -> Option<&str> {
        let _ = name;
        None
    }

    fn state(&self) -> ElementState {
        ElementState::empty()
    }

    /// Whether the element is in the custom state `name`, for `:state(name)`.
    fn has_custom_state(&self, name: &str) -> bool {
        let _ = name;
        false
    }

    /// Whether the element matches `:empty`. The default is "has no
    /// children"; override it if your elements can hold text.
    fn is_empty(&self) -> bool {
        self.first_child().is_none()
    }

    /// Whether the element matches `:root`.
    fn is_root(&self) -> bool {
        self.parent().is_none()
    }
}

/// An [`OpaqueElement`] for an element stored at `index` in an arena.
///
/// Two elements in the same tree must never share an index, and an index must
/// not be reused while styles are being computed.
pub fn opaque_element_from_index(index: usize) -> OpaqueElement {
    // OpaqueElement is only ever compared and hashed, never dereferenced, so
    // any distinct non-zero value will do.
    let address = index.wrapping_add(1).max(1) as *mut ();
    OpaqueElement::from_non_null_ptr(NonNull::new(address).expect("address is non-zero"))
}

/// Bridges the toolkit's [`Element`] to the `selectors` crate's own trait.
#[derive(Clone, Debug)]
pub(crate) struct Adapter<E>(pub(crate) E);

impl<E: Element> selectors::Element for Adapter<E> {
    type Impl = SelectorImpl;

    fn opaque(&self) -> OpaqueElement {
        self.0.opaque()
    }

    fn parent_element(&self) -> Option<Self> {
        self.0.parent().map(Adapter)
    }

    fn parent_node_is_shadow_root(&self) -> bool {
        false
    }

    fn containing_shadow_host(&self) -> Option<Self> {
        None
    }

    fn is_pseudo_element(&self) -> bool {
        false
    }

    fn prev_sibling_element(&self) -> Option<Self> {
        self.0.prev_sibling().map(Adapter)
    }

    fn next_sibling_element(&self) -> Option<Self> {
        self.0.next_sibling().map(Adapter)
    }

    fn first_element_child(&self) -> Option<Self> {
        self.0.first_child().map(Adapter)
    }

    fn is_html_element_in_html_document(&self) -> bool {
        false
    }

    fn has_local_name(&self, local_name: &str) -> bool {
        self.0.local_name() == local_name
    }

    fn has_namespace(&self, ns: &str) -> bool {
        ns.is_empty()
    }

    fn is_same_type(&self, other: &Self) -> bool {
        self.0.local_name() == other.0.local_name()
    }

    fn attr_matches(
        &self,
        ns: &NamespaceConstraint<&Atom>,
        local_name: &Atom,
        operation: &AttrSelectorOperation<&AttrValue>,
    ) -> bool {
        match ns {
            NamespaceConstraint::Any => {}
            NamespaceConstraint::Specific(url) if url.is_empty() => {}
            NamespaceConstraint::Specific(_) => return false,
        }
        match self.0.attribute(local_name) {
            Some(value) => operation.eval_str(value),
            None => false,
        }
    }

    fn match_non_ts_pseudo_class(
        &self,
        pseudo_class: &PseudoClass,
        _context: &mut MatchingContext<Self::Impl>,
    ) -> bool {
        pseudo_class.matches(&self.0)
    }

    fn match_pseudo_element(
        &self,
        pseudo_element: &PseudoElement,
        _context: &mut MatchingContext<Self::Impl>,
    ) -> bool {
        match *pseudo_element {}
    }

    fn apply_selector_flags(&self, _flags: ElementSelectorFlags) {}

    fn is_link(&self) -> bool {
        false
    }

    fn is_html_slot_element(&self) -> bool {
        false
    }

    fn has_id(&self, id: &Atom, case_sensitivity: CaseSensitivity) -> bool {
        self.0
            .id()
            .is_some_and(|own| case_sensitivity.eq(own.as_bytes(), id.as_bytes()))
    }

    fn has_class(&self, name: &Atom, case_sensitivity: CaseSensitivity) -> bool {
        self.0
            .classes()
            .any(|class| case_sensitivity.eq(class.as_bytes(), name.as_bytes()))
    }

    fn has_custom_state(&self, name: &Atom) -> bool {
        self.0.has_custom_state(name)
    }

    fn imported_part(&self, _name: &Atom) -> Option<Atom> {
        None
    }

    fn is_part(&self, _name: &Atom) -> bool {
        false
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn is_root(&self) -> bool {
        self.0.is_root()
    }

    fn add_element_unique_hashes(&self, _filter: &mut BloomFilter) -> bool {
        // No ancestor bloom filter: it is an optimisation for very deep
        // documents, and UI trees are shallow.
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arena_indices_map_to_distinct_identities() {
        assert_ne!(opaque_element_from_index(0), opaque_element_from_index(1));
        assert_eq!(opaque_element_from_index(7), opaque_element_from_index(7));
        // usize::MAX wraps to zero, which is not a valid pointer; make sure it
        // is still non-null rather than a panic.
        let _ = opaque_element_from_index(usize::MAX);
    }
}
