#!/bin/sh
# Tests `familiar-ai verify` in fresh clones, as CI runs it, without the local record.
# Covers a valid fix, a reverted fix and insufficient history.
# Run after `cargo build -p familiar-ai`, from the repository root: sh plugin/tests/test-ci.sh (VERBOSE=1 for detail).

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
RUN_SH="$ROOT/plugin/hooks/run.sh"
FIX="$ROOT/crates/familiar-ai/tests/fixtures"
SESSION=84be6339-0cc7-40ec-91b9-cae44365931b

# The binary under test: FAMILIAR_AI_BIN, else familiar-ai on PATH, else the debug build.
BIN=${FAMILIAR_AI_BIN:-$(command -v familiar-ai)}
if [ -z "$BIN" ]; then
    BIN="$ROOT/target/debug/familiar-ai"
fi
printf 'familiar-ai: %s\n' "$BIN"

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/bin"
ln -s "$BIN" "$TMP/bin/familiar-ai"
export PATH="$TMP/bin:$PATH"
unset CLAUDE_PROJECT_DIR
# a headless runner (claude -p) is not the walk's session: it would count as the bench
unset CLAUDE_CODE_ENTRYPOINT
export CLAUDE_CODE_SESSION_ID="$SESSION"
export FAMILIAR_AI_HOME="$TMP/home"
export FAMILIAR_AI_JUDGE=off
export FAMILIAR_AI_INVESTIGATOR=off

VERBOSE=${VERBOSE:-0}
FAILS=0           # failed checks and stopped scenarios in the whole walk
TOTAL_CHECKS=0    # checks run in the whole walk
EXPECTED_CHECKS=0 # checks the scenarios that ran were expected to make
CHECKS=0          # checks in the running scenario
SCENARIO_FAILS=0  # failed checks in the running scenario
SCENARIO=""
STOPPED=""        # the setup step that stopped the running scenario
OUT=""            # what the last step printed
LAST_INPUT=""     # what the last step was given
LAST_ERR=""       # what the last hook wrote to stderr
RC=0              # the last command's exit code
LAPTOP=""         # the repository the laptop scenario left green
EDITABLE=""       # the repository the editable install scenario left green

verbose() { [ "$VERBOSE" = 1 ]; }

pass() {
    CHECKS=$((CHECKS + 1))
    if verbose; then
        printf 'PASS %s\n' "$1"
    fi
}

# fail <label> <what was expected>: prints the last step's input and output
fail() {
    CHECKS=$((CHECKS + 1))
    FAILS=$((FAILS + 1))
    SCENARIO_FAILS=$((SCENARIO_FAILS + 1))
    printf 'FAIL %s: %s\n' "$SCENARIO" "$1"
    printf '  input:    %s\n' "$LAST_INPUT"
    printf '  output:   %s\n' "$OUT"
    if [ -n "$LAST_ERR" ]; then
        printf '  stderr:   %s\n' "$LAST_ERR"
    fi
    printf '  expected: %s\n' "$2"
}

# check <label> <expected> <command ...>: a check the command decides
check() {
    label=$1
    expected=$2
    shift 2
    if "$@"; then
        pass "$label"
    else
        fail "$label" "$expected"
    fi
}

# expect <label> <text that must be in $OUT>
expect() {
    case "$OUT" in
        *"$2"*) pass "$1" ;;
        *) fail "$1" "to contain: $2" ;;
    esac
}

# exits <label> <code>: the last command's exit code
exits() {
    if [ "$RC" = "$2" ]; then
        pass "$1"
    else
        fail "$1" "exit $2, got exit $RC"
    fi
}

