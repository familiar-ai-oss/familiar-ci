#!/bin/sh
# A stand-in for a familiar-ai that has an adapter, to check the lab's adapter column and its wait for the
# dry run without one: it runs the real binary (ADAPTER_LAB_STAND_IN_REAL) and, for `setup`, says a dry run
# started and writes .familiar/adapter a few seconds later, in the plan's shape. It chooses nothing: the
# reader and gap below are fixed, so the lab's verdicts on a stand-in run mean nothing; only its columns do.
#
#   ADAPTER_LAB_STAND_IN_REAL=<familiar-ai> sh run-lab.sh --bin scripts/adapter-lab/stand-in-adapter.sh pytest-plain
real=${ADAPTER_LAB_STAND_IN_REAL:?name the real familiar-ai in ADAPTER_LAB_STAND_IN_REAL}
if [ "$1" = setup ]; then
    "$real" "$@"
    rc=$?
    echo "familiar-ai: the dry run for pytest started (run d-0001); its choice goes in .familiar/adapter"
    (
        sleep 3
        printf '%s\n' '{"root":"","runner":"pytest","reader":"pytest:reporter","setup":"sha256:00","familiar":"0.1.0","gaps":["P5"]}' > .familiar/adapter
    ) > /dev/null 2>&1 &
    exit "$rc"
fi
exec "$real" "$@"
