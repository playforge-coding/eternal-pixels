// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Keyboard shortcuts, written the way they are shown in menus.

use std::fmt;

use crate::event::{Key, Modifiers};

/// A key with modifiers, such as `Ctrl+Shift+S`.
///
/// Parsed from and displayed as text so that shortcuts can live in
/// configuration and in the `shortcut` attribute of an element.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Shortcut {
    pub modifiers: Modifiers,
    pub key: Key,
}

impl Shortcut {
    pub const fn new(modifiers: Modifiers, key: Key) -> Self {
        Self { modifiers, key }
    }

    /// Parses `Ctrl+S`, `Cmd+Shift+Z`, `F5`, `Escape` and the like. Parts are
    /// separated by `+`, in any order, ignoring case. `Ctrl`, `Control`,
    /// `Shift`, `Alt`, `Option`, `Cmd`, `Meta`, `Super` and `Win` are the
    /// modifiers; a single character or a key name is the key.
    pub fn parse(text: &str) -> Result<Self, ShortcutParseError> {
        let mut modifiers = Modifiers::empty();
        let mut key = None;
        for part in text.split('+').map(str::trim) {
            if part.is_empty() {
                return Err(ShortcutParseError(text.to_owned()));
            }
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => modifiers |= Modifiers::CTRL,
                "shift" => modifiers |= Modifiers::SHIFT,
                "alt" | "option" => modifiers |= Modifiers::ALT,
                "cmd" | "meta" | "super" | "win" => modifiers |= Modifiers::META,
                name => {
                    if key
                        .replace(
                            parse_key(name).ok_or_else(|| ShortcutParseError(text.to_owned()))?,
                        )
                        .is_some()
                    {
                        return Err(ShortcutParseError(text.to_owned()));
                    }
                }
            }
        }
        let key = key.ok_or_else(|| ShortcutParseError(text.to_owned()))?;
        Ok(Self { modifiers, key })
    }

    /// Whether a key press with these modifiers triggers the shortcut.
    pub fn matches(&self, key: Key, modifiers: Modifiers) -> bool {
        self.key == key && self.modifiers == modifiers
    }
}

fn parse_key(name: &str) -> Option<Key> {
    let mut chars = name.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return Some(Key::Char(c.to_ascii_lowercase()));
    }
    Some(match name {
        "enter" | "return" => Key::Enter,
        "esc" | "escape" => Key::Escape,
        "tab" => Key::Tab,
        "backspace" => Key::Backspace,
        "delete" | "del" => Key::Delete,
        "space" => Key::Space,
        "left" | "arrowleft" => Key::ArrowLeft,
        "right" | "arrowright" => Key::ArrowRight,
        "up" | "arrowup" => Key::ArrowUp,
        "down" | "arrowdown" => Key::ArrowDown,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        _ => {
            let number: u8 = name.strip_prefix('f')?.parse().ok()?;
            if (1..=24).contains(&number) {
                Key::F(number)
            } else {
                return None;
            }
        }
    })
}

impl fmt::Display for Shortcut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (flag, name) in [
            (Modifiers::CTRL, "Ctrl"),
            (Modifiers::ALT, "Alt"),
            (Modifiers::SHIFT, "Shift"),
            (Modifiers::META, "Cmd"),
        ] {
            if self.modifiers.contains(flag) {
                f.write_str(name)?;
                f.write_str("+")?;
            }
        }
        match self.key {
            Key::Char(c) => write!(f, "{}", c.to_ascii_uppercase()),
            Key::Enter => f.write_str("Enter"),
            Key::Escape => f.write_str("Escape"),
            Key::Tab => f.write_str("Tab"),
            Key::Backspace => f.write_str("Backspace"),
            Key::Delete => f.write_str("Delete"),
            Key::Space => f.write_str("Space"),
            Key::ArrowLeft => f.write_str("Left"),
            Key::ArrowRight => f.write_str("Right"),
            Key::ArrowUp => f.write_str("Up"),
            Key::ArrowDown => f.write_str("Down"),
            Key::Home => f.write_str("Home"),
            Key::End => f.write_str("End"),
            Key::PageUp => f.write_str("PageUp"),
            Key::PageDown => f.write_str("PageDown"),
            Key::F(n) => write!(f, "F{n}"),
        }
    }
}

/// The text was not a shortcut.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShortcutParseError(String);

impl fmt::Display for ShortcutParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "not a shortcut: '{}'", self.0)
    }
}

impl std::error::Error for ShortcutParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_displays() {
        let save = Shortcut::parse("ctrl+s").unwrap();
        assert_eq!(save, Shortcut::new(Modifiers::CTRL, Key::Char('s')));
        assert_eq!(save.to_string(), "Ctrl+S");
        assert_eq!(
            Shortcut::parse("Shift + Cmd + Z").unwrap(),
            Shortcut::new(Modifiers::SHIFT | Modifiers::META, Key::Char('z'))
        );
        assert_eq!(Shortcut::parse("F5").unwrap().key, Key::F(5));
        assert_eq!(Shortcut::parse("Escape").unwrap().key, Key::Escape);
        assert_eq!(
            Shortcut::parse("Alt+Return").unwrap().to_string(),
            "Alt+Enter"
        );
        assert!(Shortcut::parse("Ctrl+").is_err());
        assert!(Shortcut::parse("Ctrl").is_err());
        assert!(Shortcut::parse("A+B").is_err());
        assert!(Shortcut::parse("F25").is_err());
        assert!(Shortcut::parse("").is_err());
    }

    #[test]
    fn matching_is_exact_on_modifiers() {
        let save = Shortcut::parse("Ctrl+S").unwrap();
        assert!(save.matches(Key::Char('s'), Modifiers::CTRL));
        assert!(!save.matches(Key::Char('s'), Modifiers::CTRL | Modifiers::SHIFT));
        assert!(!save.matches(Key::Char('s'), Modifiers::empty()));
    }
}