# field_has <label> <key> <text>: the hook's JSON (the last line of $OUT)
# holds the key, at the top level or in hookSpecificOutput, and its value,
# as the JSON writes it, contains the text
field_has() {
    value=$(printf '%s\n' "$OUT" | python3 -c '
import json, sys
lines = [line for line in sys.stdin.read().splitlines() if line.strip()]
try:
    d = json.loads(lines[-1])
except (IndexError, ValueError):
    d = None
if not isinstance(d, dict):
    sys.stdout.write("<not JSON>")
    sys.exit()
key = sys.argv[1]
inner = d.get("hookSpecificOutput")
if key in d:
    value = d[key]
elif isinstance(inner, dict) and key in inner:
    value = inner[key]
else:
    sys.stdout.write("<absent>")
    sys.exit()
text = json.dumps(value, ensure_ascii=False)
sys.stdout.write(text[1:-1] if isinstance(value, str) else text)
' "$2")
    case "$value" in
        *"$3"*) pass "$1" ;;
        *) fail "$1" "$2 containing: $3" ;;
    esac
}

# need <what> <command ...>: a setup step; when it fails, the scenario stops
need() {
    what=$1
    shift
    OUT=""
    if "$@" >"$TMP/need" 2>&1; then
        return 0
    fi
    STOPPED="$what failed"
    said="$(cat "$TMP/need")$OUT"
    if [ -n "$said" ]; then
        STOPPED="$STOPPED: $said"
    fi
    return 1
}

# A payload from a captured fixture, re-pointed at the project, with
# tool_input fields overridden (@key: a top-level one): payload <fixture> [key=value ...]
payload() {
    python3 - "$FIX/$1.json" "$PROJECT" "$@" <<'EOF'
import json, sys
d = json.load(open(sys.argv[1]))
d["cwd"] = sys.argv[2]
for kv in sys.argv[4:]:
    k, v = kv.split("=", 1)
    if k.startswith("@"):
        d[k[1:]] = v
    else:
        d.setdefault("tool_input", {})[k] = v
print(json.dumps(d))
EOF
}

# hook <label> <Event> <fixture> [key=value ...]: through run.sh, as Claude
# Code calls it; the JSON emitted is kept in $OUT.
hook() {
    label=$1
    event=$2
    shift 2
    LAST_INPUT="hook $event ($label) $(payload "$@")"
    OUT=$(payload "$@" | /bin/sh "$RUN_SH" "$event" 2>"$TMP/err")
    RC=$?
    LAST_ERR=$(cat "$TMP/err")
    if verbose; then
        printf '\n### %s: hook %s, exit %s\n%s\n' "$label" "$event" "$RC" "$OUT"
        if [ -n "$LAST_ERR" ]; then
            printf '(stderr) %s\n' "$LAST_ERR"
        fi
    fi
}

# cli <args>: `familiar-ai <args>` in the project
cli() {
    LAST_INPUT="familiar-ai $*"
    LAST_ERR=""
    OUT=$(cd "$PROJECT" && familiar-ai "$@" 2>&1)
    RC=$?
    if verbose; then
        printf '\n$ familiar-ai %s, exit %s\n%s\n' "$*" "$RC" "$OUT"
    fi
    return "$RC"
}

git_in() {
    git -C "$PROJECT" -c user.name=t -c user.email=t@example.com -c commit.gpgsign=false "$@"
}

# hook_with <label> <Event> <json>: the JSON itself through run.sh, as Claude Code pipes it
hook_with() {
    label=$1; event=$2
    LAST_INPUT="hook $event ($label) $3"
    OUT=$(printf '%s' "$3" | /bin/sh "$RUN_SH" "$event" 2>"$TMP/err")
    RC=$?
    LAST_ERR=$(cat "$TMP/err")
    if verbose; then
        printf '\n### %s: hook %s, exit %s\n%s\n' "$label" "$event" "$RC" "$OUT"
        if [ -n "$LAST_ERR" ]; then printf '(stderr) %s\n' "$LAST_ERR"; fi
    fi
}

