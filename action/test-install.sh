#!/usr/bin/env bash
# Local test of install.sh with no network: builds fake release folders for
# THIS machine, points FAMILIAR_AI_RELEASE_BASE at them as file:// URLs, and
# runs six cases. Each refusal must exit 1, install nothing, never create the
# destination folder, and print install.sh's own line for that refusal, so a
# fixture that failed to build cannot pass as a refusal. One summary line at
# the end; exit 1 if any case failed.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# The target for this machine, mirroring install.sh's own mapping.
os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
  Linux)
    case "$arch" in
      x86_64)        target="x86_64-unknown-linux-gnu" ;;
      aarch64|arm64) target="aarch64-unknown-linux-gnu" ;;
      *) echo "unsupported machine for this test: $os/$arch" >&2; exit 1 ;;
    esac
    ;;
  Darwin)
    case "$arch" in
      arm64)  target="aarch64-apple-darwin" ;;
      x86_64) target="x86_64-apple-darwin" ;;
      *) echo "unsupported machine for this test: $os/$arch" >&2; exit 1 ;;
    esac
    ;;
  *) echo "unsupported machine for this test: $os/$arch" >&2; exit 1 ;;
esac

# sha256sum on Linux, shasum -a 256 on macOS; both print "<hash>  <file>".
sum() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1"
  else
    shasum -a 256 "$1"
  fi
}

version="0.0.0-test"
asset="familiar-ai-$target.tar.gz"

# --- Setup: one fake release per case, at <base>/download/v<version>/ ------

# rel <name>: the release folder for one case.
rel() { echo "$work/$1/download/v$version"; }

src="$work/src"
mkdir -p "$src/good" "$src/without"
cat > "$src/good/familiar-ai" <<'EOF'
#!/usr/bin/env bash
echo "fake familiar-ai 0.0.0-test"
EOF
chmod +x "$src/good/familiar-ai"
printf 'not the binary\n' > "$src/without/not-it"
printf 'some other file\n' > "$src/other.bin"

# good: the archive and its right checksum.
mkdir -p "$(rel good)"
tar -czf "$(rel good)/$asset" -C "$src/good" familiar-ai
(cd "$(rel good)" && sum "$asset" > "$asset.sha256")

# corrupt: the good archive with bytes appended after its checksum was taken.
mkdir -p "$(rel corrupt)"
cp "$(rel good)/$asset" "$(rel good)/$asset.sha256" "$(rel corrupt)/"
printf 'corruption' >> "$(rel corrupt)/$asset"

# missing: the release folder exists, the archive does not.
mkdir -p "$(rel missing)"

# other: the good archive with a checksum that is right for a different file.
mkdir -p "$(rel other)"
cp "$(rel good)/$asset" "$(rel other)/"
other_hash="$(sum "$src/other.bin" | cut -d' ' -f1)"
echo "$other_hash  $asset" > "$(rel other)/$asset.sha256"

# without: an archive with no file familiar-ai, and its right checksum.
mkdir -p "$(rel without)"
tar -czf "$(rel without)/$asset" -C "$src/without" not-it
(cd "$(rel without)" && sum "$asset" > "$asset.sha256")

# Every fixture must exist before any case runs.
for f in \
  "$(rel good)/$asset" "$(rel good)/$asset.sha256" \
  "$(rel corrupt)/$asset" "$(rel corrupt)/$asset.sha256" \
  "$(rel other)/$asset" "$(rel other)/$asset.sha256" \
  "$(rel without)/$asset" "$(rel without)/$asset.sha256"; do
  if [ ! -s "$f" ]; then
    echo "setup failed: fixture missing or empty: $f" >&2
    exit 1
  fi
done
if [ ! -d "$(rel missing)" ] || [ -e "$(rel missing)/$asset" ]; then
  echo "setup failed: the missing-asset release is not as built" >&2
  exit 1
fi
if cmp -s "$(rel good)/$asset" "$(rel corrupt)/$asset"; then
  echo "setup failed: the corrupted archive is the same as the good one" >&2
  exit 1
fi
if [ "$other_hash" = "$(sum "$(rel other)/$asset" | cut -d' ' -f1)" ]; then
  echo "setup failed: the other checksum matches the archive" >&2
  exit 1
fi

# --- Cases -----------------------------------------------------------------

passed=0
results=()

# run_install <release> [args...]: runs install.sh, leaving its
# exit code in rc and its stdout and stderr in out and err.
run_install() {
  local base="$1"; shift
  rc=0
  FAMILIAR_AI_RELEASE_BASE="file://$work/$base" \
    bash "$here/install.sh" "$@" >"$work/out" 2>"$work/err" || rc=$?
  out="$(cat "$work/out")"
  err="$(cat "$work/err")"
}

pass() { results+=("PASS $1"); passed=$((passed + 1)); }
fail() { results+=("FAIL $1 (exit $rc; stdout: $out; stderr: $err)"); }

# refused <label> <dest> <line>: exit 1, no destination folder at all, and
# install.sh's own line for that refusal on stderr.
refused() {
  if [ "$rc" -eq 1 ] && [ ! -e "$2" ] && [[ "$err" == *"$3"* ]]; then
    pass "$1"
  else
    fail "$1"
  fi
}

# 1. a good release installs a runnable file.
dest="$work/dest-good"
run_install good "$version" "$dest"
if [ "$rc" -eq 0 ] && [ -x "$dest/familiar-ai" ] \
  && [ "$("$dest/familiar-ai")" = "fake familiar-ai 0.0.0-test" ] \
  && [[ "$out" == *"installed to"* ]]; then
  pass "a good release installs a runnable file"
else
  fail "a good release installs a runnable file"
fi

# 2. a corrupted archive.
dest="$work/dest-corrupt"
run_install corrupt "$version" "$dest"
refused "a corrupted archive is refused, nothing installed" "$dest" "checksum mismatch"

# 3. a missing asset.
dest="$work/dest-missing"
run_install missing "$version" "$dest"
refused "a missing asset is refused, nothing installed" "$dest" "failed to download"

# 4. a checksum file that is right for a different file.
dest="$work/dest-other"
run_install other "$version" "$dest"
refused "a checksum for another file is refused, nothing installed" "$dest" "checksum mismatch"

# 5. an archive without the file familiar-ai.
dest="$work/dest-without"
run_install without "$version" "$dest"
refused "an archive without familiar-ai is refused, nothing installed" "$dest" "archive does not contain the file familiar-ai"

# 6. no arguments.
dest="$work/dest-noargs"
run_install good
refused "no arguments is refused" "$dest" "usage:"

for line in "${results[@]}"; do
  echo "$line"
done
echo "$passed of 6 install.sh cases passed"
[ "$passed" -eq 6 ]
