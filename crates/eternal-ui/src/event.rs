// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Input events, as the windowing layer hands them to the toolkit.

use bitflags::bitflags;

use crate::geometry::Point;

bitflags! {
    /// Modifier keys held during a key or pointer event.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct Modifiers: u8 {
        const CTRL = 1 << 0;
        const SHIFT = 1 << 1;
        const ALT = 1 << 2;
        /// Command on macOS, the Windows key elsewhere.
        const META = 1 << 3;
    }
}

/// A key, independent of keyboard layout for the named ones and by produced
/// character for the rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    /// A key that types a character. Letters are lower case, so `Ctrl+S`
    /// and `Ctrl+Shift+S` both carry `Char('s')`.
    Char(char),
    Enter,
    Escape,
    Tab,
    Backspace,
    Delete,
    Space,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Home,
    End,
    PageUp,
    PageDown,
    /// A function key, `F(1)` through `F(24)`.
    F(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
}

/// Something the user did.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    PointerMove {
        position: Point,
    },
    PointerDown {
        position: Point,
        button: PointerButton,
    },
    PointerUp {
        position: Point,
        button: PointerButton,
    },
    /// The pointer left the window.
    PointerLeave,
    Wheel {
        position: Point,
        /// Scroll distance in pixels, positive down and right.
        delta: Point,
    },
    KeyDown {
        key: Key,
        modifiers: Modifiers,
    },
    KeyUp {
        key: Key,
        modifiers: Modifiers,
    },
    /// Text produced by a key press, after the platform's input method has
    /// had its say. This is what text fields insert; `KeyDown` is for
    /// shortcuts and navigation.
    Text(String),
}

impl Event {
    /// The pointer position for pointer events.
    pub fn position(&self) -> Option<Point> {
        match self {
            Self::PointerMove { position }
            | Self::PointerDown { position, .. }
            | Self::PointerUp { position, .. }
            | Self::Wheel { position, .. } => Some(*position),
            _ => None,
        }
    }
}
