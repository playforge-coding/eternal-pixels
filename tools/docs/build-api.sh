#!/usr/bin/env bash
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.
#
# Builds the rustdoc API reference for every crate into one directory, so
# that links between the crates work and GitHub Pages can serve it next to
# the book. Used by .github/workflows/docs.yml and for local previews:
#
#     tools/docs/build-api.sh            # writes docs/book/api
#     tools/docs/build-api.sh some/dir   # writes there instead
#
# Uses Cargo rather than Bazel's rust_doc targets because Cargo puts every
# crate's docs in one tree with working cross-references, and because Ink
# does not have to be built: ink-rs's build.rs skips the native build when
# DOCS_RS is set, which is enough to render its docs.

set -euo pipefail

cd "$(dirname "$0")/../.."
out="${1:-docs/book/api}"

export CARGO_TARGET_DIR="$PWD/target/api-docs"

for crate in eternal-styler eternal-ui eternal-ui-skia; do
    echo "documenting $crate"
    cargo doc --no-deps --quiet --manifest-path "crates/$crate/Cargo.toml"
done

echo "documenting ink-rs"
DOCS_RS=1 cargo doc --no-deps --quiet --manifest-path crates/ink-rs/Cargo.toml

rm -rf "$out"
mkdir -p "$(dirname "$out")"
cp -R "$CARGO_TARGET_DIR/doc" "$out"
cp docs/api/index.html "$out/index.html"

echo "API reference written to $out"
