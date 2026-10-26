#!/usr/bin/env bash
# Fail if any dependency built for a release target compiles or links C.
# A package fails when it declares `links`, is a -sys crate not in
# PURE_RUST_SYS, or is a C build tool; only what `cargo tree` builds counts.
set -euo pipefail
cd "$(dirname "$0")/.."

TARGETS="x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu aarch64-apple-darwin x86_64-apple-darwin"
# -sys crates checked by hand to hold no C and link none; add a name only with
# the reason beside it.
PURE_RUST_SYS='[
  "linux-raw-sys"
]'
# linux-raw-sys: Linux system-call numbers and types written out as Rust, for
#   rustix. No build script, no `links`, no C source in the crate.

status=0
for target in $TARGETS; do
  built=$(cargo tree --locked --workspace --target "$target" --prefix none --format '{p}' \
    | awk '{print $1 " " substr($2, 2)}' | sort -u | jq -R . | jq -s .)
  bad=$(cargo metadata --format-version 1 --locked | jq -r --argjson pure "$PURE_RUST_SYS" --argjson built "$built" '
    ["cc", "cmake", "bindgen", "pkg-config", "cxx-build", "nasm-rs", "vcpkg", "autotools", "meson"] as $tools
    | .packages[]
    | select(.source != null)
    | select("\(.name) \(.version)" as $nv | $built | index($nv))
    | select(
        .links != null
        or ((.name | endswith("-sys")) and (.name as $n | $pure | index($n) | not))
        or (.name as $n | $tools | index($n))
      )
    | "\(.name) \(.version)"')
  if [ -n "$bad" ]; then
    echo "$target: dependencies that compile or link C (not allowed):" >&2
    echo "$bad" | sed 's/^/  /' >&2
    status=1
  fi
  [ "$(echo "$built" | jq length)" -gt 0 ] || { echo "$target: could not list what is built" >&2; status=1; }
done
[ "$status" = 0 ] || exit 1
echo "No dependency compiles C (checked for: $TARGETS)."
