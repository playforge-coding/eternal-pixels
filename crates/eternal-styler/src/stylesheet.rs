// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Stylesheets, rules and declaration blocks.

use cssparser::{
    AtRuleParser, CowRcStr, DeclarationParser, Delimiter, ParseError, Parser, ParserInput,
    ParserState, QualifiedRuleParser, RuleBodyItemParser, RuleBodyParser, StyleSheetParser,
    parse_important,
};

use crate::error::{Error, StyleParseErrorKind, value_error};
use crate::properties::{PropertyDeclaration, PropertyId, parse_property_value};
use crate::selector::{RawSelectorList, SelectorList, parse_selector_list};

/// One longhand declaration and whether it was marked `!important`.
#[derive(Clone, Debug, PartialEq)]
pub struct Declaration {
    pub value: PropertyDeclaration,
    pub important: bool,
}

/// The `{ ... }` part of a rule, or an inline style.
///
/// Shorthands are expanded when parsed, so this only ever holds longhands.
/// Later declarations of the same longhand win over earlier ones.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeclarationBlock {
    declarations: Vec<Declaration>,
}

impl DeclarationBlock {
    pub fn new() -> Self {
        Self::default()
    }

    /// Parses declarations without the surrounding braces, such as
    /// `color: red; padding: 4px !important`. The first problem is an error.
    pub fn parse(css: &str) -> Result<Self, Error> {
        let (block, errors) = Self::parse_lenient(css);
        match errors.into_iter().next() {
            Some(error) => Err(error),
            None => Ok(block),
        }
    }

    /// Like [`parse`](Self::parse), but skips declarations it cannot parse
    /// the way a browser would, and returns the problems alongside.
    pub fn parse_lenient(css: &str) -> (Self, Vec<Error>) {
        let mut input = ParserInput::new(css);
        let mut parser = Parser::new(&mut input);
        let mut errors = Vec::new();
        let block = parse_declaration_block(&mut parser, &mut errors);
        (block, errors)
    }

    pub fn push(&mut self, value: PropertyDeclaration, important: bool) {
        self.declarations.push(Declaration { value, important });
    }

    pub fn declarations(&self) -> &[Declaration] {
        &self.declarations
    }

    pub fn len(&self) -> usize {
        self.declarations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty()
    }
}

/// A selector list and the declarations that apply to what it matches.
#[derive(Clone, Debug)]
pub struct StyleRule {
    pub selectors: SelectorList,
    pub declarations: DeclarationBlock,
}

/// A list of style rules in source order.
#[derive(Clone, Debug, Default)]
pub struct Stylesheet {
    rules: Vec<StyleRule>,
}

impl Stylesheet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Parses a stylesheet. The first problem is an error: an unknown
    /// property, a bad value, a selector this crate does not support, or an
    /// at-rule.
    pub fn parse(css: &str) -> Result<Self, Error> {
        let (sheet, errors) = Self::parse_lenient(css);
        match errors.into_iter().next() {
            Some(error) => Err(error),
            None => Ok(sheet),
        }
    }

    /// Like [`parse`](Self::parse), but skips what it cannot parse the way a
    /// browser would, and returns the problems alongside. A bad declaration
    /// loses only itself; a bad selector loses its whole rule.
    pub fn parse_lenient(css: &str) -> (Self, Vec<Error>) {
        let mut input = ParserInput::new(css);
        let mut parser = Parser::new(&mut input);
        let mut rule_parser = RuleParser { errors: Vec::new() };
        let mut rules = Vec::new();
        let mut errors = Vec::new();
        for result in StyleSheetParser::new(&mut parser, &mut rule_parser) {
            match result {
                Ok(rule) => rules.push(rule),
                Err((error, _)) => errors.push(Error::from_parse_error(error)),
            }
        }
        // Declaration errors were collected separately from rule errors;
        // put them back in source order.
        errors.append(&mut rule_parser.errors);
        errors.sort_by_key(|error| (error.line(), error.column()));
        (Self { rules }, errors)
    }

    pub fn push(&mut self, rule: StyleRule) {
        self.rules.push(rule);
    }

    pub fn rules(&self) -> &[StyleRule] {
        &self.rules
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

/// Parses the inside of a `{ ... }` block, collecting problems into `errors`.
fn parse_declaration_block(
    input: &mut Parser<'_, '_>,
    errors: &mut Vec<Error>,
) -> DeclarationBlock {
    let mut block = DeclarationBlock::new();
    let mut parser = DeclarationBlockParser;
    for item in RuleBodyParser::new(input, &mut parser) {
        match item {
            Ok(declarations) => block.declarations.extend(declarations),
            Err((error, _)) => errors.push(Error::from_parse_error(error)),
        }
    }
    block
}

struct DeclarationBlockParser;

impl<'i> DeclarationParser<'i> for DeclarationBlockParser {
    type Declaration = Vec<Declaration>;
    type Error = StyleParseErrorKind<'i>;

    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        declaration_start: &ParserState,
    ) -> Result<Self::Declaration, ParseError<'i, Self::Error>> {
        let Some(id) = PropertyId::from_name(&name) else {
            return Err(declaration_start
                .source_location()
                .new_custom_error(StyleParseErrorKind::UnknownProperty(name)));
        };
        let mut values = Vec::new();
        input
            .parse_until_before(Delimiter::Bang, |input| {
                parse_property_value(id, input, &mut values)?;
                input.expect_exhausted()?;
                Ok(())
            })
            .map_err(|error| value_error(name.clone(), error))?;
        let important = input.try_parse(parse_important).is_ok();
        input
            .expect_exhausted()
            .map_err(|error| value_error(name, error.into()))?;
        Ok(values
            .into_iter()
            .map(|value| Declaration { value, important })
            .collect())
    }
}

