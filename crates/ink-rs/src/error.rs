// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Errors coming back out of Ink.

use core::fmt;

use crate::ffi;

/// The kind of failure Ink reported.
///
/// These mirror the `absl::Status` codes Ink actually returns. By far the most
/// common is [`ErrorKind::InvalidArgument`]: it covers a brush size that is not
/// finite and positive, stroke inputs that go backwards in time, and inputs
/// that disagree about which optional fields they carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    /// An argument was outside the range Ink accepts.
    InvalidArgument,
    /// The object was not in a state where the call made sense, such as
    /// updating the shape of a stroke that was never started.
    FailedPrecondition,
    /// An index was past the end.
    OutOfRange,
    /// Something Ink knows about but has not implemented.
    Unimplemented,
    /// A lookup found nothing.
    NotFound,
    /// Ink hit an internal inconsistency.
    Internal,
    /// Anything else, including a status code this crate does not know yet.
    Unknown,
}

impl ErrorKind {
    fn from_status(code: i32) -> Self {
        match code {
            3 => Self::InvalidArgument,
            5 => Self::NotFound,
            9 => Self::FailedPrecondition,
            11 => Self::OutOfRange,
            12 => Self::Unimplemented,
            13 => Self::Internal,
            _ => Self::Unknown,
        }
    }

    fn describe(self) -> &'static str {
        match self {
            Self::InvalidArgument => "invalid argument",
            Self::FailedPrecondition => "failed precondition",
            Self::OutOfRange => "out of range",
            Self::Unimplemented => "unimplemented",
            Self::NotFound => "not found",
            Self::Internal => "internal error",
            Self::Unknown => "error",
        }
    }
}

/// A failure reported by Ink, with the message it came with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    kind: ErrorKind,
    message: String,
}

impl Error {
    pub(crate) fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// What sort of failure this was.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Ink's own description of the failure. May be empty.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.message.is_empty() {
            f.write_str(self.kind.describe())
        } else {
            write!(f, "{}: {}", self.kind.describe(), self.message)
        }
    }
}

impl std::error::Error for Error {}

/// Result type used throughout this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Converts a status returned by the C++ facade into a `Result`.
///
/// Reads the thread-local message the facade recorded, so it has to be called
/// before anything else crosses the boundary on this thread.
pub(crate) fn check(status: ffi::StatusCode) -> Result<()> {
    let code = ffi::status_code_value(status);
    if code == 0 {
        return Ok(());
    }
    Err(Error::new(
        ErrorKind::from_status(code),
        ffi::last_error_message(),
    ))
}
