// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Selectors: the `selectors` crate configured for UI trees.

use std::fmt;

use cssparser::{CowRcStr, ParseError, Parser, ParserInput, SourceLocation, ToCss};
use selectors::context::{
    MatchingContext, MatchingForInvalidation, MatchingMode, NeedsSelectorFlags, QuirksMode,
    SelectorCaches,
};
use selectors::matching::{matches_selector, matches_selector_list};
use selectors::parser::{NonTSPseudoClass, ParseRelative, SelectorParseErrorKind};

use crate::atom::{Atom, AttrValue};
use crate::element::{Adapter, Element, ElementState};
use crate::error::{Error, StyleError, StyleParseErrorKind};

/// The type-level configuration handed to the `selectors` crate: which
/// string types, pseudo-classes and pseudo-elements selectors are made of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectorImpl;

impl selectors::parser::SelectorImpl for SelectorImpl {
    type ExtraMatchingData<'a> = ();
    type AttrValue = AttrValue;
    type Identifier = Atom;
    type LocalName = Atom;
    type NamespaceUrl = Atom;
    type NamespacePrefix = Atom;
    type BorrowedNamespaceUrl = str;
    type BorrowedLocalName = str;
    type NonTSPseudoClass = PseudoClass;
    type PseudoElement = PseudoElement;
}

/// A pseudo-class answered by [`Element::state`] or
/// [`Element::has_custom_state`] rather than by the shape of the tree.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PseudoClass {
    Hover,
    Active,
    Focus,
    FocusVisible,
    FocusWithin,
    Enabled,
    Disabled,
    Checked,
    Indeterminate,
    ReadOnly,
    ReadWrite,
    Required,
    Optional,
    Valid,
    Invalid,
    PlaceholderShown,
    /// `:state(name)`, a state the toolkit defines itself.
    CustomState(Atom),
}

impl PseudoClass {
    pub(crate) fn matches<E: Element>(&self, element: &E) -> bool {
        let state = element.state();
        match self {
            Self::Hover => state.contains(ElementState::HOVER),
            Self::Active => state.contains(ElementState::ACTIVE),
            Self::Focus => state.contains(ElementState::FOCUS),
            Self::FocusVisible => state.contains(ElementState::FOCUS_VISIBLE),
            Self::FocusWithin => state.contains(ElementState::FOCUS_WITHIN),
            Self::Enabled => !state.contains(ElementState::DISABLED),
            Self::Disabled => state.contains(ElementState::DISABLED),
            Self::Checked => state.contains(ElementState::CHECKED),
            Self::Indeterminate => state.contains(ElementState::INDETERMINATE),
            Self::ReadOnly => state.contains(ElementState::READ_ONLY),
            Self::ReadWrite => !state.contains(ElementState::READ_ONLY),
            Self::Required => state.contains(ElementState::REQUIRED),
            Self::Optional => !state.contains(ElementState::REQUIRED),
            Self::Valid => !state.contains(ElementState::INVALID),
            Self::Invalid => state.contains(ElementState::INVALID),
            Self::PlaceholderShown => state.contains(ElementState::PLACEHOLDER_SHOWN),
            Self::CustomState(name) => element.has_custom_state(name),
        }
    }

    fn parse(location: SourceLocation, name: CowRcStr<'_>) -> Result<Self, StyleError<'_>> {
        Ok(cssparser::match_ignore_ascii_case! { &name,
            "hover" => Self::Hover,
            "active" => Self::Active,
            "focus" => Self::Focus,
            "focus-visible" => Self::FocusVisible,
            "focus-within" => Self::FocusWithin,
            "enabled" => Self::Enabled,
            "disabled" => Self::Disabled,
            "checked" => Self::Checked,
            "indeterminate" => Self::Indeterminate,
            "read-only" => Self::ReadOnly,
            "read-write" => Self::ReadWrite,
            "required" => Self::Required,
            "optional" => Self::Optional,
            "valid" => Self::Valid,
            "invalid" => Self::Invalid,
            "placeholder-shown" => Self::PlaceholderShown,
            _ => {
                return Err(location.new_custom_error(
                    SelectorParseErrorKind::UnsupportedPseudoClassOrElement(name),
                ));
            }
        })
    }
}

impl ToCss for PseudoClass {
    fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
        let name = match self {
            Self::Hover => "hover",
            Self::Active => "active",
            Self::Focus => "focus",
            Self::FocusVisible => "focus-visible",
            Self::FocusWithin => "focus-within",
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
            Self::Checked => "checked",
            Self::Indeterminate => "indeterminate",
            Self::ReadOnly => "read-only",
            Self::ReadWrite => "read-write",
            Self::Required => "required",
            Self::Optional => "optional",
            Self::Valid => "valid",
            Self::Invalid => "invalid",
            Self::PlaceholderShown => "placeholder-shown",
            Self::CustomState(name) => {
                dest.write_str(":state(")?;
                name.to_css(dest)?;
                return dest.write_str(")");
            }
        };
        dest.write_str(":")?;
        dest.write_str(name)
    }
}

impl NonTSPseudoClass for PseudoClass {
    type Impl = SelectorImpl;

    fn is_active_or_hover(&self) -> bool {
        matches!(self, Self::Active | Self::Hover)
    }

    fn is_user_action_state(&self) -> bool {
        matches!(
            self,
            Self::Active | Self::Hover | Self::Focus | Self::FocusVisible | Self::FocusWithin
        )
    }
}

/// There are no pseudo-elements. This type has no values; it exists because
/// the `selectors` crate needs one.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PseudoElement {}

impl ToCss for PseudoElement {
    fn to_css<W: fmt::Write>(&self, _dest: &mut W) -> fmt::Result {
        match *self {}
    }
}

