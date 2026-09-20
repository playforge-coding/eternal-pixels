// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Parse errors.

use std::fmt;

use cssparser::{BasicParseErrorKind, CowRcStr, ParseError, ParseErrorKind, ToCss, Token};
use selectors::parser::SelectorParseErrorKind;

/// Something in a stylesheet, declaration block or selector could not be
/// parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    kind: ErrorKind,
    line: u32,
    column: u32,
}

impl Error {
    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }

    /// The line the error was found on, starting at 1.
    pub fn line(&self) -> u32 {
        self.line
    }

    /// The column the error was found at, starting at 1 and counted in
    /// UTF-16 code units, the way editors count.
    pub fn column(&self) -> u32 {
        self.column
    }

    pub(crate) fn from_parse_error(error: ParseError<'_, StyleParseErrorKind<'_>>) -> Self {
        Self {
            kind: ErrorKind::from_parse_error_kind(error.kind),
            line: error.location.line + 1,
            column: error.location.column,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "line {}, column {}: {}",
            self.line, self.column, self.kind
        )
    }
}

impl std::error::Error for Error {}

/// What went wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorKind {
    /// A property this crate does not know about.
    UnknownProperty(String),
    /// A known property with a value it does not accept.
    InvalidValue {
        property: String,
        reason: String,
    },
    InvalidSelector(String),
    /// At-rules (`@media`, `@import`, ...) are not supported.
    UnsupportedAtRule(String),
    /// Anything else, such as an unexpected token.
    Syntax(String),
}

impl ErrorKind {
    fn from_parse_error_kind(kind: ParseErrorKind<'_, StyleParseErrorKind<'_>>) -> Self {
        match kind {
            ParseErrorKind::Basic(BasicParseErrorKind::AtRuleInvalid(name)) => {
                Self::UnsupportedAtRule(name.to_string())
            }
            ParseErrorKind::Basic(BasicParseErrorKind::QualifiedRuleInvalid) => {
                Self::InvalidSelector("invalid rule".to_owned())
            }
            ParseErrorKind::Basic(basic) => Self::Syntax(describe_basic(&basic)),
            ParseErrorKind::Custom(custom) => match custom {
                StyleParseErrorKind::UnknownProperty(name) => {
                    Self::UnknownProperty(name.to_string())
                }
                StyleParseErrorKind::InvalidValue { property, reason } => Self::InvalidValue {
                    property: property.to_string(),
                    reason,
                },
                StyleParseErrorKind::UnsupportedColorSpace(space) => {
                    Self::Syntax(format!("unsupported colour space '{space}'"))
                }
                StyleParseErrorKind::NegativeValue => {
                    Self::Syntax("negative values are not allowed here".to_owned())
                }
                StyleParseErrorKind::OutOfRange => Self::Syntax("value out of range".to_owned()),
                StyleParseErrorKind::Selector(kind) => {
                    Self::InvalidSelector(describe_selector_error(&kind))
                }
            },
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownProperty(name) => write!(f, "unknown property '{name}'"),
            Self::InvalidValue { property, reason } => {
                write!(f, "invalid value for '{property}': {reason}")
            }
            Self::InvalidSelector(reason) => write!(f, "invalid selector: {reason}"),
            Self::UnsupportedAtRule(name) => write!(f, "unsupported at-rule '@{name}'"),
            Self::Syntax(reason) => f.write_str(reason),
        }
    }
}

/// The custom half of cssparser's `ParseError` while parsing.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum StyleParseErrorKind<'i> {
    UnknownProperty(CowRcStr<'i>),
    InvalidValue {
        property: CowRcStr<'i>,
        reason: String,
    },
    UnsupportedColorSpace(&'static str),
    NegativeValue,
    OutOfRange,
    Selector(SelectorParseErrorKind<'i>),
}

impl<'i> From<SelectorParseErrorKind<'i>> for StyleParseErrorKind<'i> {
    fn from(kind: SelectorParseErrorKind<'i>) -> Self {
        Self::Selector(kind)
    }
}

/// A parse error carrying this crate's custom error kind.
pub(crate) type StyleError<'i> = ParseError<'i, StyleParseErrorKind<'i>>;

/// Wraps whatever went wrong inside a property value as an
/// [`ErrorKind::InvalidValue`] for that property, keeping the location.
pub(crate) fn value_error<'i>(property: CowRcStr<'i>, error: StyleError<'i>) -> StyleError<'i> {
    let reason = ErrorKind::from_parse_error_kind(error.kind).to_string();
    ParseError {
        kind: ParseErrorKind::Custom(StyleParseErrorKind::InvalidValue { property, reason }),
        location: error.location,
    }
}

fn describe_token(token: &Token<'_>) -> String {
    match token {
        Token::WhiteSpace(_) => "whitespace".to_owned(),
        Token::BadString(_) => "unterminated string".to_owned(),
        Token::BadUrl(_) => "bad url".to_owned(),
        token => format!("'{}'", token.to_css_string()),
    }
}

fn describe_basic(kind: &BasicParseErrorKind<'_>) -> String {
    match kind {
        BasicParseErrorKind::UnexpectedToken(token) => {
            format!("unexpected {}", describe_token(token))
        }
        BasicParseErrorKind::EndOfInput => "unexpected end of input".to_owned(),
        BasicParseErrorKind::AtRuleInvalid(name) => format!("unsupported at-rule '@{name}'"),
        BasicParseErrorKind::AtRuleBodyInvalid => "invalid at-rule body".to_owned(),
        BasicParseErrorKind::QualifiedRuleInvalid => "invalid rule".to_owned(),
    }
}

fn describe_selector_error(kind: &SelectorParseErrorKind<'_>) -> String {
    use SelectorParseErrorKind::*;
    match kind {
        EmptySelector => "empty selector".to_owned(),
        DanglingCombinator => "dangling combinator".to_owned(),
        NonCompoundSelector => "expected a compound selector".to_owned(),
        UnsupportedPseudoClassOrElement(name) => {
            format!("unsupported pseudo-class or pseudo-element ':{name}'")
        }
        UnexpectedIdent(name) => format!("unexpected identifier '{name}'"),
        ExpectedNamespace(name) => format!("unknown namespace '{name}'"),
        ClassNeedsIdent(token) => {
            format!("expected a class name, found {}", describe_token(token))
        }
        NoQualifiedNameInAttributeSelector(token) => format!(
            "expected an attribute name, found {}",
            describe_token(token)
        ),
        PseudoElementExpectedColon(token)
        | PseudoElementExpectedIdent(token)
        | NoIdentForPseudo(token) => format!(
            "expected a pseudo-class name, found {}",
            describe_token(token)
        ),
        UnexpectedTokenInAttributeSelector(token)
        | ExpectedBarInAttr(token)
        | BadValueInAttr(token)
        | InvalidQualNameInAttr(token)
        | ExplicitNamespaceUnexpectedToken(token) => {
            format!("unexpected {} in selector", describe_token(token))
        }
        NonPseudoElementAfterSlotted
        | InvalidPseudoElementAfterSlotted
        | InvalidPseudoElementInsideWhere
        | InvalidState => "invalid selector".to_owned(),
    }
}
