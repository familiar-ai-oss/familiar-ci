#!/bin/sh
# prepare/pytest.sh CACHE [package ...]: a venv with pytest (pinned) and the named plugins, built once and cached.
# Prints PATH_PREPEND=<the venv's bin>; exits 77 with "skip: <why>" when it cannot be built (no network, no venv).
cache=$1
shift
pkgs="pytest==9.1.1 $*"
key=$(printf '%s' "$pkgs" | cksum | cut -d' ' -f1)
venv=$cache/venv/$key
if [ ! -f "$venv/.ready" ]; then
    rm -rf "$venv"
    if ! python3 -m venv "$venv" >&2; then
        echo "skip: python3 -m venv failed"
        exit 77
    fi
    # shellcheck disable=SC2086
    if ! "$venv/bin/python" -m pip install -q --disable-pip-version-check $pkgs >&2; then
        rm -rf "$venv"
        echo "skip: pip install $pkgs failed (offline?)"
        exit 77
    fi
    printf '%s\n' "$pkgs" > "$venv/.ready"
fi
echo "PATH_PREPEND=$venv/bin"
