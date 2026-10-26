#!/bin/sh
# The flow of one lab project, sourced by run-lab.sh: build the project from its layers, then walk it the
# way an agent does, through the real hook entry and `familiar-ai claim`: setup, the plan's lines, the
# tests-first commit (stage 1), the fix commit (stage 2). Nothing here changes product code.

# the verdict words: ok · WRONG (a seal or a green that is not true: false security) · FALSE-ALARM (a true
# red or green refused) · CARD (a card asked where none was expected) · DIFF (anything else that differs)
STAGE_SEP='|'

say() { printf '%s\n' "$*"; }
note() { LAB_NOTES="${LAB_NOTES:+$LAB_NOTES; }$*"; }
logf() { printf '%s\n' "$*" >> "$LOGF"; }

git_in() { git -C "$PROJECT" -c user.name=lab -c user.email=lab@example.com -c commit.gpgsign=false "$@"; }

# cli ARGS: familiar-ai in the project, from the agent's shell; the words are left in $OUT, the code in $RC
cli() {
    OUT=$(cd "$PROJECT" && familiar-ai "$@" 2>&1)
    RC=$?
    logf "\$ familiar-ai $* (exit $RC)"
    logf "$OUT"
    logf ""
}

claim() {
    cli claim "$@"
    if [ "$RC" != 0 ]; then
        note "claim $1 refused (exit $RC)"
        return 1
    fi
}

# hook EVENT FIXTURE [key=value ...]: through plugin/hooks/run.sh, as Claude Code calls it
hook() {
    ev=$1
    fx=$2
    shift 2
    python3 -I "$LAB/lab.py" payload "$FIX/$fx.json" "$PROJECT" "$@" > "$WORKD/payload.json" || return 1
    OUT=$(sh "$RUN_SH" "$ev" < "$WORKD/payload.json" 2> "$WORKD/hook.err")
    RC=$?
    logf "hook $ev ($*) -> exit $RC"
    logf "$OUT"
    [ -s "$WORKD/hook.err" ] && logf "(stderr) $(cat "$WORKD/hook.err")"
    logf ""
}

# layer NAME: copy the common layer of the project's kind, then the project's own, over the project
layer() {
    for d in "$LAB/common/$COMMON/$1" "$TPL/$1"; do
        if [ -d "$d" ]; then
            cp -R "$d/." "$PROJECT/" || return 1
        fi
    done
}

# build_project: the folder, a repository, `familiar-ai setup`, the base commit
build_project() {
    mkdir -p "$PROJECT" "$WORKD"
    git_in init -q -b main >> "$LOGF" 2>&1 || { note "git init failed"; return 1; }
    cat > "$PROJECT/.git/info/exclude" <<'EOF'
node_modules
vendor
.venv
target
__pycache__
.pytest_cache
EOF
    layer base || { note "base layer failed"; return 1; }
    cli setup
    SETUP_OUT=$OUT
    [ "$RC" = 0 ] || { note "setup failed: $(printf '%s' "$OUT" | head -n 1)"; return 1; }
    git_in add -A >> "$LOGF" 2>&1
    git_in commit -q -m base >> "$LOGF" 2>&1 || { note "the base commit failed"; return 1; }
}

# plan_the_project: the plan's lines, as the walks claim them; PLACE is "file symbol first-line last-line"
plan_the_project() {
    # shellcheck disable=SC2086
    set -- $PLACE
    claim intent_submitted --for 0 --text "greet with hello" || return 1
    claim behavior --behavior_id b1 --text "the greeting" || return 1
    claim promise --promise_id p1 --behavior_id b1 --accepts "a name" --gives_back "a greeting" --refuses "nothing" || return 1
    claim criterion --criterion_id c1 --text "greet says hello" --path full --behavior_id b1 --promise_id p1 || return 1
    claim place_chosen --criterion_id c1 --places "[{\"file_path\":\"$1\",\"symbol\":\"$2\",\"line_start\":$3,\"line_end\":$4}]" || return 1
    claim test_planned --criterion_id c1 --file_path "$PLANNED_TEST" --will create || return 1
    claim test_stack_detected --codebase_id lab --project_root . --test_stack "$STACK" --test_command "$TEST_COMMAND" || return 1
    claim criterion_head --criterion_id c1 || return 1
    claim criterion_assigned --criterion_id c1 || return 1
    claim fix_location_category --criterion_id c1 --category this_repository --where '' || return 1
}

