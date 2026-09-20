// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Rust bindings for [Google Ink](https://github.com/google/ink), the freehand
//! stroke library behind Android Jetpack Ink.
//!
//! Give it sampled pointer events and a brush; get back smoothed, modelled
//! triangle meshes to draw.
//!
//! # How the bindings are put together
//!
//! Ink's public API is built on `absl::StatusOr`, `absl::Span` and templates,
//! none of which cross an FFI boundary directly. So this crate is two layers:
//!
//! 1. A small C++ facade (`crates/ink-rs/cc/ink_ffi.h`) that restates the parts
//!    of Ink we use in plain structs, enums and pointers, with C linkage.
//! 2. This crate, which wraps the generated declarations in owned types with
//!    `Drop`, turns status codes into [`Result`], and swaps Ink's sentinel
//!    values for [`Option`].
//!
//! The declarations in between are generated from the header by bindgen and
//! checked in, so building needs neither bindgen nor libclang.
//!
//! Adding to the bindings means adding to the facade first. See
//! `docs/ink-bindings.md`.
//!
//! # Threading
//!
//! Ink's types are not thread-safe, and neither are these. [`Brush`],
//! [`Stroke`] and friends are deliberately neither `Send` nor `Sync`: build and
//! read a stroke on one thread, and move the plain [`Mesh`] data if another
//! thread needs it.
//!
//! # Example
//!
//! ```no_run
//! use std::time::Duration;
//!
//! use ink_rs::{Brush, Color, InProgressStroke, Point, StockBrush, StrokeInput, StrokeInputBatch, ToolType};
//!
//! # fn main() -> ink_rs::Result<()> {
//! let family = StockBrush::PressurePen.family();
//! let brush = Brush::new(&family, Color::from_packed_rgba(0x1a1a1aff), 4.0, 0.1)?;
//!
//! let mut stroke = InProgressStroke::new();
//! stroke.start(&brush);
//!
//! // Feed in pointer events as they arrive.
//! let inputs = StrokeInputBatch::new(&[
//!     StrokeInput::new(ToolType::Stylus, Point::new(10.0, 10.0), Duration::ZERO)
//!         .with_pressure(0.4),
//!     StrokeInput::new(ToolType::Stylus, Point::new(22.0, 18.0), Duration::from_millis(16))
//!         .with_pressure(0.7),
//! ])?;
//! stroke.enqueue_inputs(&inputs, None)?;
//! stroke.update_shape(Duration::from_millis(16))?;
//!
//! // Draw it.
//! for coat in 0..stroke.coat_count() {
//!     let mesh = stroke.coat_mesh(coat);
//!     println!("{} triangles", mesh.triangle_count());
//! }
//!
//! // Keep it once the pen lifts.
//! stroke.finish_inputs();
//! stroke.update_shape(Duration::from_millis(32))?;
//! let finished = stroke.to_stroke();
//! # let _ = finished;
//! # Ok(())
//! # }
//! ```

mod brush;
mod color;
mod error;
mod ffi;
mod geometry;
mod input;
mod stroke;

pub use brush::{Brush, BrushFamily, StockBrush};
pub use color::{Color, ColorFormat, ColorSpace};
pub use error::{Error, ErrorKind, Result};
pub use geometry::{Mesh, Point, Rect, Vec2};
pub use input::{StrokeInput, StrokeInputBatch, ToolType};
pub use stroke::{InProgressStroke, Stroke};
