#!/bin/sh
# The adapter lab: hand-made projects per runner, each in a setup the research found in the wild, walked
# through a given familiar-ai binary the way an agent walks it (setup, the plan, the tests-first commit, the
# fix commit), and read back per project: the adapter's choice and gaps, whether the red sealed honestly,
# whether the green went green, any card nobody expected. See README.md.
#
#   sh scripts/adapter-lab/run-lab.sh --bin <familiar-ai> [options] [project ...]
#
# It changes no product code: the projects are built in a scratch folder, outside the repository.

usage() {
    cat <<'EOF'
usage: run-lab.sh --bin FAMILIAR_AI [options] [project-name-or-glob ...]

  --bin PATH     the familiar-ai binary to walk (else $FAMILIAR_AI_BIN, else target/debug/familiar-ai here)
  --dir DIR      the lab folder: toolchain caches, the projects, the logs
                 (else $ADAPTER_LAB_DIR, else ~/.cache/familiar-ai-adapter-lab)
  --label TEXT   names this run's folder of logs and summary (default: the time); use before / after
  --list         list the projects and what each one is for, then stop
  --verbose      print each project's log as it finishes
  --strict       exit 1 unless every project is ok or skipped
  --truth        check the projects themselves, without familiar-ai: the tests-first tree must fail, the
                 fixed tree must pass (the projects that expect a seal and a green)
  --clean-cache  delete the toolchain caches (venvs, node_modules, docker caches) first

Environment: ADAPTER_LAB_DOCKER=0 (never use docker: projects that need a missing tool are skipped),
ADAPTER_LAB_INLINE_MS (the hook's inline wait for a run, default 120000), ADAPTER_LAB_RUN_WAIT (seconds to
wait for a run, default 900), ADAPTER_LAB_DRY_WAIT (seconds to wait for a dry run's choice, default 900),
ADAPTER_LAB_TEST_WAIT (seconds a test run may take, default 900). The walk runs under `env -i`; only PATH, HOME,
these settings and the proxy, certificate and docker variables are kept.
EOF
}

LAB=$(cd "$(dirname "$0")" && pwd -P)
ROOT=$(cd "$LAB/../.." && pwd -P)

# the walk runs under a clean environment: a lane session's own variables must not reach the hooks. Only these
# names are kept: the search path and home, the lab's own settings, and what the network and docker need.
if [ -z "${ADAPTER_LAB_CLEAN:-}" ]; then
    set -- sh "$0" "$@"
    isset=""
    val=""
    for v in PATH HOME XDG_CACHE_HOME XDG_RUNTIME_DIR DOCKER_HOST DOCKER_CONFIG RUSTUP_HOME CARGO_HOME \
        HTTP_PROXY HTTPS_PROXY NO_PROXY http_proxy https_proxy no_proxy SSL_CERT_FILE SSL_CERT_DIR \
        REQUESTS_CA_BUNDLE NODE_EXTRA_CA_CERTS PIP_INDEX_URL PIP_EXTRA_INDEX_URL NPM_CONFIG_REGISTRY \
        FAMILIAR_AI_BIN ADAPTER_LAB_DIR ADAPTER_LAB_DOCKER ADAPTER_LAB_INLINE_MS ADAPTER_LAB_RUN_WAIT \
        ADAPTER_LAB_DRY_WAIT ADAPTER_LAB_TEST_WAIT ADAPTER_LAB_STAND_IN_REAL; do
        eval "isset=\${$v+yes}"
        if [ -n "$isset" ]; then
            eval "val=\$$v"
            set -- "$v=$val" "$@"
        fi
    done
    exec env -i LANG=C.UTF-8 TERM=dumb ADAPTER_LAB_CLEAN=1 "$@"
fi

BIN="${FAMILIAR_AI_BIN:-}"
DIR="${ADAPTER_LAB_DIR:-}"
LABEL=""
LIST=0
VERBOSE=0
STRICT=0
CLEAN_CACHE=0
TRUTH=0
WANT=""
while [ $# -gt 0 ]; do
    case "$1" in
        --bin) BIN=${2:?--bin needs a path}; shift 2 ;;
        --dir) DIR=${2:?--dir needs a folder}; shift 2 ;;
        --label) LABEL=${2:?--label needs a name}; shift 2 ;;
        --list) LIST=1; shift ;;
        --verbose) VERBOSE=1; shift ;;
        --strict) STRICT=1; shift ;;
        --clean-cache) CLEAN_CACHE=1; shift ;;
        --truth) TRUTH=1; shift ;;
        -h | --help) usage; exit 0 ;;
        -*) printf 'unknown option %s\n' "$1" >&2; usage >&2; exit 2 ;;
        *) WANT="$WANT $1"; shift ;;
    esac
done

