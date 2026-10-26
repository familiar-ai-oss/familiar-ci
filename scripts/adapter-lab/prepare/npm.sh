#!/bin/sh
# prepare/npm.sh CACHE package@version ...: node_modules with those packages, built once and cached; the lab links it
# into the project as ./node_modules (the run's copy links the project's untracked top-level entries).
# Prints NODE_MODULES=<folder>; exits 77 with "skip: <why>" when it cannot be built.
cache=$1
shift
key=$(printf '%s' "$*" | cksum | cut -d' ' -f1)
dir=$cache/npm/$key
if [ ! -f "$dir/.ready" ]; then
    command -v npm > /dev/null 2>&1 || { echo "skip: npm is not installed"; exit 77; }
    rm -rf "$dir"
    mkdir -p "$dir"
    printf '{"name":"lab-deps","version":"1.0.0","private":true}\n' > "$dir/package.json"
    if ! (cd "$dir" && npm install --no-audit --no-fund --loglevel=error "$@" >&2); then
        rm -rf "$dir"
        echo "skip: npm install $* failed (offline?)"
        exit 77
    fi
    printf '%s\n' "$*" > "$dir/.ready"
fi
echo "NODE_MODULES=$dir/node_modules"