# a binary that reads an adapter says so when it starts a dry run ("the dry run for ... started") or when the
# test command is claimed ("reading: pytest by ..."): wait for its choice file, then for any run in flight (a dry
# run is a run), so the tests-first commit is read under the adapter. A binary without one never waits.
wait_for_adapter() {
    case "$SETUP_OUT $OUT" in
        *"dry run"* | *"reading:"*) ;;
        *) return 0 ;;
    esac
    waited=0
    while [ "$waited" -lt "$DRY_WAIT" ]; do
        [ -s "$PROJECT/.familiar/adapter" ] && return 0
        sleep 2
        waited=$((waited + 2))
    done
    note "the dry run left no .familiar/adapter in ${DRY_WAIT}s"
}

# status_line TAG: `familiar-ai status --json`, kept; the verdict fields in $ST_STATE $ST_HOLDS $ST_CARDS
status_line() {
    (cd "$PROJECT" && familiar-ai status --json > "$WORKD/status-$1.json" 2>> "$LOGF")
    line=$(python3 -I "$LAB/lab.py" verdict "$WORKD/status-$1.json" c1)
    OLDIFS=$IFS
    IFS=$STAGE_SEP
    # shellcheck disable=SC2086
    set -- $line
    IFS=$OLDIFS
    ST_STATE=$1
    ST_HOLDS=${2:-}
    ST_CARDS=${3:-}
}

# wait_runs: until no run is in flight (the hook waits only its inline window), at most RUN_WAIT seconds
wait_runs() {
    waited=0
    while :; do
        status_line wait
        [ "$ST_STATE" != running ] && return 0
        [ "$waited" -ge "$RUN_WAIT" ] && { note "a run was still going after ${RUN_WAIT}s"; return 0; }
        sleep 2
        waited=$((waited + 2))
    done
}

# stage TAG MESSAGE: commit what the layer wrote, let the hook run the tests, wait for the run, read the state
stage() {
    git_in add -A >> "$LOGF" 2>&1
    git_in commit -q -m "$2" >> "$LOGF" 2>&1 || { note "the commit '$2' failed"; return 1; }
    hook PostToolUse post_tool_use_bash "command=git commit -m '$2'"
    wait_runs
    status_line "$1"
}

# observed_of STAGE: sealed|green · asks (a card is open) · refused (a hold, no card) · none:<state>
observed_of() {
    if [ -n "$ST_CARDS" ]; then
        printf 'asks'
    elif [ "$1" = 1 ] && [ "$ST_STATE" = sealed ]; then
        printf 'sealed'
    elif [ "$1" = 2 ] && [ "$ST_STATE" = green ]; then
        printf 'green'
    elif [ -n "$ST_HOLDS" ]; then
        printf 'refused'
    else
        printf 'none:%s' "$ST_STATE"
    fi
}

# verdict_of EXPECTED OBSERVED STAGE
verdict_of() {
    good=sealed
    [ "$3" = 2 ] && good=green
    case "$1:$2" in
        "$good:$good") printf 'ok' ;;
        "$good:asks") printf 'CARD' ;;
        "$good:refused") printf 'FALSE-ALARM' ;;
        "asks:asks") printf 'ok' ;;
        "asks:$good") printf 'WRONG' ;;
        "not_$good:$good") printf 'WRONG' ;;
        "not_$good:asks" | "not_$good:refused") printf 'ok' ;;
        *) printf 'DIFF' ;;
    esac
}

# card_mark HOLDS WANTED: the cards of one stage, each marked ! when the stage was meant to seal or go green
# (WANTED) and the project did not list the hold in EXPECT_CARDS
card_mark() {
    out=""
    for h in $(printf '%s' "$1" | tr ',' ' '); do
        m=$h
        case "$2" in
            sealed | green)
                case " $EXPECT_CARDS " in
                    *" $h "*) ;;
                    *) m="$h!" ;;
                esac
                ;;
        esac
        out="${out:+$out,}$m"
    done
    printf '%s' "$out"
}