# lens_answer <the spawn's PreToolUse reply> <session> <tool_use_id> <answer>: the PostToolUse on
# the Agent call the hook released to a lens of the lens panel, or its synthesis [draft names] (the
# owner's plan of 2026-10-26; in the shape of the spike's capture of 2026-10-25): the spawn as the
# hook rewrote it, and the subagent's final text, the fenced JSON <answer>
lens_answer() {
    python3 - "$1" "$PROJECT" "$2" "$3" "$4" <<'EOF'
import json, sys
reply = json.loads(sys.argv[1] or "{}")
spawn = reply.get("hookSpecificOutput", {}).get("updatedInput") or {}
print(json.dumps({
    "session_id": sys.argv[3], "cwd": sys.argv[2], "hook_event_name": "PostToolUse", "permission_mode": "bypassPermissions",
    "tool_name": "Agent", "tool_input": spawn, "tool_use_id": sys.argv[4],
    "tool_response": {"status": "completed", "prompt": spawn.get("prompt", ""), "agentId": "a0ca5ef1d0e4walk", "agentType": spawn.get("subagent_type", ""),
                      "content": [{"type": "text", "text": "```json\n" + sys.argv[5] + "\n```"}],
                      "resolvedModel": "claude-opus-4-5", "totalDurationMs": 1000, "totalTokens": 1234, "totalToolUseCount": 3}}))
EOF
}

# cases_kept <criterion_id>: the cases card passed [draft names] (plan cases-card, 2026-10-25),
# once the part's contract is whole; updated with the definition (the owner's plan of 2026-10-26,
# the lens panel): the agent releases six Agent calls, each with its own tool_use_id, and the
# PreToolUse of each rewrites it (subagent_type familiar-ai:lens, the binary's brief for the lens
# that call was released as, the model opus unless FAMILIAR_AI_LENS_MODEL names another); the five
# lenses answer with no finding and the synthesis that the planned cases cover it, which the binary
# reads on the PostToolUse of each call and records as lens_found and cases_found; the person keeps
# the planned cases from a terminal of their own; the agent's next tool call is told so.
cases_kept() {
    for n in 1 2 3 4 5 6; do
        answer='{"findings": []}'
        [ "$n" = 6 ] && answer='{"cases": [], "none_because": "the planned cases cover what the person asked"}'
        hook "release call $n of the lens panel for $1" PreToolUse pre_tool_use_agent "@session_id=$SESSION" "@tool_use_id=toolu_lens_walk_$1_$n"
        case "$OUT" in *'"subagent_type":"familiar-ai:lens"'*) ;; *) { STOPPED="the lens panel's call $n for $1: $OUT"; return 1; } ;; esac
        hook_with "call $n of the lens panel answered for $1" PostToolUse "$(lens_answer "$OUT" "$SESSION" "toolu_lens_walk_$1_$n" "$answer")"
    done
    cli status --json
    case "$OUT" in *'"cases_unconfirmed"'*) ;; *) { STOPPED="the cases card for $1: $OUT"; return 1; } ;; esac
    LAST_INPUT="(the person's terminal) familiar-ai claim hold_answered"
    OUT=$(cd "$PROJECT" && env -u CLAUDE_CODE_SESSION_ID familiar-ai claim hold_answered --criterion_id "$1" --hold cases_unconfirmed --choice keep_planned --typed '' 2>&1) || { STOPPED="the person keeps the planned cases for $1: $OUT"; return 1; }
    hook "the next tool call after the cases card of $1" PostToolUse post_tool_use_bash "command=ls"
    case "$OUT" in *"the person answered the question for"*) ;; *) { STOPPED="the agent told the person's answer for $1: $OUT"; return 1; } ;; esac
}

# verify_in_clone <source> <clone folder> [git clone args]: verify in a fresh clone.
# A fresh clone has no .familiar/record, so verify must prove the work from the plan committed at HEAD alone.
# CLONE_SETUP, when set, names a function run on the clone first, as CI's own setup step.
verify_in_clone() {
    clone="$TMP/$2"
    source=$1
    shift 2
    need "git clone" git clone -q "$@" "file://$source" "$clone" || return 1
    check "fresh clone has no record" "no .familiar/record" [ ! -e "$clone/.familiar/record" ]
    if [ -n "${CLONE_SETUP:-}" ]; then
        need "the clone's setup ($CLONE_SETUP)" "$CLONE_SETUP" "$clone" || return 1
    fi
    LAST_INPUT="familiar-ai verify (in a fresh clone)"
    LAST_ERR=""
    OUT=$(cd "$clone" && familiar-ai verify 2>&1)
    RC=$?
    if verbose; then
        printf '\n$ familiar-ai verify (in a fresh clone), exit %s\n%s\n' "$RC" "$OUT"
    fi
}

