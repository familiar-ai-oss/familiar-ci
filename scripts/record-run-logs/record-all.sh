#!/bin/sh
# Re-run every fixture: run_logs/<runner>/<outcome>/record.sh
#   ./record-all.sh              all runners
#   ./record-all.sh jest maven   only these runners
# Needs: node 22 (+ .cache/node from npm), python3 (+ .cache/venv with pytest),
# cargo + cargo-nextest, cmake/g++ (+ .cache/cpp-prefix with googletest and
# Catch2 built by .cache/build-cpp.sh), and docker for maven, gradle, dotnet-*,
# go-test, phpunit, pest (images named in each cmd; caches under .cache/).
# Then regenerate MATRIX.md with: python3 -I _bin/matrix.py
cd "$(dirname "$0")" || exit 1
if [ $# -gt 0 ]; then runners="$*"; else runners=$(for d in */; do case $d in _*|.*) ;; *) printf '%s ' "${d%/}";; esac; done); fi
for r in $runners; do
  for f in "$r"/*/record.sh; do
    [ -f "$f" ] && sh "$f"
  done
done