# result FIELDS...: one tab separated line, in lab.py's FIELDS order
result() {
    (
        IFS='	'
        printf '%s\n' "$*"
    ) > "$RESULT_FILE"
}

skip() {
    if [ -n "${TRUTH_MODE:-}" ]; then
        printf 'truth  %-34s skipped: %s\n' "$NAME" "$1"
    else
        result "$NAME" "-" "$EXPECT_READER" "-" "$EXPECT_RED" "-" "skipped" "$EXPECT_GREEN" "-" "skipped" "" "$1"
    fi
}

# lab_begin NAME: the project's folders, log and conf; 2 when there is no such project
lab_begin() {
    NAME=$1
    TPL=$LAB/projects/$NAME
    WORKD=$RUNDIR/$NAME
    LOGF=$WORKD/log.txt
    RESULT_FILE=$WORKD/result.tsv
    LAB_NOTES=""
    mkdir -p "$WORKD"
    : > "$LOGF"
    # the project's own variables (DESC is for --list)
    # shellcheck disable=SC2034
    DESC="" COMMON="" PREPARE="" SHIMS="" PINNED="" ENV_VARS="" STACK="" TEST_COMMAND="" TRUTH_COMMAND="" PLANNED_TEST="" PLACE=""
    EXPECT_RED=sealed EXPECT_GREEN=green EXPECT_READER="" EXPECT_CARDS=""
    if [ ! -f "$TPL/project.conf" ]; then
        say "no such project: $NAME" >&2
        return 2
    fi
    # shellcheck disable=SC1090,SC1091
    . "$TPL/project.conf"
}

# project_env: the project's toolchain, shims and environment, for this shell; 1 after a skip
project_env() {
    PATH_ADD="" NODE_MODULES="" VENDOR=""
    if [ -n "$PREPARE" ]; then
        # shellcheck disable=SC2086
        set -- $PREPARE
        script=$LAB/prepare/$1.sh
        shift
        if [ ! -f "$script" ]; then
            skip "no prepare script $script"
            return 1
        fi
        out=$(sh "$script" "$CACHE" "$@" 2>> "$LOGF")
        rc=$?
        if [ "$rc" = 77 ]; then
            skip "$(printf '%s\n' "$out" | sed -n 's/^skip: //p' | head -n 1)"
            return 1
        fi
        if [ "$rc" != 0 ]; then
            skip "preparing the toolchain failed (see $LOGF)"
            return 1
        fi
        PATH_ADD=$(printf '%s\n' "$out" | sed -n 's/^PATH_PREPEND=//p' | head -n 1)
        NODE_MODULES=$(printf '%s\n' "$out" | sed -n 's/^NODE_MODULES=//p' | head -n 1)
        VENDOR=$(printf '%s\n' "$out" | sed -n 's/^VENDOR=//p' | head -n 1)
    fi

    # shellcheck disable=SC2153
    SHIM_DIR=$WORK/shims/$NAME
    mkdir -p "$SHIM_DIR"
    for pair in $SHIMS; do
        tool=${pair%%=*}
        image=${pair#*=}
        if [ "${ADAPTER_LAB_DOCKER:-1}" != 0 ] && command -v docker > /dev/null 2>&1; then
            if ! docker image inspect "$image" > /dev/null 2>&1 && ! docker pull -q "$image" > /dev/null 2>> "$LOGF"; then
                skip "docker image $image is not available"
                return 1
            fi
            make_shim "$tool" "$image" "$SHIM_DIR"
        elif ! command -v "$tool" > /dev/null 2>&1; then
            skip "needs $tool (installed, or docker with $image)"
            return 1
        else
            case " $PINNED " in
                *" $tool "*)
                    skip "needs $image: the version of $tool is the point, and docker is not used"
                    return 1
                    ;;
            esac
        fi
    done

    PATH="${SHIM_DIR}:${PATH_ADD:+$PATH_ADD:}$PATH"
    export PATH
    for kv in $ENV_VARS; do
        export "${kv?}"
    done
    export GOCACHE="$CACHE/go/cache" GOMODCACHE="$CACHE/go/mod" GOPATH="$CACHE/go/path" GOTOOLCHAIN=local
    export MAVEN_OPTS="-Dmaven.repo.local=$CACHE/m2" MAVEN_CONFIG=/tmp/.m2
    export NUGET_PACKAGES="$CACHE/nuget" DOTNET_CLI_HOME="$CACHE/dotnet-home" DOTNET_NOLOGO=1 DOTNET_CLI_TELEMETRY_OPTOUT=1 DOTNET_SKIP_FIRST_TIME_EXPERIENCE=1
    export COMPOSER_HOME="$CACHE/composer"
}