# run_scenario <name> <function> <checks>: one line on success, each failure in full
run_scenario() {
    SCENARIO=$1
    CHECKS=0
    SCENARIO_FAILS=0
    STOPPED=""
    OUT=""
    LAST_INPUT=""
    LAST_ERR=""
    RC=0
    if verbose; then
        printf '\n=== %s ===\n' "$1"
    fi
    "$2"
    TOTAL_CHECKS=$((TOTAL_CHECKS + CHECKS))
    EXPECTED_CHECKS=$((EXPECTED_CHECKS + $3))
    if [ -n "$STOPPED" ]; then
        FAILS=$((FAILS + 1))
        printf 'STOPPED %s after %d of %d checks: %s\n' "$1" "$CHECKS" "$3" "$STOPPED"
    elif [ "$SCENARIO_FAILS" != 0 ]; then
        printf 'FAILED %s (%d of %d checks)\n' "$1" "$SCENARIO_FAILS" "$CHECKS"
    elif [ "$CHECKS" != "$3" ]; then
        FAILS=$((FAILS + 1))
        printf 'FAILED %s (ran %d of %d checks)\n' "$1" "$CHECKS" "$3"
    else
        printf 'ok %s (%d checks)\n' "$1" "$CHECKS"
    fi
}

scenario_laptop_green() {
    need "a built familiar-ai at $BIN (cargo build)" test -x "$BIN" || return
    PROJECT="$TMP/project"
    mkdir -p "$PROJECT/src"
    need "git init" git_in init -q -b main || return
    printf '#!/bin/sh\ngreet() { echo hi; }\ngreet\n' > "$PROJECT/src/app.sh"
    need "familiar-ai setup" cli setup || return
    need "git add" git_in add -A || return
    need "the base commit" git_in commit -q -m base || return

    need "claim intent_submitted" cli claim intent_submitted --for 0 --text "greet with hello" || return
    # definition change (the owner, 2026-10-11): light is for documentation only; a code anchor is the full path
    need "claim behavior" cli claim behavior --behavior_id b1 --text "the greeting" || return
    need "claim promise" cli claim promise --promise_id p1 --behavior_id b1 --accepts "a name" --gives_back "a greeting" --refuses "nothing" --cases '[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]' || return
    need "claim criterion" cli claim criterion --criterion_id c1 --text "greet says hello" --path full --behavior_id b1 --promise_id p1 || return
    cases_kept c1 || return  # the cases card passed (plan cases-card, 2026-10-25) [draft names]
    need "claim place_chosen" cli claim place_chosen --criterion_id c1 --places '[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]' || return
    need "claim test_planned" cli claim test_planned --criterion_id c1 --file_path tests/test_c1.sh --will create || return
    need "claim test_stack_detected" cli claim test_stack_detected --codebase_id ci-walk --project_root src/ --test_stack sh --test_command "sh tests/test_c1.sh" || return
    need "claim criterion_head" cli claim criterion_head --criterion_id c1 || return
    need "claim criterion_assigned" cli claim criterion_assigned --criterion_id c1 || return
    need "claim fix_location_category" cli claim fix_location_category --criterion_id c1 --category this_repository --where '' || return
    expect "plan confirmed" "familiar-ai: the plan is confirmed"

    mkdir -p "$PROJECT/tests"
    printf 'grep -q hello src/app.sh\n' > "$PROJECT/tests/test_c1.sh"
    need "git add" git_in add -A || return
    need "the tests-first commit" git_in commit -q -m "c1: tests first" || return
    hook "the tests-first commit landed" PostToolUse post_tool_use_bash "command=git add -A && git commit -m 'c1: tests first'"
    exits "tests-first hook exits 0" 0
    field_has "failing tests-only commit sealed" additionalContext "they fail, as they should before the fix · stamped"
    printf '#!/bin/sh\ngreet() { echo hello; }\ngreet\n' > "$PROJECT/src/app.sh"
    need "git add" git_in add -A || return
    need "the code commit" git_in commit -q -m "c1: code" || return
    hook "the code commit landed" PostToolUse post_tool_use_bash "command=git commit -am 'c1: code'"
    exits "code commit hook exits 0" 0
    field_has "passing code commit green" additionalContext "they pass · stamped"
    LAST_INPUT="git show HEAD:.familiar/plan"
    OUT=$(git_in show HEAD:.familiar/plan)
    expect "committed plan records the test" '"kind":"test_written"'
    LAPTOP=$PROJECT
}

