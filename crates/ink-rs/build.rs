// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Compiles the C++ facade and links it against Ink.
//!
//! Development happens under Bazel, which builds Ink from source and compiles
//! the facade itself; this script exists so the crate can also be built, and
//! published, with Cargo. It does not run bindgen: `src/ffi/generated.rs` is
//! checked in, so no consumer needs libclang. Regenerate it with
//! `tools/bindgen/generate.sh` after changing the header.
//!
//! Ink has no standard installed location and is not distributed as a library,
//! so it has to be pointed at:
//!
//!   INK_INCLUDE_DIR   Ink's headers, the directory holding `ink/`.
//!   INK_LIB_DIR       Directory holding the compiled Ink libraries.
//!   ABSL_INCLUDE_DIR  Abseil headers. Defaults to INK_INCLUDE_DIR.
//!   ABSL_LIB_DIR      Abseil libraries. Defaults to INK_LIB_DIR.
//!
//! `INK_ROOT` sets all four at once from `<root>/include` and `<root>/lib`.

use std::env;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo::rerun-if-changed=cc/ink_ffi.h");
    println!("cargo::rerun-if-changed=cc/ink_ffi.cc");
    for var in [
        "INK_ROOT",
        "INK_INCLUDE_DIR",
        "INK_LIB_DIR",
        "ABSL_INCLUDE_DIR",
        "ABSL_LIB_DIR",
    ] {
        println!("cargo::rerun-if-env-changed={var}");
    }

    // docs.rs builds documentation, not a working binary, and has no Ink to
    // link against. Rendering the docs only needs the crate to compile.
    if env::var_os("DOCS_RS").is_some() {
        println!("cargo::warning=DOCS_RS set: skipping the native build.");
        return;
    }

    let root = env::var_os("INK_ROOT").map(PathBuf::from);
    let include_dir = dir_from_env("INK_INCLUDE_DIR", root.as_deref(), "include");
    let lib_dir = dir_from_env("INK_LIB_DIR", root.as_deref(), "lib");

    let (include_dir, lib_dir) = match (include_dir, lib_dir) {
        (Some(include_dir), Some(lib_dir)) => (include_dir, lib_dir),
        _ => {
            panic!(
                "\n\nink-rs needs a built copy of Google Ink to link against, and there is no \
                 standard place to find one.\n\n\
                 Set INK_ROOT to a prefix containing include/ and lib/, or set INK_INCLUDE_DIR \
                 and INK_LIB_DIR separately.\n\n\
                 Ink is at https://github.com/google/ink and builds with Bazel. This crate is \
                 developed against commit\n\
                 dd67b9bb6524d002d5a43e1fe9286066a810af1e.\n\n"
            )
        }
    };

    let absl_include_dir = dir_from_env("ABSL_INCLUDE_DIR", root.as_deref(), "include")
        .unwrap_or_else(|| include_dir.clone());
    let absl_lib_dir =
        dir_from_env("ABSL_LIB_DIR", root.as_deref(), "lib").unwrap_or_else(|| lib_dir.clone());

    cc::Build::new()
        .cpp(true)
        .std("c++20")
        .file("cc/ink_ffi.cc")
        .include(&include_dir)
        .include(&absl_include_dir)
        .compile("ink_ffi");

    println!("cargo::rustc-link-search=native={}", lib_dir.display());
    if absl_lib_dir != lib_dir {
        println!("cargo::rustc-link-search=native={}", absl_lib_dir.display());
    }

    // Ink and Abseil are C++, so the standard library has to come along. Which
    // one depends on the platform.
    if cfg!(target_os = "macos") {
        println!("cargo::rustc-link-lib=c++");
    } else if cfg!(target_os = "linux") {
        println!("cargo::rustc-link-lib=stdc++");
    }

    println!("cargo::rustc-link-lib=static=ink");
}

/// Resolves a directory from `var`, falling back to `<root>/<suffix>`.
///
/// Returns `None` rather than a path that does not exist, so a stale
/// environment variable produces the explanatory panic above instead of an
/// obscure compiler error later.
fn dir_from_env(var: &str, root: Option<&Path>, suffix: &str) -> Option<PathBuf> {
    let candidate = match env::var_os(var) {
        Some(value) => PathBuf::from(value),
        None => root?.join(suffix),
    };
    candidate.is_dir().then_some(candidate)
}
