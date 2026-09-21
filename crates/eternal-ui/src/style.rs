// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Themes and inline styles.

use std::fmt;
use std::path::Path;

use eternal_styler::{Error, Stylesheet};

/// The stylesheet [`Ui::new`](crate::Ui::new) starts with: dark, dense,
/// flat, 1px borders, in the spirit of Aseprite. Read it to see which
/// classes it understands (`panel`, `toolbar`, `primary`, `title`, `dim`).
pub const DEFAULT_THEME_CSS: &str = include_str!("../themes/pixel-dark.css");

/// The default theme, parsed.
pub fn default_theme() -> Stylesheet {
    Stylesheet::parse(DEFAULT_THEME_CSS).expect("the built-in theme parses")
}

/// Reads and parses a stylesheet from a file.
pub fn load_stylesheet(path: impl AsRef<Path>) -> Result<Stylesheet, LoadError> {
    let path = path.as_ref();
    let css = std::fs::read_to_string(path).map_err(|source| LoadError::Io {
        path: path.display().to_string(),
        source,
    })?;
    Stylesheet::parse(&css).map_err(|source| LoadError::Parse {
        path: path.display().to_string(),
        source,
    })
}

/// A stylesheet file could not be read or parsed.
#[derive(Debug)]
pub enum LoadError {
    Io {
        path: String,
        source: std::io::Error,
    },
    Parse {
        path: String,
        source: Error,
    },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "{path}: {source}"),
            Self::Parse { path, source } => write!(f, "{path}: {source}"),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
        }
    }
}

/// Builds inline CSS from Rust values, for `style={...}` attributes whose
/// values are computed.
///
/// ```
/// use eternal_ui::Style;
///
/// let width = 120.0;
/// let style = Style::new().width_px(width).padding_px(4.0).set("color", "#fff");
/// assert_eq!(style.to_string(), "width: 120px; padding: 4px; color: #fff");
/// ```
///
/// The result is parsed by eternal-styler like any other CSS, so a
/// property it does not know is reported when the element is built.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Style {
    declarations: Vec<(String, String)>,
}

impl Style {
    pub fn new() -> Self {
        Self::default()
    }

    /// Any property and value, as CSS text.
    pub fn set(mut self, property: &str, value: impl fmt::Display) -> Self {
        self.declarations
            .push((property.to_owned(), value.to_string()));
        self
    }

    pub fn width_px(self, px: f32) -> Self {
        self.set("width", Px(px))
    }

    pub fn height_px(self, px: f32) -> Self {
        self.set("height", Px(px))
    }

    pub fn min_width_px(self, px: f32) -> Self {
        self.set("min-width", Px(px))
    }

    pub fn min_height_px(self, px: f32) -> Self {
        self.set("min-height", Px(px))
    }

    pub fn padding_px(self, px: f32) -> Self {
        self.set("padding", Px(px))
    }

    pub fn margin_px(self, px: f32) -> Self {
        self.set("margin", Px(px))
    }

    pub fn gap_px(self, px: f32) -> Self {
        self.set("gap", Px(px))
    }

    /// Any CSS colour syntax, such as `#1e1e1e` or `oklch(60% 0.1 250)`.
    pub fn color(self, css: &str) -> Self {
        self.set("color", css)
    }

    pub fn background(self, css: &str) -> Self {
        self.set("background-color", css)
    }

    /// A uniform border: `border(1.0, "solid", "#000")`.
    pub fn border(self, width_px: f32, style: &str, color: &str) -> Self {
        self.set("border", format_args!("{} {style} {color}", Px(width_px)))
    }

    pub fn font_size_px(self, px: f32) -> Self {
        self.set("font-size", Px(px))
    }

    pub fn opacity(self, opacity: f32) -> Self {
        self.set("opacity", opacity)
    }

    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty()
    }
}

impl fmt::Display for Style {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, (property, value)) in self.declarations.iter().enumerate() {
            if index > 0 {
                f.write_str("; ")?;
            }
            write!(f, "{property}: {value}")?;
        }
        Ok(())
    }
}

/// A pixel length that prints without a trailing `.0`.
struct Px(f32);

impl fmt::Display for Px {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.fract() == 0.0 {
            write!(f, "{}px", self.0 as i64)
        } else {
            write!(f, "{}px", self.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_parses() {
        let theme = default_theme();
        assert!(theme.len() > 10);
    }

    #[test]
    fn style_builder_prints_css() {
        assert_eq!(Style::new().to_string(), "");
        assert_eq!(
            Style::new()
                .border(1.0, "solid", "#000")
                .opacity(0.5)
                .width_px(12.5)
                .to_string(),
            "border: 1px solid #000; opacity: 0.5; width: 12.5px"
        );
        assert!(
            eternal_styler::DeclarationBlock::parse(
                &Style::new().padding_px(4.0).background("#123").to_string()
            )
            .is_ok()
        );
    }
}
