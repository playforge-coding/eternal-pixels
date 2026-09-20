#!/usr/bin/env bash
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.
#
# Regenerates the Rust declarations for the C++ facade from ink_ffi.h.
#
# The output is checked in, so neither Bazel nor a `cargo build` needs bindgen
# or libclang. Re-run this whenever crates/ink-rs/cc/ink_ffi.h changes. The
# layout checks bindgen emits are compile-time assertions on every struct size
# and field offset, so a header change that is not reflected here fails to
# build rather than corrupting memory at run time.
#
# Requires bindgen: cargo install bindgen-cli

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
header="${repo_root}/crates/ink-rs/cc/ink_ffi.h"
output="${repo_root}/crates/ink-rs/src/ffi/generated.rs"

command -v bindgen >/dev/null || {
  echo "error: bindgen not found. Run 'cargo install bindgen-cli'." >&2
  exit 1
}

mkdir -p "$(dirname "${output}")"

bindgen "${header}" \
  --output "${output}" \
  --rust-target 1.85 \
  --enable-cxx-namespaces \
  --default-enum-style newtype \
  --allowlist-file '.*ink_ffi\.h' \
  --no-doc-comments \
  --raw-line '// This Source Code Form is subject to the terms of the Mozilla Public' \
  --raw-line '// License, v. 2.0. If a copy of the MPL was not distributed with this' \
  --raw-line '// file, You can obtain one at https://mozilla.org/MPL/2.0/.' \
  --raw-line '' \
  --raw-line '//! Generated from crates/ink-rs/cc/ink_ffi.h by tools/bindgen/generate.sh.' \
  --raw-line '//! Do not edit by hand. The compile-time assertions below pin every struct' \
  --raw-line '//! size and field offset to what the C++ compiler produced, so a header' \
  --raw-line '//! change that is not reflected here fails to build.' \
  --raw-line '' \
  --raw-line '#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals)]' \
  --raw-line '#![allow(dead_code)]' \
  -- \
  -x c++ -std=c++20 \
  $(if [[ "$(uname -s)" == "Darwin" ]]; then echo "-isysroot $(xcrun --show-sdk-path)"; fi)

echo "wrote ${output}"
echo "  types:     $(grep -c "pub struct " "${output}" || true)"
echo "  functions: $(grep -c 'pub fn ' "${output}" || true)"