# The CI scenarios start from the laptop scenario's green repository and never change it.
scenario_valid_fix() {
    need "the laptop scenario's green repository" test -n "$LAPTOP" || return
    verify_in_clone "$LAPTOP" clone-good || return
    exits "valid fix exits 0" 0
    expect "valid fix reported proven" 'familiar-ai: "greet says hello" · proven · red at '
    expect "proof names the test command" "ran: sh tests/test_c1.sh"
    check "verify records in the clone" "a .familiar/record" [ -e "$TMP/clone-good/.familiar/record" ]
    PROJECT="$TMP/clone-good"
    cli status
    expect "clone status shows verify's proof, locally verified: no CI run was seen" '"greet says hello" · locally verified'
}

scenario_reverted_fix() {
    need "the laptop scenario's green repository" test -n "$LAPTOP" || return
    PROJECT="$TMP/broken"
    need "git clone" git clone -q "file://$LAPTOP" "$PROJECT" || return
    printf '#!/bin/sh\ngreet() { echo hi; }\ngreet\n' > "$PROJECT/src/app.sh"
    need "git add" git_in add -A || return
    need "the commit that undoes the fix" git_in commit -q -m "c1: the fix undone" || return
    verify_in_clone "$PROJECT" clone-broken || return
    exits "reverted fix exits 1" 1
    expect "reverted fix reported failing at the head" '"greet says hello" · not proven: its tests fail at the head'
}

scenario_shallow_clone() {
    need "the laptop scenario's green repository" test -n "$LAPTOP" || return
    verify_in_clone "$LAPTOP" clone-shallow --depth 1 || return
    exits "shallow clone exits 2" 2
    expect "shallow clone reports missing history" "the history does not reach the intent's commit"
    expect "shallow clone suggests fetch-depth: 0" "fetch-depth: 0"
}

# editable_install <folder>: a .venv whose site-packages points at <folder>/src, the line
# `pip install -e .` writes for a src layout (written by hand: the walk installs nothing).
editable_install() {
    python3 -m venv --without-pip "$1/.venv" || return 1
    for site in "$1"/.venv/lib/python*/site-packages; do
        printf '%s/src\n' "$1" > "$site/__editable__.pkg-0.pth"
    done
}

