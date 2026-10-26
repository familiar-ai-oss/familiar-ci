#!/usr/bin/env bash
# Install a checksum-verified familiar-ai release.
# Usage: install.sh <version> <dest-dir>
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "usage: $0 <version> <dest-dir>" >&2
  exit 1
fi
version="$1"
dest_dir="$2"

os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
  Linux)
    case "$arch" in
      x86_64)          target="x86_64-unknown-linux-gnu" ;;
      aarch64|arm64)   target="aarch64-unknown-linux-gnu" ;;
      *) echo "unsupported architecture for Linux: $arch" >&2; exit 1 ;;
    esac
    ;;
  Darwin)
    case "$arch" in
      arm64)           target="aarch64-apple-darwin" ;;
      x86_64)          target="x86_64-apple-darwin" ;;
      *) echo "unsupported architecture for macOS: $arch" >&2; exit 1 ;;
    esac
    ;;
  *) echo "unsupported operating system: $os" >&2; exit 1 ;;
esac

base="${FAMILIAR_AI_RELEASE_BASE:-https://github.com/familiar-ai-oss/familiar-ci/releases}"
base="${base%/}"
url="$base/download/v$version"
asset="familiar-ai-$target.tar.gz"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

if ! curl -fsSL "$url/$asset" -o "$work/$asset"; then
  echo "failed to download $url/$asset" >&2
  exit 1
fi
if ! curl -fsSL "$url/$asset.sha256" -o "$work/$asset.sha256"; then
  echo "failed to download $url/$asset.sha256" >&2
  exit 1
fi

# macOS provides shasum instead of sha256sum.
if [ "$os" = "Darwin" ]; then
  check_ok="$(cd "$work" && shasum -a 256 -c "$asset.sha256" >/dev/null 2>&1 && echo yes || echo no)"
else
  check_ok="$(cd "$work" && sha256sum -c "$asset.sha256" >/dev/null 2>&1 && echo yes || echo no)"
fi
if [ "$check_ok" != "yes" ]; then
  echo "checksum mismatch for $asset; refusing to install" >&2
  exit 1
fi

mkdir -p "$work/unpack"
if ! tar -xzf "$work/$asset" -C "$work/unpack"; then
  echo "failed to unpack $asset" >&2
  exit 1
fi
if [ ! -f "$work/unpack/familiar-ai" ]; then
  echo "archive does not contain the file familiar-ai" >&2
  exit 1
fi
mkdir -p "$dest_dir"
install -m 0755 "$work/unpack/familiar-ai" "$dest_dir/familiar-ai"

echo "familiar-ai $version ($target) installed to $dest_dir/familiar-ai"
