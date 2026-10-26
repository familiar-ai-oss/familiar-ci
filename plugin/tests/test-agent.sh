#!/bin/sh
# A scripted agent walks the plugin end to end: every event goes through the
# real hook entry (plugin/hooks/run.sh) against a real git repository, and the
# agent's own moves go through `familiar-ai claim`.
# Run from the worktree root after cargo build:  sh plugin/tests/test-agent.sh
# It uses familiar-ai on PATH, else target/debug/familiar-ai, and a release
# binary only when FAMILIAR_AI_BIN names it. It exits non-zero if anything differs.

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
RUN_SH="$ROOT/plugin/hooks/run.sh"
FIX="$ROOT/crates/familiar-ai/tests/fixtures"
SESSION=84be6339-0cc7-40ec-91b9-cae44365931b

if [ -n "${FAMILIAR_AI_BIN:-}" ]; then
    BIN=$FAMILIAR_AI_BIN
elif command -v familiar-ai >/dev/null 2>&1; then
    BIN=$(command -v familiar-ai)
else
    BIN="$ROOT/target/debug/familiar-ai"
fi
if [ ! -x "$BIN" ]; then
    printf 'no familiar-ai at %s (run cargo build)\n' "$BIN" >&2
    exit 1
fi
# a stale release build is never tested by accident
if [ -z "${FAMILIAR_AI_BIN:-}" ] && [ "$(cd "$(dirname "$BIN")" && pwd -P)" = "$(cd "$ROOT" && pwd -P)/target/release" ]; then
    printf 'familiar-ai on PATH is the release build %s; name it with FAMILIAR_AI_BIN to test it\n' "$BIN" >&2
    exit 1
fi
printf 'binary: %s\n' "$BIN"

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/bin"
ln -s "$BIN" "$TMP/bin/familiar-ai"
export PATH="$TMP/bin:$PATH"
unset CLAUDE_PROJECT_DIR
# a headless runner (claude -p) is not the walk's session: it would count as the bench
unset CLAUDE_CODE_ENTRYPOINT
export CLAUDE_CODE_SESSION_ID="$SESSION"
# the setup markers of this walk stay in the scratch folder, never the real home
export FAMILIAR_AI_HOME="$TMP/home"
export FAMILIAR_AI_JUDGE=off  # only test-agent-judge.sh asks a judge, a fake one
export FAMILIAR_AI_INVESTIGATOR=off  # the binary-run investigator only in test-agent-investigator.sh, a fake one

VERBOSE=${VERBOSE:-0}
FAILS=0           # failed checks in the whole walk
CHECKS=0          # checks in the running scenario
SCENARIO_FAILS=0  # failed checks in the running scenario
SCENARIO=""
OUT=""            # what the last step printed
LAST_INPUT=""     # what the last step was given
LAST_ERR=""       # what the last hook wrote to stderr
RC=0              # the last command's exit code

verbose() { [ "$VERBOSE" = 1 ]; }

pass() {
    CHECKS=$((CHECKS + 1))
    if verbose; then
        printf 'PASS %s\n' "$1"
    fi
}

# fail <label> <what was expected>: prints the last step's input, the
# expected value beside the actual output, and the step's stderr
fail() {
    CHECKS=$((CHECKS + 1))
    SCENARIO_FAILS=$((SCENARIO_FAILS + 1))
    printf 'FAIL %s: %s\n' "$SCENARIO" "$1"
    printf '  input:    %s\n' "$LAST_INPUT"
    printf '  expected: %s\n' "$2"
    printf '  actual:   %s\n' "$OUT"
    if [ -n "$LAST_ERR" ]; then
        printf '  stderr:   %s\n' "$LAST_ERR"
    fi
}

# expect <label> <text that must be in $OUT>
expect() {
    case "$OUT" in
        *"$2"*)
            pass "$1"
            ;;
        *)
            fail "$1" "to contain: $2"
            ;;
    esac
}

# exactly <label> <the whole of $OUT>
exactly() {
    if [ "$OUT" = "$2" ]; then
        pass "$1"
    else
        fail "$1" "exactly: $2"
    fi
}

# exits <label> <code>: the last command's exit code
exits() {
    if [ "$RC" = "$2" ]; then
        pass "$1"
    else
        fail "$1" "exit $2, got exit $RC"
    fi
}

# setup_failed <what>: a step the scenario stands on failed; the scenario
# stops here (it runs in a subshell, so the walk goes on) and run_scenario
# names the step in its one line, apart from the counted checks
setup_failed() {
    printf '%s\n' "$1" > "$TMP/stopped"
    exit 0
}

# need <what> <command ...>: a setup step, not a check; its failure stops the scenario
need() {
    what=$1
    shift
    if ! "$@" > "$TMP/need.out" 2>&1; then
        setup_failed "$what: $(head -n 1 "$TMP/need.out")"
    fi
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
    if k.startswith("@"): d[k[1:]] = v  # a top-level field
    else: d.setdefault("tool_input", {})[k] = v
print(json.dumps(d))
EOF
}

# hook <label> <Event> <fixture> [key=value ...]: through run.sh, as Claude
# Code calls it; the JSON emitted is kept in $OUT.
hook() {
    label=$1; event=$2; shift 2
    LAST_INPUT="hook $event ($label) $(payload "$@")"
    OUT=$(payload "$@" | /bin/sh "$RUN_SH" "$event" 2>"$TMP/err")
    RC=$?
    LAST_ERR=$(cat "$TMP/err")
    if verbose; then
        printf '\n### %s — hook %s → exit %s\n%s\n' "$label" "$event" "$RC" "$OUT"
        if [ -n "$LAST_ERR" ]; then printf '(stderr) %s\n' "$LAST_ERR"; fi
    fi
    [ "$RC" = 0 ] || fail "$label" "exit 0, got exit $RC"
}

# cli <args>: `familiar-ai <args>` in the project, from the agent's shell
cli() {
    LAST_INPUT="familiar-ai $*"; LAST_ERR=""
    OUT=$(cd "$PROJECT" && familiar-ai "$@" 2>&1)
    RC=$?
    if verbose; then printf '\n$ familiar-ai %s → exit %s\n%s\n' "$*" "$RC" "$OUT"; fi
}

# person_cli <args>: the same from the person's terminal, with no session id
person_cli() {
    LAST_INPUT="(the person's terminal) familiar-ai $*"; LAST_ERR=""
    OUT=$(cd "$PROJECT" && env -u CLAUDE_CODE_SESSION_ID familiar-ai "$@" 2>&1)
    RC=$?
    if verbose; then printf '\n$ familiar-ai %s (the person'"'"'s terminal) → exit %s\n%s\n' "$*" "$RC" "$OUT"; fi
}