# A Python project installed editable into its own .venv: the tests-first commit and the
# teeth run in verify's copies must import the copy's src, not the clone's fixed one.
scenario_editable_laptop() {
    need "a built familiar-ai at $BIN (cargo build)" test -x "$BIN" || return
    PROJECT="$TMP/editable"
    mkdir -p "$PROJECT/src/pkg"
    need "git init" git_in init -q -b main || return
    printf 'def add(a, b):\n    return a - b\n' > "$PROJECT/src/pkg/__init__.py"
    need "familiar-ai setup" cli setup || return
    printf '.venv/\n__pycache__/\n' >> "$PROJECT/.gitignore"
    need "git add" git_in add -A || return
    need "the base commit" git_in commit -q -m base || return
    need "the editable install" editable_install "$PROJECT" || return

    need "claim intent_submitted" cli claim intent_submitted --for 0 --text "add adds" || return
    need "claim behavior" cli claim behavior --behavior_id b1 --text "the sum" || return
    need "claim promise" cli claim promise --promise_id p1 --behavior_id b1 --accepts "two numbers" --gives_back "their sum" --refuses "nothing" --cases '[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]' || return
    need "claim criterion" cli claim criterion --criterion_id c1 --text "add returns the sum" --path full --behavior_id b1 --promise_id p1 || return
    cases_kept c1 || return  # the cases card passed (plan cases-card, 2026-10-25) [draft names]
    need "claim place_chosen" cli claim place_chosen --criterion_id c1 --places '[{"file_path":"src/pkg/__init__.py","symbol":"add","line_start":1,"line_end":2}]' || return
    need "claim test_planned" cli claim test_planned --criterion_id c1 --file_path tests/test_add.py --will create || return
    need "claim test_stack_detected" cli claim test_stack_detected --codebase_id ci-editable --project_root . --test_stack python --test_command ".venv/bin/python -m unittest tests.test_add" || return
    need "claim criterion_head" cli claim criterion_head --criterion_id c1 || return
    need "claim criterion_assigned" cli claim criterion_assigned --criterion_id c1 || return
    need "claim fix_location_category" cli claim fix_location_category --criterion_id c1 --category this_repository --where '' || return
    expect "plan confirmed" "familiar-ai: the plan is confirmed"

    mkdir -p "$PROJECT/tests"
    printf 'import unittest\nimport pkg\n\n\nclass AddTest(unittest.TestCase):\n    def test_add_returns_the_sum(self):\n        self.assertEqual(pkg.add(1, 2), 3, pkg.__file__)\n' > "$PROJECT/tests/test_add.py"
    need "git add" git_in add -A || return
    need "the tests-first commit" git_in commit -q -m "c1: tests first" || return
    hook "the tests-first commit landed" PostToolUse post_tool_use_bash "command=git add -A && git commit -m 'c1: tests first'"
    field_has "failing tests-only commit sealed" additionalContext "they fail, as they should before the fix · stamped"
    printf 'def add(a, b):\n    return a + b\n' > "$PROJECT/src/pkg/__init__.py"
    need "git add" git_in add -A || return
    need "the code commit" git_in commit -q -m "c1: code" || return
    hook "the code commit landed" PostToolUse post_tool_use_bash "command=git commit -am 'c1: code'"
    field_has "passing code commit green" additionalContext "they pass · stamped"
    EDITABLE=$PROJECT
}

# The clone is installed editable as CI would; verify's copies still import their own src.
scenario_editable_clone() {
    need "the editable scenario's green repository" test -n "$EDITABLE" || return
    CLONE_SETUP=editable_install verify_in_clone "$EDITABLE" clone-editable || return
    exits "editable install exits 0" 0
    expect "editable install reported proven" 'familiar-ai: "add returns the sum" · proven · red at '
    PROJECT="$TMP/clone-editable"
    cli status
    expect "clone status reads locally verified" '"add returns the sum" · locally verified'
}

# python_path_install <folder>: a bare .venv, and PYTHONPATH naming <folder>/src exported.
python_path_install() {
    python3 -m venv --without-pip "$1/.venv" || return 1
    export PYTHONPATH="$1/src"
}

scenario_python_path_clone() {
    need "the editable scenario's green repository" test -n "$EDITABLE" || return
    CLONE_SETUP=python_path_install verify_in_clone "$EDITABLE" clone-python-path
    unset PYTHONPATH
    [ -z "$STOPPED" ] || return
    exits "PYTHONPATH naming the clone exits 0" 0
    expect "PYTHONPATH naming the clone reported proven" 'familiar-ai: "add returns the sum" · proven · red at '
    PROJECT="$TMP/clone-python-path"
    cli status
    expect "clone status reads locally verified" '"add returns the sum" · locally verified'
}

run_scenario "laptop walk turns green" scenario_laptop_green 6
run_scenario "valid fix proven in a fresh clone" scenario_valid_fix 6
run_scenario "reverted fix not proven" scenario_reverted_fix 3
run_scenario "shallow clone lacks history" scenario_shallow_clone 4
run_scenario "editable install walk turns green" scenario_editable_laptop 3
run_scenario "editable install proven in a fresh clone" scenario_editable_clone 4
run_scenario "PYTHONPATH naming the clone proven in a fresh clone" scenario_python_path_clone 4

printf '\n'
if [ "$FAILS" = 0 ]; then
    printf 'ALL PASS (%d of %d checks)\n' "$TOTAL_CHECKS" "$EXPECTED_CHECKS"
    exit 0
else
    printf 'checks run: %d of %d\n' "$TOTAL_CHECKS" "$EXPECTED_CHECKS"
    printf '%d expectation(s) FAILED\n' "$FAILS"
    exit 1
fi
