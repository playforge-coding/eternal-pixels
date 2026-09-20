#!/usr/bin/env bash
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.
#
# Produces an INK_ROOT prefix that a Cargo build of ink-rs can link against.
#
# Ink builds under Bazel as a couple of hundred separate static archives, and
# there is no install step and no single libink.a. The Cargo build cannot
# reasonably chase all of those, so this merges everything Ink and Abseil
# produce into one archive and lays the headers out beside it:
#
#     <out>/include/ink/...      Ink headers
#     <out>/include/absl/...     Abseil headers
#     <out>/lib/libink.a         everything, merged
#
# Then:
#
#     INK_ROOT=<out> cargo build
#
# This is for developing and testing the Cargo path. Someone consuming ink-rs
# from crates.io does the same thing with their own Ink build.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
out_dir="${1:-${repo_root}/.ink-prefix}"

die() { printf '\nerror: %s\n' "$*" >&2; exit 1; }
note() { printf '\n==> %s\n' "$*"; }

command -v bazel >/dev/null || die "bazel is required to build Ink."

note "Building the facade so every Ink and Abseil archive exists"
(cd "${repo_root}" && bazel build //crates/ink-rs/cc:ink_ffi >/dev/null)

bazel_bin="$(cd "${repo_root}" && bazel info bazel-bin)"
external_bin="$(dirname "${bazel_bin}")"

mkdir -p "${out_dir}/lib" "${out_dir}/include"

#
# Headers. Taken from the fetched source repositories rather than the build
# outputs, since that is where the .h files actually live.
#

output_base="$(cd "${repo_root}" && bazel info output_base)"
ink_src="${output_base}/external/ink+"
absl_src="${output_base}/external/abseil-cpp+"

[[ -d "${ink_src}/ink" ]] || die "No Ink headers at '${ink_src}/ink'."
[[ -d "${absl_src}/absl" ]] || die "No Abseil headers at '${absl_src}/absl'."

note "Copying headers"
rm -rf "${out_dir}/include/ink" "${out_dir}/include/absl"

# Headers only: the source trees also carry .cc files and test data. `.inc` is
# included because Abseil's int128.h and friends include .inc files directly.
# Done with a plain loop rather than `install -D`, which is a GNU extension
# that BSD install (and so macOS) does not have.
copy_headers() {
  local src_root="$1" subdir="$2" rel
  (
    cd "${src_root}"
    find "${subdir}" \( -name '*.h' -o -name '*.inc' \) -type f -print
  ) | while IFS= read -r rel; do
    mkdir -p "${out_dir}/include/$(dirname "${rel}")"
    cp "${src_root}/${rel}" "${out_dir}/include/${rel}"
  done
}

copy_headers "${ink_src}" ink
copy_headers "${absl_src}" absl

#
# Archives
#

note "Collecting static archives"
# Read into an array without mapfile, which macOS's bash 3.2 does not have.
archives=()
while IFS= read -r archive; do
  archives+=("${archive}")
done < <(
  find "${external_bin}" \
    \( -path '*/external/ink+/*' -o -path '*/external/abseil-cpp+/*' \) \
    -name '*.a' -type f 2>/dev/null | sort -u
)

[[ ${#archives[@]} -gt 0 ]] || die "Found no Ink or Abseil archives under '${external_bin}'."
note "Merging ${#archives[@]} archives into libink.a"

merged="${out_dir}/lib/libink.a"
rm -f "${merged}"

if [[ "$(uname -s)" == "Darwin" ]]; then
  # libtool merges archives directly and keeps the index right. It warns about
  # members with no symbols, which is normal for header-only targets.
  libtool -static -o "${merged}" "${archives[@]}" 2>/dev/null
else
  # GNU ar via an MRI script, which is the portable way to concatenate
  # archives without unpacking them by hand.
  {
    echo "create ${merged}"
    for archive in "${archives[@]}"; do echo "addlib ${archive}"; done
    echo "save"
    echo "end"
  } | ar -M
  ranlib "${merged}"
fi

note "Done"
printf '  headers: %s/include\n' "${out_dir}"
printf '  library: %s (%s)\n' "${merged}" "$(du -h "${merged}" | cut -f1 | tr -d ' ')"
printf '\nBuild the crate with:\n\n    INK_ROOT=%s cargo build --manifest-path crates/ink-rs/Cargo.toml\n\n' "${out_dir}"