impl selectors::parser::PseudoElement for PseudoElement {
    type Impl = SelectorImpl;
}

/// The hooks the `selectors` parser calls for the bits it does not define
/// itself.
pub(crate) struct SelectorParser;

impl<'i> selectors::Parser<'i> for SelectorParser {
    type Impl = SelectorImpl;
    type Error = StyleParseErrorKind<'i>;

    fn parse_is_and_where(&self) -> bool {
        true
    }

    fn parse_has(&self) -> bool {
        true
    }

    fn parse_nth_child_of(&self) -> bool {
        true
    }

    fn parse_non_ts_pseudo_class(
        &self,
        location: SourceLocation,
        name: CowRcStr<'i>,
    ) -> Result<PseudoClass, ParseError<'i, Self::Error>> {
        PseudoClass::parse(location, name)
    }

    fn parse_non_ts_functional_pseudo_class<'t>(
        &self,
        name: CowRcStr<'i>,
        parser: &mut Parser<'i, 't>,
        _after_part: bool,
    ) -> Result<PseudoClass, ParseError<'i, Self::Error>> {
        if name.eq_ignore_ascii_case("state") {
            let state = parser.expect_ident()?;
            return Ok(PseudoClass::CustomState(Atom::new(state)));
        }
        Err(
            parser.new_custom_error(SelectorParseErrorKind::UnsupportedPseudoClassOrElement(
                name,
            )),
        )
    }
}

pub(crate) type RawSelector = selectors::parser::Selector<SelectorImpl>;
pub(crate) type RawSelectorList = selectors::SelectorList<SelectorImpl>;

pub(crate) fn parse_selector_list<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<RawSelectorList, StyleError<'i>> {
    RawSelectorList::parse(&SelectorParser, input, ParseRelative::No)
}

/// Runs `f` with a matching context set up the way this crate matches: no
/// quirks, no bloom filter, no invalidation bookkeeping.
pub(crate) fn with_matching_context<R>(
    f: impl FnOnce(&mut MatchingContext<'_, SelectorImpl>) -> R,
) -> R {
    let mut caches = SelectorCaches::default();
    let mut context = MatchingContext::new(
        MatchingMode::Normal,
        None,
        &mut caches,
        QuirksMode::NoQuirks,
        NeedsSelectorFlags::No,
        MatchingForInvalidation::No,
    );
    f(&mut context)
}

/// One parsed selector, such as `button.primary:hover`.
#[derive(Clone, Debug)]
pub struct Selector(RawSelector);

impl Selector {
    /// The selector's specificity, packed the way browsers do it: ids in the
    /// top bits, then classes, attributes and pseudo-classes, then types.
    /// Higher wins.
    pub fn specificity(&self) -> u32 {
        self.0.specificity()
    }

    pub fn matches<E: Element>(&self, element: &E) -> bool {
        let element = Adapter(element.clone());
        with_matching_context(|context| matches_selector(&self.0, 0, None, &element, context))
    }
}

impl fmt::Display for Selector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.to_css(f)
    }
}

/// A comma-separated list of selectors, the prelude of a style rule.
#[derive(Clone, Debug)]
pub struct SelectorList(RawSelectorList);

impl SelectorList {
    /// Parses something like `button, .link:hover`.
    pub fn parse(css: &str) -> Result<Self, Error> {
        let mut input = ParserInput::new(css);
        let mut parser = Parser::new(&mut input);
        parser
            .parse_entirely(parse_selector_list)
            .map(Self)
            .map_err(Error::from_parse_error)
    }

    pub(crate) fn from_raw(raw: RawSelectorList) -> Self {
        Self(raw)
    }

    pub(crate) fn raw(&self) -> &RawSelectorList {
        &self.0
    }

    /// Whether any selector in the list matches.
    pub fn matches<E: Element>(&self, element: &E) -> bool {
        let element = Adapter(element.clone());
        with_matching_context(|context| matches_selector_list(&self.0, &element, context))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.len() == 0
    }

    /// The selectors in the list, in source order.
    pub fn selectors(&self) -> impl Iterator<Item = Selector> + '_ {
        self.0.slice().iter().map(|raw| Selector(raw.clone()))
    }
}

impl fmt::Display for SelectorList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.to_css(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorKind;

    #[test]
    fn parses_and_serialises() {
        let list = SelectorList::parse("button.primary:hover, #save:not(:disabled)").unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(
            list.to_string(),
            "button.primary:hover, #save:not(:disabled)"
        );
        let state = SelectorList::parse("panel:state(loading)").unwrap();
        assert_eq!(state.to_string(), "panel:state(loading)");
    }

    #[test]
    fn specificity_orders_as_expected() {
        let spec = |css: &str| {
            SelectorList::parse(css)
                .unwrap()
                .selectors()
                .next()
                .unwrap()
                .specificity()
        };
        assert!(spec("#a") > spec(".a.b.c"));
        assert!(spec(".a") > spec("a b c"));
        assert!(spec("a:hover") > spec("a"));
        assert_eq!(spec(":where(.a)"), spec("*"));
        assert_eq!(spec(":is(.a, #b)"), spec("#b"));
    }

    #[test]
    fn rejects_what_it_does_not_support() {
        let err = SelectorList::parse("a::before").unwrap_err();
        assert!(matches!(err.kind(), ErrorKind::InvalidSelector(_)), "{err}");
        let err = SelectorList::parse("a:visited").unwrap_err();
        assert_eq!(
            err.to_string(),
            "line 1, column 3: invalid selector: unsupported pseudo-class or pseudo-element ':visited'"
        );
        assert!(SelectorList::parse("a,").is_err());
        assert!(SelectorList::parse("").is_err());
    }
}