# keep_evidence: what the binary kept of its runs (the output it read) and its adapter file, beside the log
keep_evidence() {
    [ -d "$PROJECT/.familiar/runs" ] && cp -R "$PROJECT/.familiar/runs" "$WORKD/runs" 2> /dev/null
    [ -f "$PROJECT/.familiar/adapter" ] && cp "$PROJECT/.familiar/adapter" "$WORKD/adapter"
    return 0
}

# lab_truth NAME: the project's own truth, without familiar-ai: the tests-first tree must fail and the fixed tree
# must pass. Only for projects that expect a seal and then a green: the attacks are built to make the exit lie.
lab_truth() {
    lab_begin "$1" || return $?
    if [ "$EXPECT_RED" != sealed ] || [ "$EXPECT_GREEN" != green ]; then
        printf 'truth  %-34s skipped: an attack, the exit code is not its truth\n' "$NAME"
        return 0
    fi
    TRUTH_MODE=1
    project_env || return 0
    PROJECT=$WORK/truth/$NAME
    mkdir -p "$PROJECT"
    layer base
    layer red
    [ -n "$NODE_MODULES" ] && ln -s "$NODE_MODULES" "$PROJECT/node_modules"
    [ -n "$VENDOR" ] && ln -s "$VENDOR" "$PROJECT/vendor"
    (cd "$PROJECT" && sh -c "${TRUTH_COMMAND:-$TEST_COMMAND}") > "$WORKD/truth-red.log" 2>&1 < /dev/null
    rc1=$?
    layer fix
    (cd "$PROJECT" && sh -c "${TRUTH_COMMAND:-$TEST_COMMAND}") > "$WORKD/truth-fix.log" 2>&1 < /dev/null
    rc2=$?
    if [ "$rc1" != 0 ] && [ "$rc2" = 0 ]; then
        ok=ok
    else
        ok=BAD
    fi
    printf '%s\n' "$ok" > "$WORKD/truth.status"
    printf 'truth  %-34s tests first: exit %s (want not 0) · fixed: exit %s (want 0) · %s\n' "$NAME" "$rc1" "$rc2" "$ok"
}

