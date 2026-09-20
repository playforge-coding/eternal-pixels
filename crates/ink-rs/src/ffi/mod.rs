// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The single point of contact with the C++ facade.
//!
//! Everything below this module is safe Rust. `generated` holds the raw
//! declarations, produced from `cc/ink_ffi.h` by `tools/bindgen/generate.sh`
//! and checked in so that neither building nor publishing needs bindgen or
//! libclang. It also carries compile-time assertions on every struct size and
//! field offset, so the Rust view of the facade cannot silently drift from the
//! C++ one.

mod generated;

pub(crate) use generated::root::ink_ffi::*;

/// Longest error message read back from C++. Ink's messages are short; anything
/// past this is truncated rather than allocated for.
const ERROR_BUFFER_LEN: usize = 512;

/// Reads the message belonging to the most recent failing call on this thread.
///
/// Must be called immediately after the failing call: the message lives in
/// thread-local storage on the C++ side and any later call overwrites it.
pub(crate) fn last_error_message() -> String {
    let mut buffer = [0u8; ERROR_BUFFER_LEN];
    let written = unsafe { LastErrorMessage(buffer.as_mut_ptr().cast(), ERROR_BUFFER_LEN as i32) };

    let len = usize::try_from(written).unwrap_or(0).min(ERROR_BUFFER_LEN);
    String::from_utf8_lossy(&buffer[..len]).into_owned()
}

/// Turns a `StatusCode` into the integer it wraps, for matching.
pub(crate) fn status_code_value(code: StatusCode) -> i32 {
    code.0
}