# the agent's own move: claim <kind> --<field> <value> ...
claim() {
    cli claim "$@"
    [ "$RC" = 0 ] || fail "claim $1" "exit 0, got exit $RC"
}

# read_file <path>: the file's text into $OUT
read_file() {
    LAST_INPUT="the file $1"; LAST_ERR=""
    OUT=$(cat "$1" 2>/dev/null)
}

git_in() { git -C "$PROJECT" -c user.name=t -c user.email=t@example.com -c commit.gpgsign=false "$@"; }

# run_scenario <name> <function>: one line on success, each failure in full;
# the scenario runs in a subshell and hands back its counts through a file
run_scenario() {
    SCENARIO=$1
    if verbose; then
        printf '\n=== %s ===\n' "$1"
    fi
    rm -f "$TMP/stopped"
    (
        CHECKS=0
        SCENARIO_FAILS=0
        OUT=""
        LAST_INPUT=""
        LAST_ERR=""
        trap 'printf "%s %s\n" "$CHECKS" "$SCENARIO_FAILS" > "$TMP/counts"' EXIT
        "$2"
    )
    read -r CHECKS SCENARIO_FAILS < "$TMP/counts"
    FAILS=$((FAILS + SCENARIO_FAILS))
    if [ -e "$TMP/stopped" ]; then
        FAILS=$((FAILS + 1))
        printf 'STOPPED %s: setup failed: %s (%d of %d checks failed before it)\n' "$1" "$(cat "$TMP/stopped")" "$SCENARIO_FAILS" "$CHECKS"
    elif [ "$SCENARIO_FAILS" = 0 ]; then
        printf 'ok %s (%d checks)\n' "$1" "$CHECKS"
    else
        printf 'FAILED %s (%d of %d checks)\n' "$1" "$SCENARIO_FAILS" "$CHECKS"
    fi
}

# a fresh repository, set up as the setup skill does it: `familiar-ai setup`
# makes the .familiar folder and the ignore line (the base commit carries
# the .gitignore, so the tests-first commit later is tests alone); setup's
# words are left in $OUT
fresh_project() {
    PROJECT="$TMP/$1"
    mkdir -p "$PROJECT/src"
    printf '#!/bin/sh\ngreet() { echo hi; }\ngreet\n' > "$PROJECT/src/app.sh"
    need "git init" git_in init -q -b main
    cli setup
    if [ "$RC" != 0 ]; then
        setup_failed "familiar-ai setup: $OUT"
    fi
    need "git add" git_in add -A
    need "the base commit" git_in commit -q -m base
}

# hook_with <label> <Event> <json>: the JSON itself through run.sh, as Claude Code pipes it
hook_with() {
    label=$1; event=$2
    LAST_INPUT="hook $event ($label) $3"
    OUT=$(printf '%s' "$3" | /bin/sh "$RUN_SH" "$event" 2>"$TMP/err")
    RC=$?
    LAST_ERR=$(cat "$TMP/err")
    if verbose; then
        printf '\n### %s — hook %s → exit %s\n%s\n' "$label" "$event" "$RC" "$OUT"
        if [ -n "$LAST_ERR" ]; then printf '(stderr) %s\n' "$LAST_ERR"; fi
    fi
}

# finder_answer <the spawn's PreToolUse reply> <session>: the PostToolUse on the Agent call that
# released the case-finder [draft names] (plan cases-card §1b, in the shape of the spike's capture
# of 2026-10-25): the spawn as the hook rewrote it, and the finder's final text, a fenced JSON
# answer with no cases and why
finder_answer() {
    python3 - "$1" "$PROJECT" "$2" <<'EOF'
import json, sys
reply = json.loads(sys.argv[1] or "{}")
spawn = reply.get("hookSpecificOutput", {}).get("updatedInput") or {}
answer = {"cases": [], "none_because": "the planned cases cover what the person asked"}
print(json.dumps({
    "session_id": sys.argv[3], "cwd": sys.argv[2], "hook_event_name": "PostToolUse", "permission_mode": "bypassPermissions",
    "tool_name": "Agent", "tool_input": spawn, "tool_use_id": "toolu_01CaseFinderWalk",
    "tool_response": {"status": "completed", "prompt": spawn.get("prompt", ""), "agentId": "a0ca5ef1d0e4walk", "agentType": spawn.get("subagent_type", ""),
                      "content": [{"type": "text", "text": "```json\n" + json.dumps(answer) + "\n```"}],
                      "resolvedModel": "claude-sonnet-4-5", "totalDurationMs": 1000, "totalTokens": 1234, "totalToolUseCount": 3}}))
EOF
}

# cases_kept <criterion_id>: the cases card passed [draft names] (plan cases-card, 2026-10-25),
# once the part's contract is whole: the agent releases the case-finder with the Agent tool and
# the PreToolUse rewrites the spawn (subagent_type familiar-ai:case-finder, the binary's brief,
# the model sonnet unless FAMILIAR_AI_CASE_FINDER_MODEL names another); the finder answers that
# the planned cases cover it, which the binary reads on the PostToolUse of that Agent call and
# records as cases_found; the person keeps the planned cases from a terminal of their own; the
# agent's next tool call is told so. Setup steps, not checks: a step that differs stops the
# scenario, naming it.
cases_kept() {
    hook "release the case-finder for $1" PreToolUse pre_tool_use_agent "@session_id=$SESSION"
    case "$OUT" in *'"subagent_type":"familiar-ai:case-finder"'*) ;; *) setup_failed "the case-finder's spawn for $1: $OUT" ;; esac
    hook_with "the case-finder answered for $1" PostToolUse "$(finder_answer "$OUT" "$SESSION")"
    cli status --json
    case "$OUT" in *'"cases_unconfirmed"'*) ;; *) setup_failed "the cases card for $1: $OUT" ;; esac
    person_cli claim hold_answered --criterion_id "$1" --hold cases_unconfirmed --choice keep_planned --typed ''
    [ "$RC" = 0 ] || setup_failed "the person keeps the planned cases for $1: $OUT"
    hook "the next tool call after the cases card of $1" PostToolUse post_tool_use_bash "command=ls"
    case "$OUT" in *"the person answered the question for"*) ;; *) setup_failed "the agent told the person's answer for $1: $OUT" ;; esac
}

