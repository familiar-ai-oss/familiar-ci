#!/bin/sh
# prepare/tparse.sh CACHE: tparse (the `go test -json | tparse` summariser), built once with the golang image and
# cached. Prints PATH_PREPEND=<its folder>; exits 77 with "skip: <why>" when it cannot be built.
cache=$1
dir=$cache/tparse
if [ ! -x "$dir/bin/tparse" ]; then
    if [ "${ADAPTER_LAB_DOCKER:-1}" = 0 ] || ! command -v docker > /dev/null 2>&1; then
        echo "skip: tparse is built in docker (golang:1.25-bookworm) and docker is not used"
        exit 77
    fi
    mkdir -p "$dir/bin" "$cache/go"
    if ! docker run --rm -u "$(id -u):$(id -g)" -e HOME=/tmp -e GOBIN="$dir/bin" -e GOPATH="$cache/go/path" \
        -e GOCACHE="$cache/go/cache" -e GOMODCACHE="$cache/go/mod" -e CGO_ENABLED=0 \
        -v "$cache:$cache" golang:1.25-bookworm go install github.com/mfridman/tparse@latest >&2; then
        echo "skip: building tparse failed (offline?)"
        exit 77
    fi
fi
echo "PATH_PREPEND=$dir/bin"
