#!/bin/sh
# prepare/composer.sh CACHE package:constraint ...: a vendor folder with those dev packages, built once in the
# composer:2 image and cached; the lab links it into the project as ./vendor. Prints VENDOR=<folder>.
cache=$1
shift
key=$(printf '%s' "$*" | cksum | cut -d' ' -f1)
dir=$cache/php/$key
if [ ! -f "$dir/.ready" ]; then
    if [ "${ADAPTER_LAB_DOCKER:-1}" = 0 ] || ! command -v docker > /dev/null 2>&1; then
        echo "skip: composer runs in docker (composer:2) and docker is not used"
        exit 77
    fi
    rm -rf "$dir"
    mkdir -p "$dir" "$cache/composer"
    printf '{"name":"lab/deps","require-dev":{}}\n' > "$dir/composer.json"
    if ! docker run --rm -u "$(id -u):$(id -g)" -e HOME=/tmp -e COMPOSER_HOME="$cache/composer" \
        -v "$cache:$cache" -w "$dir" composer:2 composer require --dev --no-interaction --quiet "$@" >&2; then
        rm -rf "$dir"
        echo "skip: composer require $* failed (no composer:2 image, offline?)"
        exit 77
    fi
    printf '%s\n' "$*" > "$dir/.ready"
fi
echo "VENDOR=$dir/vendor"