# the agent fills the plan, one line at a time; the planning scenario checks
# what each claim answers
plan_the_project() {
    claim intent_submitted --for 0 --text "greet with hello"
    # definition change (the owner, 2026-10-11): light is for documentation only; a code anchor is the full path
    claim behavior --behavior_id b1 --text "the greeting"
    claim promise --promise_id p1 --behavior_id b1 --accepts "a name" --gives_back "a greeting" --refuses "nothing" --cases '[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]'
    claim criterion --criterion_id c1 --text "greet says hello" --path full --behavior_id b1 --promise_id p1
    cases_kept c1  # the cases card passed (plan cases-card, 2026-10-25) [draft names]
    claim place_chosen --criterion_id c1 --places '[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]'
    claim test_planned --criterion_id c1 --file_path tests/test_c1.sh --will create
    claim test_stack_detected --codebase_id agent-walk --project_root src/ --test_stack sh --test_command "sh tests/test_c1.sh"
    claim criterion_head --criterion_id c1
    claim criterion_assigned --criterion_id c1
    claim fix_location_category --criterion_id c1 --category this_repository --where ''
}

# ---------------------------------------------------------------------------
scenario_planning_guidance() {
    fresh_project planning
    expect "setup made the folder" "made .familiar at $PROJECT"
    expect "setup marked the project outside the repository" "marked this project as set up at $FAMILIAR_AI_HOME/projects/"
    expect "setup ignored the record" "added .familiar/record to $PROJECT/.gitignore"
    claim intent_submitted --for 0 --text "greet with hello"
    expect "the intent's answer names line 2" "claim it: familiar-ai claim criterion"
    # definition change (the owner, 2026-10-11): light is for documentation only; a code anchor is the full path
    claim behavior --behavior_id b1 --text "the greeting"
    claim promise --promise_id p1 --behavior_id b1 --accepts "a name" --gives_back "a greeting" --refuses "nothing" --cases '[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]'
    claim criterion --criterion_id c1 --text "greet says hello" --path full --behavior_id b1 --promise_id p1
    # definition change (plan cases-card, 2026-10-25) [draft words]: a whole contract owes the case-finder first; line 4 follows its card
    expect "the criterion's answer names the case-finder" "\"greet says hello\" · before the test, a case-finder looks for the checks the planned cases miss"
    cases_kept c1  # the cases card passed (plan cases-card, 2026-10-25) [draft names]
    claim place_chosen --criterion_id c1 --places '[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]'
    expect "the anchor's answer names line 6" "claim it: familiar-ai claim test_planned"
    claim test_planned --criterion_id c1 --file_path tests/test_c1.sh --will create
    expect "the tests' answer names line 7" "claim it: familiar-ai claim test_stack_detected"
    expect "the planned files are sh's: line 7's claim runs them" "7 the command     claim it: familiar-ai claim test_stack_detected --project_root . --test_stack sh --test_command 'sh tests/test_c1.sh'"
    claim test_stack_detected --codebase_id agent-walk --project_root src/ --test_stack sh --test_command "sh tests/test_c1.sh"
    expect "the command's answer names line 9" "claim it: familiar-ai claim criterion_head"
    claim criterion_head --criterion_id c1
    expect "the string's answer names line 10" "claim it: familiar-ai claim criterion_assigned"
    claim criterion_assigned --criterion_id c1
    expect "the holder's answer names line 11" "claim it: familiar-ai claim fix_location_category --criterion_id c1"
    claim fix_location_category --criterion_id c1 --category this_repository --where ''
    expect "the plan is confirmed" "familiar-ai: the plan is confirmed"
    expect "c1 is testing: its test comes first" "familiar-ai: \"greet says hello\" · the test comes first"
    read_file "$PROJECT/.familiar/plan"
    expect "the start check is recorded as c1 enters testing" '"checked_at":"start"'
    expect "the planning check matched" '"matched":true,"checked_at":"planning"'
}

# ---------------------------------------------------------------------------
scenario_tests_first() {
    fresh_project right

    hook "session start" SessionStart session_start
    expect "SessionStart teaches the discipline" "familiar-ai: proof that the work is done"
    expect "SessionStart names the first missing line's command" "claim it: familiar-ai claim intent_submitted"

    hook "edit before the plan" PreToolUse pre_tool_use_edit "file_path=$PROJECT/src/app.sh"
    expect "the edit is refused" '"permissionDecision":"deny"'
    expect "the refusal says what comes first" "familiar-ai: the plan comes first · line 1: the intent · src/app.sh is code"
    case "$OUT" in *planning_incomplete*|*refused*) fail "the person's words, not the model's" "no planning_incomplete or refused" ;; *) pass "the person's words, not the model's" ;; esac
    expect "the refusal names the claim command, the form's example, no placeholder" "claim it: familiar-ai claim intent_submitted --text 'greet with hello'"
    case "$OUT" in *'<'*) fail "no placeholder in the hook's line" "no <" ;; *) pass "no placeholder in the hook's line" ;; esac

    plan_the_project

    hook "write the test" PreToolUse pre_tool_use_write "file_path=$PROJECT/tests/test_c1.sh"
    expect "writing the planned test is allowed, told what it is for" '{"hookSpecificOutput":{"hookEventName":"PreToolUse","additionalContext":"familiar-ai: tests/test_c1.sh is the planned test of \"greet says hello\"\n  asked for: greet with hello\n  it must fail on the code as it is, committed alone, before any fix; familiar-ai runs sh tests/test_c1.sh after each commit"}}'
    mkdir -p "$PROJECT/tests"
    printf 'grep -q hello src/app.sh\n' > "$PROJECT/tests/test_c1.sh"
    hook "the tests-first commit (courtesy)" PreToolUse pre_tool_use_bash "command=git add -A && git commit -m 'c1: tests first'"
    expect "a commit is allowed once the plan is confirmed" "{}"
    git_in add -A && git_in commit -q -m "c1: tests first"
    hook "the tests-first commit landed" PostToolUse post_tool_use_bash "command=git add -A && git commit -m 'c1: tests first'"
    expect "the tests failed and were stamped: sealed" "ran the tests of \\\"greet says hello\\\": they fail, as they should before the fix · stamped"

    hook "edit of the sealed test" PreToolUse pre_tool_use_edit "file_path=$PROJECT/tests/test_c1.sh"
    expect "the sealed test may not be edited" '"permissionDecision":"deny"'
    expect "the words" "the sealed test was changed; fix the code without editing the test, or commit the changed test alone and see it fail"
    expect "what comes next" "· next: announce the test change first\\n"
    expect "the claim command for it, the criterion filled in" "claim it: familiar-ai claim test_change_announced --criterion_id c1 --reason 'the assertion named the wrong file'"

    hook "edit of the anchor" PreToolUse pre_tool_use_edit "file_path=$PROJECT/src/app.sh"
    expect "the code on the anchor may be edited" "{}"
    printf '#!/bin/sh\ngreet() { echo hello; }\ngreet\n' > "$PROJECT/src/app.sh"
    git_in add -A && git_in commit -q -m "c1: code"
    hook "the code commit landed" PostToolUse post_tool_use_bash "command=git commit -am 'c1: code'"
    expect "the tests pass under the same stamp: green" "they pass · stamped"

    hook "stop" Stop stop
    expect "every criterion green: the agent may stop, told the call to action as facts" '{"hookSpecificOutput":{"hookEventName":"Stop","additionalContext":"familiar-ai: every criterion is locally green. CI proves it on the pull request. The branch is not pushed. The person has not been asked whether to push and open the pull request. The push begins with familiar-ai tidy, which keeps one seal commit and one fix commit per criterion and drops the failed attempts."}}'

    if verbose; then
        printf '\n### the final .familiar/plan (%s lines)\n' "$(wc -l < "$PROJECT/.familiar/plan" | tr -d ' ')"
        cat "$PROJECT/.familiar/plan"
    fi
    LAST_INPUT="git show --stat HEAD~1"
    OUT=$(git_in show --stat --format= HEAD~1)
    expect "the plan file rode along in the tests-first commit" ".familiar/plan"
}

