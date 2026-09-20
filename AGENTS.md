# Eternal Pixels

## Overview

A UI suite and pixel art editor!

## Tests

Always write tests when needed (only when needed though; don't write tests for something as trivial as, say, a hello world).

## Build

Bazel builds everything; there is no Cargo workspace.

```bash
bazel build //...
bazel test //...
```

## Code Quality

Always lint and format:

```bash
rustfmt --edition 2024 $(git ls-files '*.rs')
bazel build --config=clippy //...
```

If a clippy error already existed, you don't need to fix it, but do note it.

## Outdated packages

PLEASE, PLEASE do not use a package that is old or deprecated. When possible use the latest version.

## Bump

Bump the crate versions when needed.

## Wrappers

Prefer to use crates like [`zerocopy`](https://github.com/google/zerocopy) for memory management instead of `unsafe` when applicable.

## Keep things human

Do not use em dashes or other special symbols not normally found in writing. Do not word things in a weird way. Keep it looking human.

## DOCS (IMPORTANT!!!)

Always update the README and docs if necessary (again, not if unnecessary) when changing or adding code. If they already contain outdated information, you don't need to fix it, but do note it.