// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The string types parsed selectors are built from.

use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

use cssparser::{ToCss, serialize_identifier, serialize_string};
use precomputed_hash::PrecomputedHash;

/// A cheaply cloneable string with a precomputed hash.
///
/// Element names, ids, class names and `:state()` arguments are stored as
/// atoms inside parsed selectors. You only meet this type through
/// [`PseudoClass::CustomState`](crate::PseudoClass::CustomState).
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Atom {
    text: Arc<str>,
    hash: u32,
}

impl Atom {
    pub fn new(text: &str) -> Self {
        Self {
            text: Arc::from(text),
            hash: fnv1a(text),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }
}

impl Default for Atom {
    fn default() -> Self {
        Self::new("")
    }
}

impl Deref for Atom {
    type Target = str;

    fn deref(&self) -> &str {
        &self.text
    }
}

impl Borrow<str> for Atom {
    fn borrow(&self) -> &str {
        &self.text
    }
}

impl From<&str> for Atom {
    fn from(text: &str) -> Self {
        Self::new(text)
    }
}

impl fmt::Debug for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.text, f)
    }
}

impl fmt::Display for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

impl PrecomputedHash for Atom {
    fn precomputed_hash(&self) -> u32 {
        self.hash
    }
}

impl ToCss for Atom {
    fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
        serialize_identifier(&self.text, dest)
    }
}

/// The value part of an attribute selector, the `"x"` in `[name="x"]`.
#[derive(Clone, PartialEq, Eq, Hash, Default)]
pub struct AttrValue(Arc<str>);

impl AsRef<str> for AttrValue {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for AttrValue {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl From<&str> for AttrValue {
    fn from(text: &str) -> Self {
        Self(Arc::from(text))
    }
}

impl fmt::Debug for AttrValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.0, f)
    }
}

impl ToCss for AttrValue {
    fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
        serialize_string(&self.0, dest)
    }
}

fn fnv1a(text: &str) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in text.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_atoms_share_a_hash() {
        assert_eq!(Atom::new("button"), Atom::from("button"));
        assert_eq!(
            Atom::new("button").precomputed_hash(),
            Atom::new("button").precomputed_hash()
        );
        assert_ne!(
            Atom::new("button").precomputed_hash(),
            Atom::new("label").precomputed_hash()
        );
        assert_eq!(Atom::default(), Atom::new(""));
    }

    #[test]
    fn serialises_as_css() {
        assert_eq!(Atom::new("a b").to_css_string(), "a\\ b");
        assert_eq!(AttrValue::from("a\"b").to_css_string(), "\"a\\\"b\"");
    }
}