# ---------------------------------------------------------------------------
scenario_mixed_commit() {
    fresh_project wrong
    plan_the_project

    mkdir -p "$PROJECT/tests"
    printf 'grep -q hello src/app.sh\n' > "$PROJECT/tests/test_c1.sh"
    printf '#!/bin/sh\ngreet() { echo hello; }\ngreet\n' > "$PROJECT/src/app.sh"
    git_in add -A && git_in commit -q -m "c1: tests and code together"
    hook "the mixed commit landed" PostToolUse post_tool_use_bash "command=git add -A && git commit -m 'c1: tests and code together'"
    expect "the hold is derived from the commit" "tests and code are in one commit; the tests go first, alone"
    hook "stop while held" Stop stop
    expect "the stop is refused on a hold" '"decision":"block"'
    expect "the refusal names the criterion by its words and the hold" '"reason":"familiar-ai: \"greet says hello\" · tests and code are in one commit; the tests go first, alone · next: '
}

# ---------------------------------------------------------------------------
scenario_removed_state() {
    fresh_project removal
    plan_the_project
    git_in add -A && git_in commit -q -m "the plan"
    PLAN_COMMITTED=$(cat "$PROJECT/.familiar/plan")
    rm -rf "$PROJECT/.familiar"
    hook "edit after rm -rf .familiar" PreToolUse pre_tool_use_edit "file_path=$PROJECT/src/app.sh"
    expect "the edit is denied" '"permissionDecision":"deny"'
    expect "the line says the folder was removed" "this project was set up, and its .familiar folder, plan file or record was removed"
    expect "the line names the setup command" "next: \`familiar-ai setup\`"
    hook "stop after rm -rf .familiar" Stop stop
    expect "the stop is blocked" '"decision":"block"'
    [ -e "$PROJECT/.familiar" ] && fail "nothing is created by a refusal" "no .familiar"
    hook "the setup call" PreToolUse pre_tool_use_bash "command=familiar-ai setup"
    expect "the exact setup call is let through" "{}"
    cli setup
    [ "$RC" = 0 ] || fail "familiar-ai setup (restore)" "exit 0, got exit $RC"
    expect "setup made the folder again" "made .familiar at $PROJECT"
    expect "setup brought the plan back from git" "restored .familiar/plan from git"
    expect "setup started the record from the committed plan" ".familiar/record starts from the committed plan: 1 part, 1 commit rechecked"
    read_file "$PROJECT/.familiar/plan"
    [ "$OUT" = "$PLAN_COMMITTED" ] && pass "the plan is the committed one" || fail "the plan" "the committed plan: $PLAN_COMMITTED"
    hook "edit after the restore" PreToolUse pre_tool_use_edit "file_path=$PROJECT/src/app.sh"
    expect "work continues from the committed plan: the edit goes ahead" "{}"
    cli status
    expect "the committed plan is confirmed in the record" "familiar-ai: the plan is confirmed"
    expect "its part is in the record" '"greet says hello"'
    claim intent_submitted --for 0 --text "greet with hello, again"
    rm -f "$PROJECT/.familiar/record"
    hook "edit after rm .familiar/record" PreToolUse pre_tool_use_edit "file_path=$PROJECT/src/app.sh"
    expect "the record alone removed is denied the same way" "or record was removed"
    cli setup
    expect "setup starts the record again, from the committed plan" ".familiar/record starts from the committed plan: 1 part, 1 commit rechecked"
    hook "edit after the record restart" PreToolUse pre_tool_use_edit "file_path=$PROJECT/src/app.sh"
    expect "work continues from the committed plan" "{}"
}