# every project's name, and the ones wanted (globs allowed)
ALL=""
for d in "$LAB"/projects/*/; do
    [ -f "${d}project.conf" ] && ALL="$ALL $(basename "$d")"
done
SELECTED=""
if [ -z "$WANT" ]; then
    SELECTED=$ALL
else
    for pat in $WANT; do
        hit=0
        for name in $ALL; do
            # shellcheck disable=SC2254
            case "$name" in
                $pat) SELECTED="$SELECTED $name"; hit=1 ;;
            esac
        done
        [ "$hit" = 1 ] || { printf 'no project matches %s (--list shows them)\n' "$pat" >&2; exit 2; }
    done
fi

if [ "$LIST" = 1 ]; then
    for name in $ALL; do
        DESC="" EXPECT_RED="" EXPECT_GREEN="" EXPECT_READER=""
        # shellcheck disable=SC1090,SC1091
        . "$LAB/projects/$name/project.conf"
        printf '%-34s %s\n' "$name" "$DESC"
        printf '%-34s   want: red %s, green %s, reader %s\n' "" "${EXPECT_RED:-sealed}" "${EXPECT_GREEN:-green}" "${EXPECT_READER:-unspecified}"
    done
    exit 0
fi

if [ -z "$BIN" ] && [ -x "$ROOT/target/debug/familiar-ai" ]; then
    BIN="$ROOT/target/debug/familiar-ai"
fi
if [ -z "$BIN" ] || [ ! -x "$BIN" ]; then
    printf 'name the binary to walk with --bin (a debug build of the tip: cargo build -p familiar-ai)\n' >&2
    exit 2
fi
BIN=$(cd "$(dirname "$BIN")" && pwd -P)/$(basename "$BIN")
if ! command -v python3 > /dev/null 2>&1 || ! command -v git > /dev/null 2>&1; then
    printf 'the lab needs python3 and git\n' >&2
    exit 2
fi

DIR=${DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/familiar-ai-adapter-lab}
mkdir -p "$DIR"
DIR=$(cd "$DIR" && pwd -P)
CACHE=$DIR/cache
WORK=$DIR/work
LABEL=${LABEL:-$(date +%Y%m%d-%H%M%S)}
RUNDIR=$DIR/out/$LABEL
[ "$CLEAN_CACHE" = 1 ] && rm -rf "$CACHE"
rm -rf "$WORK" "$RUNDIR"
mkdir -p "$CACHE" "$WORK/bin" "$WORK/tmp" "$WORK/projects" "$WORK/shims" "$WORK/home" "$RUNDIR"
ln -s "$BIN" "$WORK/bin/familiar-ai"

# the walk's environment, as plugin/tests/test-agent.sh sets it
PATH="$WORK/bin:$PATH"
export PATH
export TMPDIR="$WORK/tmp"
export FAMILIAR_AI_HOME="$WORK/home"
export FAMILIAR_AI_JUDGE=off
export FAMILIAR_AI_INVESTIGATOR=off
export FAMILIAR_AI_RUN_INLINE_MS="${ADAPTER_LAB_INLINE_MS:-120000}"
export FAMILIAR_AI_TEST_RUN_WAIT="${ADAPTER_LAB_TEST_WAIT:-900}"
export CLAUDE_CODE_SESSION_ID=84be6339-0cc7-40ec-91b9-cae44365931b
RUN_WAIT=${ADAPTER_LAB_RUN_WAIT:-900}
DRY_WAIT=${ADAPTER_LAB_DRY_WAIT:-900}
RUN_SH="$ROOT/plugin/hooks/run.sh"
FIX="$ROOT/crates/familiar-ai/tests/fixtures"
export LAB ROOT DIR CACHE WORK RUNDIR RUN_SH FIX RUN_WAIT DRY_WAIT

if [ ! -f "$RUN_SH" ] || [ ! -f "$FIX/post_tool_use_bash.json" ]; then
    printf 'the lab runs from a checkout of familiar-ai (plugin/hooks/run.sh and the hook fixtures are used)\n' >&2
    exit 2
fi

# shellcheck disable=SC1091
. "$LAB/flow.sh"

printf 'adapter lab: %s (%s)\n' "$BIN" "$(familiar-ai --version 2>&1 | head -n 1)"
printf 'tree:        %s at %s\n' "$ROOT" "$(git -C "$ROOT" rev-parse --short HEAD 2> /dev/null || echo unknown)"
printf 'logs:        %s\n\n' "$RUNDIR"

if [ "$TRUTH" = 1 ]; then
    BAD=0
    for name in $SELECTED; do
        ( lab_truth "$name" ) || BAD=$((BAD + 1))
        [ "$(cat "$RUNDIR/$name/truth.status" 2> /dev/null)" = BAD ] && BAD=$((BAD + 1))
    done
    printf '\n%s project(s) whose own truth is wrong\n' "$BAD"
    [ "$BAD" = 0 ] && exit 0
    exit 1
fi

N=0
TOTAL=0
for name in $SELECTED; do
    TOTAL=$((TOTAL + 1))
done
for name in $SELECTED; do
    N=$((N + 1))
    printf '[%s/%s] %s ...\n' "$N" "$TOTAL" "$name" >&2
    START=$(date +%s)
    ( lab_flow "$name" ) || printf '%s\t-\t\t-\t\tflow broke\tFAIL\t\t-\t-\t\tthe flow stopped (see the log)\n' "$name" > "$RUNDIR/$name/result.tsv"
    [ -s "$RUNDIR/$name/result.tsv" ] || printf '%s\t-\t\t-\t\tno result\tFAIL\t\t-\t-\t\tthe flow wrote no result (see the log)\n' "$name" > "$RUNDIR/$name/result.tsv"
    printf '    %s in %ss\n' "$(cut -f6,7,9,10 "$RUNDIR/$name/result.tsv" | tr '\t' ' ')" "$(($(date +%s) - START))" >&2
    [ "$VERBOSE" = 1 ] && cat "$RUNDIR/$name/log.txt" >&2
done

RESULTS=""
for name in $SELECTED; do
    RESULTS="$RESULTS $RUNDIR/$name/result.tsv"
done
printf '\n'
# shellcheck disable=SC2086
python3 -I "$LAB/lab.py" table $RESULTS | tee "$RUNDIR/summary.txt"
# shellcheck disable=SC2086
cat $RESULTS > "$RUNDIR/summary.tsv"
printf '\nlogs per project: %s/<project>/log.txt (every claim and hook, in full)\n' "$RUNDIR"

if [ "$STRICT" = 1 ]; then
    # shellcheck disable=SC2086
    if cut -f7,10 $RESULTS | tr '\t' '\n' | grep -v -x -e ok -e skipped -e '-' -e '' > /dev/null; then
        exit 1
    fi
fi
exit 0