# lab_flow NAME: the whole walk; prints nothing, writes $RESULT_FILE and the project's log
lab_flow() {
    lab_begin "$1" || return $?
    # shellcheck disable=SC2153
    PROJECT=$WORK/projects/$NAME
    project_env || return 0

    if ! build_project; then
        result "$NAME" "-" "$EXPECT_READER" "-" "$EXPECT_RED" "setup failed" "FAIL" "$EXPECT_GREEN" "-" "-" "" "$LAB_NOTES"
        return 0
    fi
    [ -n "$NODE_MODULES" ] && ln -s "$NODE_MODULES" "$PROJECT/node_modules"
    [ -n "$VENDOR" ] && ln -s "$VENDOR" "$PROJECT/vendor"

    if ! plan_the_project; then
        result "$NAME" "-" "$EXPECT_READER" "-" "$EXPECT_RED" "plan refused" "FAIL" "$EXPECT_GREEN" "-" "-" "" "$LAB_NOTES"
        return 0
    fi
    wait_for_adapter
    wait_runs

    # stage 1: the tests-first commit
    layer red || note "red layer failed"
    stage red "c1: tests first"
    obs1=$(observed_of 1)
    ver1=$(verdict_of "$EXPECT_RED" "$obs1" 1)
    cards=$(card_mark "$ST_CARDS" "$EXPECT_RED")
    detail1="$ST_STATE${ST_HOLDS:+ holds=$ST_HOLDS}${ST_CARDS:+ cards=$ST_CARDS}"
    last_tag=red

    # stage 2: the fix commit, only on a seal
    obs2="-"
    ver2="-"
    detail2=""
    if [ "$ST_STATE" = sealed ] && [ "$EXPECT_GREEN" != "-" ]; then
        layer fix || note "fix layer failed"
        stage green "c1: fix"
        obs2=$(observed_of 2)
        ver2=$(verdict_of "$EXPECT_GREEN" "$obs2" 2)
        [ -n "$ST_CARDS" ] && cards="${cards:+$cards,}$(card_mark "$ST_CARDS" "$EXPECT_GREEN")"
        detail2="$ST_STATE${ST_HOLDS:+ holds=$ST_HOLDS}${ST_CARDS:+ cards=$ST_CARDS}"
        last_tag=green
    elif [ "$EXPECT_GREEN" != "-" ]; then
        note "no fix commit: nothing sealed"
        obs2="not run"
    fi

    # the adapter's choice, read from the project after the last stage
    adapter_line=$(python3 -I "$LAB/lab.py" adapter "$PROJECT" "$WORKD/status-$last_tag.json" 2> /dev/null)
    [ -n "$adapter_line" ] || adapter_line="-$STAGE_SEP-"
    OLDIFS=$IFS
    IFS=$STAGE_SEP
    # shellcheck disable=SC2086
    set -- $adapter_line
    IFS=$OLDIFS
    reader=${1:--}
    gaps=${2:--}

    logf "RESULT stage 1: expected $EXPECT_RED, observed $obs1 ($detail1) -> $ver1"
    logf "RESULT stage 2: expected $EXPECT_GREEN, observed $obs2 ${detail2:+($detail2) }-> $ver2"
    logf "RESULT adapter: $reader gaps $gaps (expected: ${EXPECT_READER:-unspecified})"
    keep_evidence
    result "$NAME" "$reader" "$EXPECT_READER" "$gaps" "$EXPECT_RED" "$obs1" "$ver1" "$EXPECT_GREEN" "$obs2" "$ver2" "$cards" "$LAB_NOTES"
}

# make_shim TOOL IMAGE DIR: a stand-in for TOOL that runs it in IMAGE, as this user, in the same folder,
# with this environment, so the project's own command line (`go test ./...`) is the one that is claimed
make_shim() {
    mkdir -p "$3" "$CACHE"
    if [ ! -s "$CACHE/passwd" ]; then
        printf 'root:x:0:0:root:/root:/bin/sh\nnobody:x:65534:65534:nobody:/nonexistent:/bin/false\nlab:x:%s:%s:lab:/tmp:/bin/sh\n' "$(id -u)" "$(id -g)" > "$CACHE/passwd"
        printf 'root:x:0:\nnogroup:x:65534:\nlab:x:%s:\n' "$(id -g)" > "$CACHE/group"
    fi
    cat > "$3/$1" <<EOF
#!/bin/sh
envf=\$(mktemp "$WORK/tmp/shim-env.XXXXXX") || exit 127
env | grep -E '^[A-Za-z_][A-Za-z0-9_]*=' | grep -v -E '^(PATH|HOME|PWD|OLDPWD|SHLVL|_|TMPDIR|HOSTNAME|USER|LOGNAME|SHELL|TERM)=' > "\$envf"
docker run --rm --init -u "\$(id -u):\$(id -g)" --env-file "\$envf" -e HOME=/tmp \\
    -v "$CACHE/passwd:/etc/passwd:ro" -v "$CACHE/group:/etc/group:ro" \\
    -v "$WORK:$WORK" -v "$CACHE:$CACHE" -w "\$PWD" $2 $1 "\$@"
rc=\$?
rm -f "\$envf"
exit \$rc
EOF
    chmod +x "$3/$1"
}