# ---------------------------------------------------------------------------
scenario_uncommitted_state() {
    fresh_project hatch
    claim intent_submitted --for 0 --text "greet with hello"
    # definition change (the owner, 2026-10-11): light is for documentation only; a code anchor is the full path
    claim behavior --behavior_id b1 --text "the greeting"
    claim promise --promise_id p1 --behavior_id b1 --accepts "a name" --gives_back "a greeting" --refuses "nothing" --cases '[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]'
    claim criterion --criterion_id c1 --text "greet says hello" --path full --behavior_id b1 --promise_id p1
    cases_kept c1  # the cases card passed (plan cases-card, 2026-10-25) [draft names]
    hook "git reset --hard while the plan is uncommitted" PreToolUse pre_tool_use_bash "command=git reset --hard HEAD"
    expect "the reset is denied" '"permissionDecision":"deny"'
    expect "the line: not the agent's to remove" "familiar-ai: Familiar's folder stays as it is: it is the person's to remove, not the agent's"
    hook "rm -rf by absolute path" PreToolUse pre_tool_use_bash "command=rm -rf $PROJECT/.familiar"
    expect "the rm is denied" '"permissionDecision":"deny"'
    expect "the same line" "it is the person's to remove, not the agent's"
    hook "reading the plan" PreToolUse pre_tool_use_bash "command=cat .familiar/plan"
    expect "reading the folder stays allowed" "{}"
    [ -d "$PROJECT/.familiar" ] && pass "nothing was removed" || fail "the courtesy" ".familiar still there"
    # forced past the courtesy: the script removes the folder itself (the shell can hide the path)
    git_in reset -q --hard HEAD
    rm -rf "$PROJECT/.familiar"
    hook "edit after the forced removal" PreToolUse pre_tool_use_edit "file_path=$PROJECT/src/app.sh"
    expect "the edit is denied by the marker: a removal, not a fresh project" '"permissionDecision":"deny"'
    expect "the line says the folder was removed" "this project was set up, and its .familiar folder, plan file or record was removed"
    expect "the line names the setup command" "next: \`familiar-ai setup\`"
    hook "stop after the forced removal" Stop stop
    expect "the stop is blocked" '"decision":"block"'
    hook "the setup call" PreToolUse pre_tool_use_bash "command=familiar-ai setup"
    expect "the exact setup call is let through" "{}"
    cli setup
    [ "$RC" = 0 ] || fail "familiar-ai setup (after the hatch)" "exit 0, got exit $RC"
    expect "setup made the folder again" "made .familiar at $PROJECT"
    expect "the marker was already there" "this project is already marked as set up at $FAMILIAR_AI_HOME/projects/"
    hook "edit after the restore" PreToolUse pre_tool_use_edit "file_path=$PROJECT/src/app.sh"
    expect "work continues from the empty record" "claim it: familiar-ai claim intent_submitted"
    if verbose; then
        MARKER=$(grep -l "^root: $(cd "$PROJECT" && pwd -P)\$" "$FAMILIAR_AI_HOME"/projects/*)
        printf '\n### the marker %s\n%s\n' "$MARKER" "$(cat "$MARKER")"
    fi
}

# ---------------------------------------------------------------------------
scenario_subfolder_root() {
    PROJECT="$TMP/subfolder"
    mkdir -p "$PROJECT/pkg"
    git_in init -q -b main
    printf 'def schema():\n    return {}\n' > "$PROJECT/pkg/json_schema.py"
    (cd "$PROJECT" && familiar-ai setup >/dev/null 2>&1) || fail "familiar-ai setup (subfolder)" "exit 0"
    git_in add -A && git_in commit -q -m base

    # the door's teaching, before any claim: the listing and a form
    cli claim </dev/null
    expect "claim alone lists the kinds" "  test_stack_detected       line 7: the command that runs the tests"
    cli claim test_stack_detected </dev/null
    expect "a kind alone prints its form" "--project_root  folder  required         . is the whole project"
    # definition change (lane l1, 2026-10-15): the example test file is named after what it tests, never a plan id
    # definition change (the owner, 2026-10-15, §9 Q3): python -m pytest, python3 when no python is on PATH
    if command -v python >/dev/null 2>&1; then PY=python; else PY=python3; fi
    expect "the form carries a complete example" "familiar-ai claim test_stack_detected --project_root . --test_stack pytest --test_command '$PY -m pytest -q tests/test_greet.py'"

    claim intent_submitted --for 0 --text "schema names the model"
    # definition change (the owner, 2026-10-11): light is for documentation only; a code anchor is the full path
    claim behavior --behavior_id b1 --text "the greeting"
    claim promise --promise_id p1 --behavior_id b1 --accepts "a name" --gives_back "a greeting" --refuses "nothing" --cases '[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]'
    claim criterion --criterion_id c1 --text "schema carries the title" --path full --behavior_id b1 --promise_id p1
    cases_kept c1  # the cases card passed (plan cases-card, 2026-10-25) [draft names]
    claim place_chosen --criterion_id c1 --places '[{"file_path":"pkg/json_schema.py","symbol":"schema","line_start":1,"line_end":2}]'
    claim test_planned --criterion_id c1 --file_path tests/test_c1.sh --will create
    expect "line 7 is missing" "claim it: familiar-ai claim test_stack_detected"

    # a live agent pasted the example verbatim
    cli claim test_stack_detected --project_root . --test_stack pytest --test_command "$PY -m pytest -q tests/test_greet.py"
    exits "the example claimed verbatim is refused" 1
    expect "the refusal says it is the example and names the planned file" "is the example; name the command that runs tests/test_c1.sh"
    cli claim test_stack_detected --project_root . --test_stack sh --test_command "sh tests/test_c9.sh"
    exits "a command naming a file neither in the tree nor planned is refused" 1
    expect "the refusal names the missing file" "names tests/test_c9.sh, which is neither in the working tree nor a planned test file"

    # what the live agent tried: a kind it invented, a field it left out, roots that never applied
    cli claim criterion_behavior_chosen --criterion_id c1
    exits "an invented kind is refused" 1
    expect "the refusal is followed by the listing" "  intent_submitted          line 1: what the person asked for"
    cli claim test_stack_detected --project_root . --test_stack pytest
    exits "a missing field is refused" 1
    expect "the refusal names the field" "missing the required field 'test_command'"
    expect "the refusal is followed by the form" "--test_command  text    required         what the binary runs from the project's root"
    cli claim test_stack_detected --project_root /work/repo --test_stack pytest --test_command "sh tests/test_c1.sh"
    exits "a root outside the project is refused" 1
    expect "the refusal says where the project is" "project_root /work/repo is outside the project at $PROJECT"
    claim test_stack_detected --project_root src/ --test_stack sh --test_command "sh tests/test_c1.sh"
    expect "src/ does not cover pkg/: line 7 still missing" "claim it: familiar-ai claim test_stack_detected"
    claim test_stack_detected --project_root . --test_stack sh --test_command "sh tests/test_c1.sh"
    expect "the dot is stored as the empty prefix" '"project_root":""'
    expect "the id is the repository folder's name" '"codebase_id":"subfolder"'
    expect "the dot covers pkg/: line 9 is next" "claim it: familiar-ai claim criterion_head"
    claim criterion_head --criterion_id c1
    claim criterion_assigned --criterion_id c1
    claim fix_location_category --criterion_id c1 --category this_repository --where ''
    expect "the plan is confirmed with the dot root" "familiar-ai: the plan is confirmed"
}

# ---------------------------------------------------------------------------
scenario_shell_writes() {
    fresh_project shellwrite
    hook "edit before the plan" PreToolUse pre_tool_use_edit "file_path=$PROJECT/src/app.sh"
    # the Edit's words, less the changed line it names (a shell write's text is unknown)
    EDIT_REASON=$(printf '%s' "$OUT" | python3 -c 'import json,sys; print("\n".join(l for l in json.load(sys.stdin)["hookSpecificOutput"]["permissionDecisionReason"].split("\n") if not (l.startswith("  line ") and " is code: " in l)))')
    hook "cat > the anchor before the plan" PreToolUse pre_tool_use_bash "command=$(printf "cat > src/app.sh << 'EOF'\\necho hi\\nEOF")"
    expect "the shell write is denied" '"permissionDecision":"deny"'
    SHELL_REASON=$(printf '%s' "$OUT" | python3 -c 'import json,sys; print(json.load(sys.stdin)["hookSpecificOutput"]["permissionDecisionReason"])')
    [ "$SHELL_REASON" = "$EDIT_REASON
(a shell write counts as an edit)" ] && pass "the words are the Edit's, then the one line" || fail "the shell write's words" "the Edit's words, then (a shell write counts as an edit): $EDIT_REASON"
    hook "python heredoc that writes the anchor" PreToolUse pre_tool_use_bash "command=$(printf "python3 - << 'EOF'\\nsrc = open('src/app.sh').read()\\nopen('src/app.sh', 'w').write(src)\\nEOF")"
    expect "the python write is denied" '"permissionDecision":"deny"'
    expect "with the line" "(a shell write counts as an edit)"
    hook "cat >> a test file before the plan" PreToolUse pre_tool_use_bash "command=$(printf "cat >> tests/test_c1.sh << 'EOF'\\ngrep -q hello src/app.sh\\nEOF")"
    expect "the append is denied" "familiar-ai: the plan comes first"
    hook "a write to /tmp" PreToolUse pre_tool_use_bash "command=echo x > /tmp/familiar-ai-agent-walk-outside"
    expect "outside the project: allowed" "{}"
    hook "a read" PreToolUse pre_tool_use_bash "command=cat src/app.sh"
    expect "a read: allowed" "{}"
    hook "a claim whose text carries >" PreToolUse pre_tool_use_bash "command=familiar-ai claim intent_submitted --text 'a > b'"
    expect "a quoted > is text: allowed" "{}"
    [ -e "$PROJECT/tests" ] && fail "nothing is written by a refusal" "no tests/"
    plan_the_project
    hook "cat > the anchor after the plan" PreToolUse pre_tool_use_bash "command=$(printf "cat > src/app.sh << 'EOF'\\necho hello\\nEOF")"
    expect "the plan confirmed: the anchor may be written by the shell" "{}"
    hook "cat > another file after the plan" PreToolUse pre_tool_use_bash "command=echo x > src/other.sh"
    expect "another file: told, as an Edit is" "familiar-ai: this edit is outside the place the plan named for"
}

# ---------------------------------------------------------------------------
scenario_full_path() {
    fresh_project fullpath
    claim intent_submitted --for 0 --text "greet with hello"
    claim criterion --criterion_id c1 --text "greet says hello" --path full
    expect "the gap is named" "familiar-ai: the plan comes first · next: \"greet says hello\" is on the full path but names no behavior and promise: claim behavior b1 and promise p1, then re-claim the criterion with --behavior_id b1 --promise_id p1"
    expect "the behavior's command" "claim it: familiar-ai claim behavior --behavior_id b1 --text 'greeting by name'"
    expect "the re-claim with the ids" "then:     familiar-ai claim criterion --criterion_id c1 --text 'greet says hello' --path full --behavior_id b1 --promise_id p1"
    claim behavior --behavior_id b1 --text "greeting by name"
    claim promise --promise_id p1 --behavior_id b1 --accepts "a name" --gives_back "a greeting" --refuses "an empty name" --cases '[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]'
    expect "both claimed, nothing linked: the re-claim is what is left" "behavior b1 and promise p1 are claimed; re-claim the criterion with --behavior_id b1 --promise_id p1"
    claim criterion --criterion_id c1 --text "greet says hello" --path full --behavior_id b1 --promise_id p1
    # definition change (plan cases-card, 2026-10-25) [draft words]: linked, the contract is whole and owes the case-finder first; line 4 follows its card
    expect "linked: the case-finder is next" "\"greet says hello\" · before the test, a case-finder looks for the checks the planned cases miss"
    cases_kept c1  # the cases card passed (plan cases-card, 2026-10-25) [draft names]
}

# ---------------------------------------------------------------------------
scenario_passing_test() {
    fresh_project notred
    plan_the_project
    mkdir -p "$PROJECT/tests"
    printf 'grep -q hi src/app.sh\n' > "$PROJECT/tests/test_c1.sh"
    git_in add -A && git_in commit -q -m "c1: a test that already passes"
    hook "the tests-first commit landed, green" PostToolUse post_tool_use_bash "command=git add -A && git commit -m 'c1: tests first'"
    expect "the hold" "the test must fail on the code as it is, committed alone before any fix; reverting a fix does not count"
    hook "stop while held" Stop stop
    expect "the stop is refused" '"decision":"block"'
    expect "what comes next" "the test must fail on the code as it is, committed alone before any fix; reverting a fix does not count · next: commit its tests alone, with no code, and see them fail"
    expect "the teaching line, with the plan's command" "write a test that fails on the code as it is now; familiar-ai runs sh tests/test_c1.sh after each commit"
    expect "and that a second pass is a failed try" "a second run that passes counts as a failed try; two in a row and the person decides"
    # a seal whose run passed proves nothing, so editing the
    # test is no proof_tampered; the next tests-only commit is a fresh seal attempt,
    # and a second pass is a second failed attempt in a row: two_failures
    hook "edit the test that never went red" PreToolUse pre_tool_use_edit "file_path=$PROJECT/tests/test_c1.sh"
    expect "is allowed, told what the test is for" '"additionalContext":"familiar-ai: tests/test_c1.sh is the planned test of \"greet says hello\"'
    case "$OUT" in *deny*) fail "the edit of an unproven seal goes" "no deny" ;; *) pass "the edit of an unproven seal goes" ;; esac
    printf 'grep -q h src/app.sh\n' > "$PROJECT/tests/test_c1.sh"
    git_in add -A && git_in commit -q -m "c1: a second test that passes"
    hook "the second tests-first run passed too" PostToolUse post_tool_use_bash "command=git add -A && git commit -m 'c1: tests again'"
    cli status
    expect "two runs in a row that passed hold two_tries_not_red" "two tries to go red failed; this needs you"
    case "$OUT" in *"the test was changed after it was sealed"*) fail "no proof_tampered on an unproven seal" "no: the test was changed after it was sealed" ;; *) pass "no proof_tampered on an unproven seal" ;; esac
    hook "stop while held on two_tries_not_red" Stop stop
    expect "the stop is refused" '"decision":"block"'
    # D5 (the owner's announced definition change, 2026-10-16): no investigator for two_tries_not_red
    expect "the stop names the person's card" "the person answers its card: try again, plan it again or release it"
    case "$OUT" in *"stronger model"*) fail "no stronger model offered" "a stronger model named" ;; *) pass "no stronger model offered" ;; esac
    hook "the holder spawns a subagent" PreToolUse pre_tool_use_agent_spawn_on_haiku "@session_id=$SESSION"
    case "$OUT" in *'"subagent_type":"familiar-ai:investigator"'*) fail "the spawn is not made an investigator" "rewritten" ;; *) pass "the spawn is not made an investigator" ;; esac
}

# ---------------------------------------------------------------------------
scenario_missing_command() {
    fresh_project notrun
    plan_the_project
    mkdir -p "$PROJECT/tests"
    printf 'pytset -q\n' > "$PROJECT/tests/test_c1.sh"
    git_in add -A && git_in commit -q -m "c1: a test whose command is missing"
    hook "the tests-first commit landed; its command was not found" PostToolUse post_tool_use_bash "command=git add -A && git commit -m 'c1: tests first'"
    expect "the line says the command did not run, with its stderr" "familiar-ai: the test command did not run: tests/test_c1.sh:"
    expect "naming the missing command" "pytset"
    expect "and the way out" " · next: fix the command or the environment"
}

# ---------------------------------------------------------------------------
scenario_person_only_holds() {
    fresh_project card
    claim intent_submitted --for 0 --text "greet with hello"
    # definition change (the owner, 2026-10-11): light is for documentation only; a code anchor is the full path
    claim behavior --behavior_id b1 --text "the greeting"
    claim promise --promise_id p1 --behavior_id b1 --accepts "a name" --gives_back "a greeting" --refuses "nothing" --cases '[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]'
    claim criterion --criterion_id c1 --text "greet says hello" --path full --behavior_id b1 --promise_id p1
    cases_kept c1  # the cases card passed (plan cases-card, 2026-10-25) [draft names]
    claim place_chosen --criterion_id c1 --places '[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]'
    claim test_planned --criterion_id c1 --file_path tests/test_c1.sh --will create
    claim test_stack_detected --project_root . --test_stack sh --test_command "sh tests/test_c1.sh"
    claim criterion_head --criterion_id c1
    # someone edits the anchored lines before the plan is confirmed: the start check sees it
    printf '#!/bin/sh\ngreet() { echo hey; }\ngreet\n' > "$PROJECT/src/app.sh"
    claim criterion_assigned --criterion_id c1
    claim fix_location_category --criterion_id c1 --category this_repository --where ''
    expect "anchor_moved holds" "familiar-ai: \"greet says hello\" · the code at the plan's place changed after the plan was made"
    expect "the card is shown" "familiar-ai: a question for you · \"greet says hello\": the code at src/app.sh greet changed after the plan was made. What now?"
    hook "stop while anchor_moved" Stop stop
    expect "the stop is refused" '"decision":"block"'
    expect "the person is asked with AskUserQuestion" "Ask the person with AskUserQuestion: the hook turns that call into the question card"
    case "$OUT" in *"familiar-ai claim hold_answered"*) fail "the agent is handed no claim of the person's" "no hold_answered claim line" ;; *) pass "the agent is handed no claim of the person's" ;; esac
    # the agent answers from its own shell (CLAUDE_CODE_SESSION_ID is set here, as in Claude Code's Bash tool)
    cli claim hold_answered --criterion_id c1 --hold anchor_moved --choice keep_going --typed ''
    exits "the agent's answer is refused" 1
    expect "the line names the person and the command" "this card is the person's to answer: ask the person with AskUserQuestion; the hook turns that call into the question card and records their pick (in a terminal of their own the person can also run: familiar-ai claim hold_answered --criterion_id c1 --hold anchor_moved --choice keep_going --typed '')"
    expect "the card's question follows" "familiar-ai: a question for you · \"greet says hello\": the code at src/app.sh greet changed after the plan was made. What now?"
    # the choices in the person's world
    expect "and its choices" "  what now?
    1) carry on with part 1 on the code as it is now
    2) have the agent pick the code for part 1 again
    3) drop it"
    case "$OUT" in *"--as-person"*) fail "no flag to paste" "no --as-person" ;; *) pass "no flag to paste" ;; esac
    cli claim hold_answered --criterion_id c1 --hold not_stamped --choice keep_going --typed ''
    exits "a hold that is not a card is refused" 1
    expect "the line names the card holds" "hold_answered answers a card, and not_stamped is not a card hold; the card holds are anchor_moved, two_failures, two_tries_not_red, fix_elsewhere, anchor_untouched, two_test_broken, two_crashed, two_timed_out, environment_error, run_unclassified, retry_spent, crashed, command_broke, miscoped, reading_gap, setup_changed, review_disputed, cases_unconfirmed and no_test_stack"
    hook "stop, still held" Stop stop
    expect "nothing was written: still anchor_moved" "the code at the plan's place changed after the plan was made · next: the person answers its card"
    # the person, in a terminal of their own: no CLAUDE_CODE_SESSION_ID
    person_cli claim hold_answered --criterion_id c1 --hold anchor_moved --choice keep_going --typed ''
    exits "the person's answer is accepted" 0
    expect "c1 is testing again" "familiar-ai: \"greet says hello\" · the test comes first"
    hook "stop after the answer" Stop stop
    expect "the hold is gone: a not-yet" "familiar-ai: not done yet · "
    expect "the test comes first" " · the test comes first"
    # a release, by the person: the line says so
    person_cli claim hold_answered --criterion_id c1 --hold two_failures --choice release --typed ''
    expect "released by the person" "familiar-ai: \"greet says hello\" · released by you"
    hook "stop after the release" Stop stop
    expect "released: the agent may stop" "{}"
}

# ---------------------------------------------------------------------------
scenario_fix_beside_anchor() {
    fresh_project untouched
    printf 'echo other\n' > "$PROJECT/src/other.sh"
    git_in add -A && git_in commit -q -m "a second file"
    claim intent_submitted --for 0 --text "greet with hello"
    # definition change (the owner, 2026-10-11): light is for documentation only; a code anchor is the full path
    claim behavior --behavior_id b1 --text "the greeting"
    claim promise --promise_id p1 --behavior_id b1 --accepts "a name" --gives_back "a greeting" --refuses "nothing" --cases '[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]'
    claim criterion --criterion_id c1 --text "greet says hello" --path full --behavior_id b1 --promise_id p1
    cases_kept c1  # the cases card passed (plan cases-card, 2026-10-25) [draft names]
    # the anchor is the greet function alone: line 2 of src/app.sh
    claim place_chosen --criterion_id c1 --places '[{"file_path":"src/app.sh","symbol":"greet","line_start":2,"line_end":2}]'
    claim test_planned --criterion_id c1 --file_path tests/test_c1.sh --will create
    claim test_stack_detected --project_root . --test_stack sh --test_command "sh tests/test_c1.sh"
    claim criterion_head --criterion_id c1
    claim criterion_assigned --criterion_id c1
    claim fix_location_category --criterion_id c1 --category this_repository --where ''
    expect "the plan is confirmed" "familiar-ai: the plan is confirmed"
    mkdir -p "$PROJECT/tests"
    printf 'grep -q hello src/app.sh\n' > "$PROJECT/tests/test_c1.sh"
    git_in add -A && git_in commit -q -m "c1: tests first"
    hook "the tests-first commit landed" PostToolUse post_tool_use_bash "command=git add -A && git commit -m 'c1: tests first'"
    expect "sealed" "they fail, as they should before the fix · stamped"
    # the fix outside the anchor's lines (line 3, and another file)
    printf '#!/bin/sh\ngreet() { echo hi; }\ngreet; echo hello\n' > "$PROJECT/src/app.sh"
    printf 'echo other, too\n' > "$PROJECT/src/other.sh"
    git_in add -A && git_in commit -q -m "c1: the fix, beside the anchor"
    hook "the code commit landed, beside the anchor" PostToolUse post_tool_use_bash "command=git commit -am 'c1: the fix'"
    expect "the hold" "the change landed outside the place the plan named"
    hook "stop while anchor_untouched" Stop stop
    expect "the stop is refused" '"decision":"block"'
    expect "the card is the person's" "a question for the person is open: ask them now with AskUserQuestion"
    expect "the hold's words and what comes next" "the change landed outside the place the plan named · next: the person answers its card: plan it again, keep going as planned or release it; or choose its place in the code again and have the lines checked"
    expect "the card's question" "its tests pass, but no commit changed the code at src/app.sh greet. What now?"
    expect "the line naming what the commits did change against the anchor" "the commits since the seal changed src/app.sh (and 1 more); the place the plan named is src/app.sh greet 2–2"
    expect "and the re-anchor's command" "claim it: familiar-ai claim place_chosen --criterion_id c1 --places"
}

# ---------------------------------------------------------------------------
# every link names its string's head; the links take their turns in the order
# claimed (the owner, 2026-10-15): line 9's claims are pasted as printed

# the claim on the checklist's line 9 in `familiar-ai status`, into $LINE_9
line_9_claim() {
    cli status
    LINE_9=$(printf '%s\n' "$OUT" | sed -n 's/^    9 the string      claim it: //p')
    OUT=$LINE_9
}

# each criterion's state in `familiar-ai status --json`, as "c1=testing c2=waiting", into $OUT
turns() {
    cli status --json
    OUT=$(printf '%s' "$OUT" | python3 -c 'import json, sys; print(" ".join(c["id"] + "=" + c["state_name"] for c in json.load(sys.stdin)["criteria"]))')
}

# one part's journey: its test committed alone (sealed), then its code (green)
seal_then_green() {
    printf 'grep -q "echo %s" src/app.sh\n' "$2" > "$PROJECT/tests/$1"
    git_in add -A && git_in commit -q -m "$1: tests first"
    hook "$1: the tests-first commit" PostToolUse post_tool_use_bash "command=git commit -m '$1: tests first'"
    expect "$1 fails first: sealed" "they fail, as they should before the fix · stamped"
    printf '#!/bin/sh\ngreet() { echo %s; }\ngreet\n' "$2" > "$PROJECT/src/app.sh"
    git_in add -A && git_in commit -q -m "$1: code"
    hook "$1: the code commit" PostToolUse post_tool_use_bash "command=git commit -am '$1: code'"
    expect "$1 passes: green" "they pass · stamped"
}

scenario_three_parts() {
    fresh_project three
    mkdir -p "$PROJECT/tests"
    claim intent_submitted --for 0 --text "greet with hello, then hello there, then hello there friend"
    claim behavior --behavior_id b1 --text "the greeting"
    claim promise --promise_id p1 --behavior_id b1 --accepts "a name" --gives_back "a greeting" --refuses "nothing" --cases '[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]'
    for part in "c1 hello test_hello.sh" "c2 hello there test_hello_there.sh" "c3 hello there friend test_friend.sh"; do
        c=${part%% *}; file=${part##* }; words=${part#* }; words=${words% *}
        claim criterion --criterion_id "$c" --text "greet says $words" --path full --behavior_id b1 --promise_id p1
        cases_kept "$c"  # the cases card passed (plan cases-card, 2026-10-25) [draft names]
        claim place_chosen --criterion_id "$c" --places '[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]'
        claim test_planned --criterion_id "$c" --file_path "tests/$file" --will create --test_command "sh tests/$file"
        claim criterion_assigned --criterion_id "$c"
        claim fix_location_category --criterion_id "$c" --category this_repository --where ''
    done
    claim test_stack_detected --project_root . --test_stack sh --test_command "sh tests/test_hello.sh"
    claim criterion_head --criterion_id c1

    line_9_claim
    exactly "line 9 for the second part names the head" "familiar-ai claim criterion_strung_on_head --criterion_id c2 --head c1"
    eval "cli ${LINE_9#familiar-ai }"
    exits "the second part's line 9, pasted as printed, is claimed" 0
    line_9_claim
    exactly "line 9 for the third part names the head too, not the second part" "familiar-ai claim criterion_strung_on_head --criterion_id c3 --head c1"
    cli claim criterion_strung_on_head --criterion_id c3 --head c2
    exits "a link named as the head is refused at the claim" 1
    expect "the refusal names the head" "c2 is not a head; the head of its string is c1: --head c1"
    eval "cli ${LINE_9#familiar-ai }"
    exits "the third part's line 9, pasted as printed, is claimed" 0
    expect "the plan is confirmed" "familiar-ai: the plan is confirmed"

    turns
    exactly "the first part's turn; the others wait" "c1=testing c2=waiting c3=waiting"
    seal_then_green test_hello.sh hello
    turns
    exactly "the second part's turn comes after the first" "c1=green c2=testing c3=waiting"
    seal_then_green test_hello_there.sh "hello there"
    turns
    exactly "the third part's turn comes after the second" "c1=green c2=green c3=testing"
    seal_then_green test_friend.sh "hello there friend"
    turns
    exactly "all three green" "c1=green c2=green c3=green"
}

. "$ROOT/plugin/tests/test-agent-investigator.sh"
. "$ROOT/plugin/tests/test-agent-judge.sh"
. "$ROOT/plugin/tests/test-agent-person.sh"
. "$ROOT/plugin/tests/test-agent-prove.sh"

run_scenario "planning" scenario_planning_guidance
run_scenario "the right path" scenario_tests_first
run_scenario "the wrong path: tests and code in one commit" scenario_mixed_commit
run_scenario "the removal: the plan committed, then rm -rf .familiar" scenario_removed_state
run_scenario "the escape hatch: the plan never committed" scenario_uncommitted_state
run_scenario "the subfolder: the anchor in pkg/, the stack claimed with ." scenario_subfolder_root
run_scenario "the shell write" scenario_shell_writes
run_scenario "the full path" scenario_full_path
run_scenario "tests not red" scenario_passing_test
run_scenario "the command did not run" scenario_missing_command
run_scenario "the person's card" scenario_person_only_holds
run_scenario "anchor untouched" scenario_fix_beside_anchor
run_scenario "the investigator" scenario_investigator
run_scenario "rejected test can be replaced and judged again" scenario_judge_rejected
run_scenario "invalid judge response leaves the seal unjudged" scenario_judge_invalid
run_scenario "the person's words" scenario_person
run_scenario "the call to action" scenario_prove
run_scenario "three parts on one string, line 9 pasted as printed" scenario_three_parts

# ---------------------------------------------------------------------------
printf '\n'
if [ "$FAILS" = 0 ]; then
    printf 'ALL PASS\n'
    exit 0
else
    printf '%d expectation(s) FAILED\n' "$FAILS"
    exit 1
fi