// Declaration blocks hold neither at-rules nor nested rules. The default
// implementations reject both.
impl<'i> AtRuleParser<'i> for DeclarationBlockParser {
    type Prelude = ();
    type AtRule = Vec<Declaration>;
    type Error = StyleParseErrorKind<'i>;
}

impl<'i> QualifiedRuleParser<'i> for DeclarationBlockParser {
    type Prelude = ();
    type QualifiedRule = Vec<Declaration>;
    type Error = StyleParseErrorKind<'i>;
}

impl<'i> RuleBodyItemParser<'i, Vec<Declaration>, StyleParseErrorKind<'i>>
    for DeclarationBlockParser
{
    fn parse_declarations(&self) -> bool {
        true
    }

    fn parse_qualified(&self) -> bool {
        false
    }
}

struct RuleParser {
    errors: Vec<Error>,
}

impl<'i> QualifiedRuleParser<'i> for RuleParser {
    type Prelude = RawSelectorList;
    type QualifiedRule = StyleRule;
    type Error = StyleParseErrorKind<'i>;

    fn parse_prelude<'t>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
        parse_selector_list(input)
    }

    fn parse_block<'t>(
        &mut self,
        prelude: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::QualifiedRule, ParseError<'i, Self::Error>> {
        let declarations = parse_declaration_block(input, &mut self.errors);
        Ok(StyleRule {
            selectors: SelectorList::from_raw(prelude),
            declarations,
        })
    }
}

// No at-rules. The default implementation reports them as invalid, which
// becomes `ErrorKind::UnsupportedAtRule`.
impl<'i> AtRuleParser<'i> for RuleParser {
    type Prelude = ();
    type AtRule = StyleRule;
    type Error = StyleParseErrorKind<'i>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorKind;
    use crate::properties::LonghandId;

    #[test]
    fn parses_rules_and_declarations() {
        let sheet = Stylesheet::parse(
            "button, .link { color: red; padding: 4px 8px !important }\n\
             /* a comment */\n\
             #save:hover { opacity: .5 }",
        )
        .unwrap();
        assert_eq!(sheet.len(), 2);
        let first = &sheet.rules()[0];
        assert_eq!(first.selectors.len(), 2);
        assert_eq!(first.declarations.len(), 5);
        assert!(!first.declarations.declarations()[0].important);
        assert!(first.declarations.declarations()[1].important);
        assert_eq!(
            first.declarations.declarations()[4].value.id(),
            LonghandId::PaddingLeft
        );
        assert_eq!(sheet.rules()[1].selectors.to_string(), "#save:hover");
    }

    #[test]
    fn empty_input_is_fine() {
        assert!(Stylesheet::parse("").unwrap().is_empty());
        assert!(Stylesheet::parse("  \n ").unwrap().is_empty());
        assert!(DeclarationBlock::parse("").unwrap().is_empty());
        assert!(DeclarationBlock::parse(";;").unwrap().is_empty());
    }

    #[test]
    fn reports_where_things_went_wrong() {
        let error = Stylesheet::parse("button {\n  colour: red;\n}").unwrap_err();
        assert_eq!(error.kind(), &ErrorKind::UnknownProperty("colour".into()));
        assert_eq!((error.line(), error.column()), (2, 3));

        let error = Stylesheet::parse("button { width: red }").unwrap_err();
        assert_eq!(
            error.to_string(),
            "line 1, column 17: invalid value for 'width': unexpected 'red'"
        );

        let error = Stylesheet::parse("button { width: 1px 2px }").unwrap_err();
        assert!(
            matches!(error.kind(), ErrorKind::InvalidValue { .. }),
            "{error}"
        );

        let error = Stylesheet::parse("button { color: red !importantly }").unwrap_err();
        assert!(
            matches!(error.kind(), ErrorKind::InvalidValue { .. }),
            "{error}"
        );

        let error = Stylesheet::parse("@media (width > 0) { }").unwrap_err();
        assert_eq!(error.kind(), &ErrorKind::UnsupportedAtRule("media".into()));

        let error = Stylesheet::parse("a::before { }").unwrap_err();
        assert!(
            matches!(error.kind(), ErrorKind::InvalidSelector(_)),
            "{error}"
        );

        let error = DeclarationBlock::parse("color red").unwrap_err();
        assert!(matches!(error.kind(), ErrorKind::Syntax(_)), "{error}");
    }

    #[test]
    fn lenient_parsing_keeps_what_it_can() {
        let (sheet, errors) = Stylesheet::parse_lenient(
            "a { color: red; colour: blue; width: 1px }\n\
             b::after { color: red }\n\
             @import 'x.css';\n\
             c { color: green }",
        );
        assert_eq!(sheet.len(), 2);
        assert_eq!(sheet.rules()[0].declarations.len(), 2);
        assert_eq!(sheet.rules()[1].selectors.to_string(), "c");
        let lines: Vec<u32> = errors.iter().map(Error::line).collect();
        assert_eq!(lines, [1, 2, 3]);

        let (block, errors) = DeclarationBlock::parse_lenient("color: red; nope: 1; opacity: 0");
        assert_eq!(block.len(), 2);
        assert_eq!(errors.len(), 1);
    }
}
