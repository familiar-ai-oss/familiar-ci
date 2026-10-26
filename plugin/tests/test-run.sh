#!/bin/sh
# End-to-end hook tests: each scenario drives plugin/hooks/run.sh with Claude Code
# payloads, with no binary, a fake one or the built one, in temporary git repositories.
# Build with `cargo build -p familiar-ai`, then from the repository root: sh plugin/tests/test-run.sh (VERBOSE=1 for detail).

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
RUN_SH="$ROOT/plugin/hooks/run.sh"

# The binary under test: FAMILIAR_AI_BIN, else familiar-ai on PATH, else the debug build.
BIN=${FAMILIAR_AI_BIN:-$(command -v familiar-ai)}
if [ -z "$BIN" ]; then
    BIN="$ROOT/target/debug/familiar-ai"
fi
printf 'familiar-ai: %s\n' "$BIN"

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
unset CLAUDE_PROJECT_DIR
# a headless runner (claude -p) is not the walk's session: it would count as the bench
unset CLAUDE_CODE_ENTRYPOINT
unset CLAUDE_CODE_SESSION_ID
# the setup markers of this run stay in the scratch folder, never the real home
export FAMILIAR_AI_HOME="$TMP/home"
# Any hook call may ask the judge or start an investigator; only the judge scenario asks one, a fake one.
export FAMILIAR_AI_INVESTIGATOR=off
export FAMILIAR_AI_JUDGE=off

# a PATH with no familiar-ai on it
mkdir -p "$TMP/emptybin"
NOFA_PATH="$TMP/emptybin:/usr/bin:/bin"

# a fake familiar-ai first on PATH: its arguments on line 1, its stdin on
# line 2, and the exit code in FAKE_EXIT (default 0)
mkdir -p "$TMP/fakebin"
cat > "$TMP/fakebin/familiar-ai" <<'EOF'
#!/bin/sh
printf 'args:%s\n' "$*"
cat
printf '\n'
exit "${FAKE_EXIT:-0}"
EOF
chmod +x "$TMP/fakebin/familiar-ai"
FAKE_PATH="$TMP/fakebin:/usr/bin:/bin"

# the binary under test first on PATH
mkdir -p "$TMP/realbin"
ln -s "$BIN" "$TMP/realbin/familiar-ai"
REAL_PATH="$TMP/realbin:/usr/bin:/bin"

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
HOOK_PATH=$NOFA_PATH

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

# expect_no <label> <text that must not be in $OUT>
expect_no() {
    case "$OUT" in
        *"$2"*) fail "$1" "not to contain: $2" ;;
        *) pass "$1" ;;
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

# like <label> <shell pattern the whole of $OUT matches>
like() {
    case "$OUT" in
        $2) pass "$1" ;;
        *) fail "$1" "to match: $2" ;;
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

# field <key>: the key's value in the hook's JSON (the last line of $OUT), at
# the top level or in hookSpecificOutput, as the JSON writes it (a string
# without its quotes); <absent> when the key is not there, <not JSON> when the
# line is not a JSON object
field() {
    printf '%s\n' "$OUT" | python3 -c '
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
' "$1"
}

# field_is <label> <key> <value>
field_is() {
    value=$(field "$2")
    if [ "$value" = "$3" ]; then
        pass "$1"
    else
        fail "$1" "$2 = $3"
    fi
}

# field_like <label> <key> <shell pattern the value matches>
field_like() {
    value=$(field "$2")
    case "$value" in
        $3) pass "$1" ;;
        *) fail "$1" "$2 matching: $3" ;;
    esac
}

# field_absent <label> <key>
field_absent() {
    value=$(field "$2")
    if [ "$value" = "<absent>" ]; then
        pass "$1"
    else
        fail "$1" "no $2"
    fi
}

# denied <label>: the tool call is refused by Claude Code's hook protocol:
# exit 0 with permissionDecision deny
denied() {
    value=$(field permissionDecision)
    if [ "$RC" = 0 ] && [ "$value" = deny ]; then
        pass "$1"
    else
        fail "$1" "exit 0 and permissionDecision = deny, got exit $RC and permissionDecision = $value"
    fi
}

# allowed <label>: exit 0 with no permissionDecision
allowed() {
    value=$(field permissionDecision)
    if [ "$RC" = 0 ] && { [ -z "$OUT" ] || [ "$value" = "<absent>" ]; }; then
        pass "$1"
    else
        fail "$1" "exit 0 and no permissionDecision, got exit $RC and permissionDecision = $value"
    fi
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

# hook <label> <Event> <json> [VAR=value ...]: the JSON piped to run.sh as
# Claude Code pipes it, with $HOOK_PATH as PATH
hook() {
    label=$1
    event=$2
    json=$3
    shift 3
    if [ "${#json}" -gt 4000 ]; then
        LAST_INPUT="hook $event ($label) a JSON of ${#json} bytes"
    else
        LAST_INPUT="hook $event ($label) $json"
    fi
    OUT=$(printf '%s' "$json" | env PATH="$HOOK_PATH" "$@" /bin/sh "$RUN_SH" "$event" 2>"$TMP/err")
    RC=$?
    LAST_ERR=$(cat "$TMP/err")
    if verbose; then
        printf '\n### %s: hook %s, exit %s\n%s\n' "$label" "$event" "$RC" "$OUT"
        if [ -n "$LAST_ERR" ]; then
            printf '(stderr) %s\n' "$LAST_ERR"
        fi
    fi
}

# bash_call <command>: a PreToolUse payload for the Bash tool
bash_call() {
    printf '{"tool_name":"Bash","tool_input":{"command":"%s"}}' "$1"
}

# escape backslashes and quotes for embedding in JSON
json_escape() {
    printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

# payload <Event> <the rest of the object>: a hook payload whose cwd is the project
payload() {
    printf '{"hook_event_name":"%s","session_id":"s1","cwd":"%s"%s}' "$1" "$PROJECT" "$2"
}

# cli <args>: `familiar-ai <args>` in the project, from a person's terminal
cli() {
    LAST_INPUT="familiar-ai $*"
    LAST_ERR=""
    OUT=$(cd "$PROJECT" && env PATH="$HOOK_PATH" familiar-ai "$@" 2>&1)
    RC=$?
    if verbose; then
        printf '\n$ familiar-ai %s, exit %s\n%s\n' "$*" "$RC" "$OUT"
    fi
    return "$RC"
}

# agent_cli <args>: the same from the agent's shell, which carries the session id
agent_cli() {
    LAST_INPUT="(the agent's shell) familiar-ai $*"
    LAST_ERR=""
    OUT=$(cd "$PROJECT" && env PATH="$HOOK_PATH" CLAUDE_CODE_SESSION_ID=s1 familiar-ai "$@" 2>&1)
    RC=$?
    if verbose; then
        printf '\n$ familiar-ai %s (the agent'"'"'s shell), exit %s\n%s\n' "$*" "$RC" "$OUT"
    fi
    return "$RC"
}

git_in() {
    git -C "$PROJECT" -c user.name=t -c user.email=t@example.com -c commit.gpgsign=false "$@"
}

need_binary() {
    need "a built familiar-ai at $BIN (cargo build)" test -x "$BIN"
}

# set_up_project <name>: a git repository set up by `familiar-ai setup`
set_up_project() {
    PROJECT="$TMP/$1"
    HOOK_PATH=$REAL_PATH
    mkdir -p "$PROJECT"
    need "git init" git_in init -q -b main || return 1
    need "familiar-ai setup" cli setup
}

# finder_answer <the spawn's PreToolUse reply>: the PostToolUse on the Agent call that released
# the case-finder [draft names] (plan cases-card §1b, in the shape of the spike's capture of
# 2026-10-25): the spawn as the hook rewrote it, and the finder's final text, a fenced JSON answer
# with no cases and why
finder_answer() {
    python3 - "$1" "$PROJECT" <<'EOF'
import json, sys
reply = json.loads(sys.argv[1] or "{}")
spawn = reply.get("hookSpecificOutput", {}).get("updatedInput") or {}
answer = {"cases": [], "none_because": "the planned cases cover what the person asked"}
print(json.dumps({
    "session_id": "s1", "cwd": sys.argv[2], "hook_event_name": "PostToolUse", "permission_mode": "bypassPermissions",
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
# agent's next tool call is told so. A setup step: returns 1 with STOPPED naming what differed.
cases_kept() {
    hook "release the case-finder for $1" PreToolUse "$(payload PreToolUse ',"tool_name":"Agent","tool_input":{"description":"look for more cases","prompt":"look for more cases","subagent_type":"general-purpose"}')"
    case "$OUT" in *'"subagent_type":"familiar-ai:case-finder"'*) ;; *) STOPPED="the case-finder's spawn for $1: $OUT"; return 1 ;; esac
    hook "the case-finder answered for $1" PostToolUse "$(finder_answer "$OUT")"
    agent_cli status --json
    case "$OUT" in *'"cases_unconfirmed"'*) ;; *) STOPPED="the cases card for $1: $OUT"; return 1 ;; esac
    cli claim hold_answered --criterion_id "$1" --hold cases_unconfirmed --choice keep_planned --typed '' || { STOPPED="the person keeps the planned cases for $1: $OUT"; return 1; }
    hook "the next tool call after the cases card of $1" PostToolUse "$(payload PostToolUse ',"tool_name":"Bash","tool_input":{"command":"ls"},"tool_response":{}')"
    case "$OUT" in *"the person answered the question for"*) ;; *) STOPPED="the agent told the person's answer for $1: $OUT"; return 1 ;; esac
}

# plan_the_project: src/app.sh committed, then the plan claimed line by line
plan_the_project() {
    mkdir -p "$PROJECT/src"
    printf '#!/bin/sh\ngreet() { echo hi; }\ngreet\n' > "$PROJECT/src/app.sh"
    need "git add" git_in add -A || return 1
    need "the base commit" git_in commit -q -m base || return 1
    need "claim intent_submitted" agent_cli claim intent_submitted --for 0 --text "greet with hello" || return 1
    # definition change (the owner, 2026-10-11): light is for documentation only; a code anchor is the full path
    need "claim behavior" agent_cli claim behavior --behavior_id b1 --text "the greeting" || return 1
    need "claim promise" agent_cli claim promise --promise_id p1 --behavior_id b1 --accepts "a name" --gives_back "a greeting" --refuses "nothing" --cases '[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]' || return 1
    need "claim criterion" agent_cli claim criterion --criterion_id c1 --text "greet says hello" --path full --behavior_id b1 --promise_id p1 || return 1
    cases_kept c1 || return 1  # the cases card passed (plan cases-card, 2026-10-25) [draft names]
    need "claim place_chosen" agent_cli claim place_chosen --criterion_id c1 --places '[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]' || return 1
    need "claim test_planned" agent_cli claim test_planned --criterion_id c1 --file_path tests/test_c1.sh --will create || return 1
    need "claim test_stack_detected" agent_cli claim test_stack_detected --project_root . --test_stack sh --test_command "sh tests/test_c1.sh" || return 1
    need "claim criterion_head" agent_cli claim criterion_head --criterion_id c1 || return 1
    need "claim criterion_assigned" agent_cli claim criterion_assigned --criterion_id c1 || return 1
    need "claim fix_location_category" agent_cli claim fix_location_category --criterion_id c1 --category this_repository --where ''
}

# commit_the_tests: a failing test for c1, committed alone
commit_the_tests() {
    mkdir -p "$PROJECT/tests"
    printf 'grep -q hello src/app.sh\n' > "$PROJECT/tests/test_c1.sh"
    git_in add -A || return 1
    git_in commit -q -m "c1: tests first"
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

# the setup skill's install commands, and one with a command in its version field
INSTALL_CMD='curl -fsSL https://github.com/familiar-ai-oss/familiar-ci/releases/download/v0.1.0/familiar-ai-x86_64-unknown-linux-gnu.tar.gz -o /tmp/familiar-ai-x86_64-unknown-linux-gnu.tar.gz && curl -fsSL https://github.com/familiar-ai-oss/familiar-ci/releases/download/v0.1.0/familiar-ai-x86_64-unknown-linux-gnu.tar.gz.sha256 -o /tmp/familiar-ai-x86_64-unknown-linux-gnu.tar.gz.sha256 && cd /tmp && sha256sum -c familiar-ai-x86_64-unknown-linux-gnu.tar.gz.sha256 && mkdir -p "$HOME/.local/bin" && tar xzf familiar-ai-x86_64-unknown-linux-gnu.tar.gz -C "$HOME/.local/bin" && chmod +x "$HOME/.local/bin/familiar-ai"'
INSTALL_MAC='curl -fsSL https://github.com/familiar-ai-oss/familiar-ci/releases/download/v0.1.0/familiar-ai-aarch64-apple-darwin.tar.gz -o /tmp/familiar-ai-aarch64-apple-darwin.tar.gz && curl -fsSL https://github.com/familiar-ai-oss/familiar-ci/releases/download/v0.1.0/familiar-ai-aarch64-apple-darwin.tar.gz.sha256 -o /tmp/familiar-ai-aarch64-apple-darwin.tar.gz.sha256 && cd /tmp && shasum -a 256 -c familiar-ai-aarch64-apple-darwin.tar.gz.sha256 && mkdir -p "$HOME/.local/bin" && tar xzf familiar-ai-aarch64-apple-darwin.tar.gz -C "$HOME/.local/bin" && chmod +x "$HOME/.local/bin/familiar-ai"'
EVIL='curl -fsSL https://github.com/familiar-ai-oss/familiar-ci/releases/download/v0.1.0;rm -rf ~/familiar-ai-x86_64-unknown-linux-gnu.tar.gz -o /tmp/familiar-ai-x86_64-unknown-linux-gnu.tar.gz && curl -fsSL https://github.com/familiar-ai-oss/familiar-ci/releases/download/v0.1.0/familiar-ai-x86_64-unknown-linux-gnu.tar.gz.sha256 -o /tmp/familiar-ai-x86_64-unknown-linux-gnu.tar.gz.sha256 && cd /tmp && sha256sum -c familiar-ai-x86_64-unknown-linux-gnu.tar.gz.sha256 && mkdir -p "$HOME/.local/bin" && tar xzf familiar-ai-x86_64-unknown-linux-gnu.tar.gz -C "$HOME/.local/bin" && chmod +x "$HOME/.local/bin/familiar-ai"'

scenario_missing_binary_refuses() {
    HOOK_PATH=$NOFA_PATH
    hook "Edit" PreToolUse '{"tool_name":"Edit","tool_input":{"file_path":"/tmp/x"}}'
    denied "edit denied"
    field_like "edit denial names the setup skill" permissionDecisionReason "*/familiar-ai:setup*"
    hook "install, then rm" PreToolUse "$(bash_call "$(json_escape "$INSTALL_CMD"); rm -rf x")"
    denied "install with a command appended denied"
    hook "a command in the version" PreToolUse "$(bash_call "$(json_escape "$EVIL")")"
    denied "install with a command in the version denied"
    hook "ls -la" PreToolUse "$(bash_call "ls -la")"
    denied "other shell command denied"
    hook "a Write quoting a setup command" PreToolUse '{"tool_name":"Write","tool_input":{"file_path":"Bash","content":"\"tool_name\":\"Bash\",\"command\":\"uname -s\""}}'
    denied "Write quoting a setup command denied"
    hook "a Write naming the question tool" PreToolUse '{"tool_name":"Write","tool_input":{"file_path":"x","y":{"tool_name":"AskUserQuestion"}}}'
    denied "Write naming the question tool denied"
}

scenario_missing_binary_allows_setup() {
    HOOK_PATH=$NOFA_PATH
    hook "install" PreToolUse "$(bash_call "$(json_escape "$INSTALL_CMD")")"
    allowed "Linux install allowed"
    hook "macOS install" PreToolUse "$(bash_call "$(json_escape "$INSTALL_MAC")")"
    allowed "macOS install allowed"
    # The setup commands are matched whole, so nothing appended or inserted runs unchecked.
    for command in "uname -s" "uname -m" "cargo install familiar-ai" "familiar-ai --version" "familiar-ai setup"; do
        hook "$command" PreToolUse "$(bash_call "$command")"
        allowed "setup command allowed whole: $command"
        for changed in "$command; rm -rf x" "$command && rm -rf x" "$command | sh" " $command" "$command " "sudo $command" "$command\\nrm -rf x" "$command \$(rm -rf x)"; do
            hook "$changed" PreToolUse "$(bash_call "$changed")"
            denied "setup command with an addition denied: $changed"
        done
    done
    hook "AskUserQuestion" PreToolUse '{"session_id":"s","tool_name":"AskUserQuestion","tool_input":{"questions":[{"question":"Continue?","header":"x","options":[],"multiSelect":false}]}}'
    allowed "question tool allowed"
    hook "setup command, fields reordered" PreToolUse '{"tool_input":{"command":"uname -s"},"tool_name":"Bash"}'
    allowed "setup command read by key"
    hook "setup command as a description" PreToolUse '{"tool_name":"Bash","tool_input":{"command":"ls","description":"uname -s"}}'
    denied "setup command in another field denied"
    hook "setup command nested" PreToolUse '{"tool_name":"Bash","tool_input":{"description":{"command":"uname -s"}}}'
    denied "nested setup command denied"
    hook "question tool, fields reordered" PreToolUse '{"tool_input":{"questions":[]},"session_id":"s","tool_name":"AskUserQuestion"}'
    allowed "question tool read by key"
}

scenario_missing_binary_never_blocks() {
    HOOK_PATH=$NOFA_PATH
    hook "Stop" Stop '{}'
    exits "stop exits 0" 0
    hook "UserPromptSubmit" UserPromptSubmit '{"prompt":"hello"}'
    exits "prompt exits 0" 0
    field_like "prompt shows the setup skill" systemMessage "*/familiar-ai:setup*"
    hook "SessionStart" SessionStart '{"source":"startup"}'
    exits "session start exits 0" 0
    hook "PostToolUse" PostToolUse '{"tool_name":"Edit"}'
    exits "post-tool hook exits 0" 0
    hook "Stop" Stop '{}'
    field_like "stop says not installed, names the setup skill" systemMessage "*not installed*/familiar-ai:setup*"
    field_absent "stop not blocked" decision
}

scenario_long_commands() {
    HOOK_PATH=$NOFA_PATH
    # A long command must be refused well inside the hook's time limit.
    long=$(head -c 200000 /dev/zero | tr '\0' 'a')
    start=$(date +%s)
    hook "a 200 kB command" PreToolUse "$(bash_call "$long")"
    took=$(($(date +%s) - start))
    denied "200 kB command denied"
    check "200 kB command answered in ${took}s" "under 5 s" [ "$took" -lt 5 ]
    mid=$(head -c 7000 /dev/zero | tr '\0' 'a')
    start=$(date +%s)
    hook "a 7 kB command, under the length cut-off" PreToolUse "$(bash_call "$mid")"
    took=$(($(date +%s) - start))
    denied "7 kB command denied"
    check "7 kB command answered in ${took}s" "under 5 s" [ "$took" -lt 5 ]
}

scenario_refusal_protocol() {
    HOOK_PATH=$NOFA_PATH
    # Nothing on stderr, so the agent is shown only the reason.
    hook "Edit" PreToolUse '{"tool_name":"Edit","tool_input":{"file_path":"/tmp/x"}}'
    exits "refusal exits 0" 0
    field_is "refusal names its event" hookEventName PreToolUse
    field_is "refusal is a deny" permissionDecision deny
    field_like "refusal reason names the setup skill" permissionDecisionReason "*/familiar-ai:setup*"
    check "refusal writes nothing on stderr" "no stderr" [ -z "$LAST_ERR" ]
    hook "uname -s" PreToolUse "$(bash_call "uname -s")"
    exits "allowed setup command exits 0" 0
    exactly "allowed setup command prints no decision" ""
}

scenario_events_relayed() {
    HOOK_PATH=$FAKE_PATH
    for event in SessionStart UserPromptSubmit PreToolUse PostToolUse Stop SubagentStop SubagentStart PreModelSwitch PostModelSwitch; do
        body="{\"hook_event_name\":\"$event\",\"probe\":\"stdin-$event\"}"
        hook "$event" "$event" "$body"
        exits "$event exits 0" 0
        exactly "$event reaches the binary as hook $event with its stdin" "args:hook $event
$body"
    done
    hook "binary exit 2" PreToolUse '{}' FAKE_EXIT=2
    exits "binary exit 2 passed on" 2
}

scenario_failing_binary() {
    HOOK_PATH=$FAKE_PATH
    # Claude Code lets a tool call through on any exit but 0 or 2.
    hook "binary exit 7" PreToolUse '{}' FAKE_EXIT=7
    denied "binary exit 7 denies the tool call"
    hook "binary exit 7" Stop '{}' FAKE_EXIT=7
    exits "binary exit 7 on stop exits 0" 0
    field_absent "binary exit 7 does not block the stop" decision
    mkdir -p "$TMP/broken"
    printf 'this is not a program\n' > "$TMP/broken/familiar-ai"
    chmod +x "$TMP/broken/familiar-ai"
    HOOK_PATH="$TMP/broken:/nonexistent"
    hook "a binary that cannot run" PreToolUse '{"tool_name":"Edit","tool_input":{"file_path":"/tmp/x"}}'
    denied "binary that cannot run denies the edit"
}

edit_call() {
    payload PreToolUse ",\"tool_name\":\"Edit\",\"tool_input\":{\"file_path\":\"$PROJECT/a.txt\"}"
}

# bash_in_project <command>: a PreToolUse payload for the Bash tool in the project
bash_in_project() {
    payload PreToolUse ",\"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"$1\"}"
}

scenario_unset_project() {
    need_binary || return
    PROJECT="$TMP/unset"
    HOOK_PATH=$REAL_PATH
    mkdir -p "$PROJECT"
    need "git init" git_in init -q -b main || return
    for event in UserPromptSubmit PreToolUse PostToolUse Stop SubagentStop SubagentStart PreModelSwitch PostModelSwitch; do
        hook "$event" "$event" "$(payload "$event" ",\"tool_name\":\"Edit\",\"tool_input\":{\"file_path\":\"$PROJECT/a.txt\"}")"
        exits "$event exits 0" 0
        exactly "$event answers {}" "{}"
    done
    check "hooks create no .familiar" "no .familiar" [ ! -e "$PROJECT/.familiar" ]
    hook "SessionStart" SessionStart "$(payload SessionStart "")"
    exits "session start exits 0" 0
    field_is "session start names its event" hookEventName SessionStart
    field_like "session start says not set up" additionalContext "*not set up*"
    field_like "session start names the setup command" additionalContext "*familiar-ai setup*"
    field_like "session start names the setup skill" additionalContext "*/familiar-ai:setup*"
    field_absent "session start not blocked" decision
    field_absent "session start makes no permission decision" permissionDecision
    check "session start creates no .familiar" "no .familiar" [ ! -e "$PROJECT/.familiar" ]
}

scenario_setup() {
    need_binary || return
    PROJECT="$TMP/setup"
    HOOK_PATH=$REAL_PATH
    mkdir -p "$PROJECT"
    need "git init" git_in init -q -b main || return
    hook "the setup call" PreToolUse "$(bash_in_project "familiar-ai setup")"
    exactly "setup command allowed before the folder exists" "{}"
    cli setup
    exits "setup exits 0" 0
    like "setup reports each step and its commit" "familiar-ai: made .familiar at $PROJECT
familiar-ai: marked this project as set up at $FAMILIAR_AI_HOME/projects/*
familiar-ai: added .familiar/record to $PROJECT/.gitignore
familiar-ai: added .familiar/judge.log to $PROJECT/.gitignore
familiar-ai: added .familiar/investigator.log to $PROJECT/.gitignore
familiar-ai: added .familiar/runs to $PROJECT/.gitignore
familiar-ai: added .familiar/investigator to $PROJECT/.gitignore
familiar-ai: added .familiar/verify-report.json to $PROJECT/.gitignore
familiar-ai: added the familiar-ai status line to $PROJECT/.claude/settings.json
familiar-ai: added the familiar-ai lines to $PROJECT/CLAUDE.md
familiar-ai: committed .gitignore and .claude/settings.json and CLAUDE.md as one commit \"familiar-ai setup\" (*)"
    marker=$(printf '%s\n' "$OUT" | sed -n 's/^familiar-ai: marked this project as set up at //p')
    LAST_INPUT="the marker $marker"
    OUT=$(cat "$marker" 2>&1)
    root=$(cd "$PROJECT" && pwd -P)
    check "marker holds the project root" "line 1: root: $root" [ "$(sed -n 1p "$marker")" = "root: $root" ]
    check "marker holds the time" "line 2: at: <seconds>" eval 'sed -n 2p "$marker" | grep -q "^at: [0-9][0-9]*$"'
    LAST_INPUT="$PROJECT/.gitignore"
    OUT=$(cat "$PROJECT/.gitignore")
    exactly "setup writes the ignore lines" ".familiar/record
.familiar/judge.log
.familiar/investigator.log
.familiar/runs
.familiar/investigator
.familiar/verify-report.json"
    LAST_INPUT="ls -A .familiar"
    OUT=$(ls -A "$PROJECT/.familiar" 2>&1)
    exactly "setup leaves .familiar empty" ""
    for ignored in .familiar/record .familiar/judge.log .familiar/investigator.log .familiar/runs/0123abc.log .familiar/investigator/u1.jsonl; do
        check "git ignores $ignored" "ignored" git_in check-ignore -q "$ignored"
    done
    if git_in check-ignore -q .familiar/plan; then
        fail "git keeps the plan" "not ignored"
    else
        pass "git keeps the plan"
    fi
    cli setup
    exits "second setup exits 0" 0
    exactly "second setup reports every step already done" "familiar-ai: .familiar is already at $PROJECT
familiar-ai: this project is already marked as set up at $marker
familiar-ai: .familiar/record is already ignored by git
familiar-ai: .familiar/judge.log is already ignored by git
familiar-ai: .familiar/investigator.log is already ignored by git
familiar-ai: .familiar/runs is already ignored by git
familiar-ai: .familiar/investigator is already ignored by git
familiar-ai: .familiar/verify-report.json is already ignored by git
familiar-ai: the familiar-ai status line is already set in $PROJECT/.claude/settings.json
familiar-ai: CLAUDE.md already carries the familiar-ai lines
familiar-ai: already set up: setup has nothing to commit"
    LAST_INPUT="$PROJECT/.gitignore"
    OUT=$(cat "$PROJECT/.gitignore")
    exactly "second setup leaves .gitignore as it was" ".familiar/record
.familiar/judge.log
.familiar/investigator.log
.familiar/runs
.familiar/investigator
.familiar/verify-report.json"
}

scenario_plan_comes_first() {
    need_binary || return
    set_up_project planfirst || return
    hook "edit before the plan" PreToolUse "$(edit_call)"
    denied "edit before the plan denied"
    field_like "denial asks for the intent" permissionDecisionReason "familiar-ai: the plan comes first · line 1: the intent*"
    check "first hook starts the record" "a .familiar/record" [ -f "$PROJECT/.familiar/record" ]
    # A stop with no run open goes: the person may only be talking.
    hook "stop before the plan" Stop "$(payload Stop "")"
    field_absent "stop with no run open not blocked" decision
    hook "SessionStart" SessionStart "$(payload SessionStart "")"
    field_absent "session start not blocked" decision
    field_absent "session start makes no permission decision" permissionDecision
    field_like "session start shows the plan's state" additionalContext "*familiar-ai: the plan is still being written*"
    hook "edit before the plan" PreToolUse "$(edit_call)"
    field_like "denial gives the intent's claim command" permissionDecisionReason "*claim it: familiar-ai claim intent_submitted --text*"
}

scenario_shell_writes() {
    need_binary || return
    set_up_project shellwrite || return
    hook "edit before the plan" PreToolUse "$(edit_call)"
    edit_decision=$(field permissionDecision)
    edit_reason=$(field permissionDecisionReason)
    need "the Edit's denial" test "$edit_decision" = deny || return
    for command in 'cat > a.txt << EOF\nhello\nEOF' "python3 - << 'EOF'\nopen('a.txt', 'w').write('x')\nEOF" 'sed -i s/a/b/ a.txt'; do
        hook "$command" PreToolUse "$(bash_in_project "$command")"
        denied "shell write denied: $command"
        field_is "shell-write denial explains why: $command" permissionDecisionReason "$edit_reason\\n(a shell write counts as an edit)"
    done
    for command in 'echo x > /tmp/familiar-ai-test-run-outside' 'cat a.txt' 'ls -la' "familiar-ai claim intent_submitted --text 'a > b'"; do
        hook "$command" PreToolUse "$(bash_in_project "$command")"
        exactly "command that writes nothing in the project allowed: $command" "{}"
    done
    check "denied shell writes wrote nothing" "no a.txt" [ ! -e "$PROJECT/a.txt" ]
}

scenario_session_start_rulebook() {
    need_binary || return
    set_up_project rulebook || return
    # The rulebook is the binary's alone, so the plan skill carries no copy to drift from it.
    skill="$ROOT/plugin/skills/plan/SKILL.md"
    LAST_INPUT="$skill"
    OUT=$(grep -n 'discipline-begin\|discipline-end' "$skill")
    exactly "plan skill has no rulebook block" ""
    for line in 'Nothing is edited or committed until planning_confirm_complete' 'The record is a database file: read it with familiar-ai status' 'Stopping is refused until every criterion is green, proven or released.'; do
        OUT=$(grep -F "$line" "$skill")
        exactly "plan skill has no rulebook line: $line" ""
    done
    hook "SessionStart" SessionStart "$(payload SessionStart "")"
    context=$(printf '%s' "$OUT" | python3 -c 'import json,sys; print(json.load(sys.stdin)["hookSpecificOutput"]["additionalContext"])')
    mode_line=$(printf '%s\n' "$context" | sed -n 1p)
    rulebook=$(printf '%s\n' "$context" | sed '1,2d' | python3 -c 'import sys; print(sys.stdin.read().split("\n\n")[0])')
    # The mode line may end with the record's CI line.
    case "$mode_line" in
        "familiar-ai: idle · CI: no run for "???????) mode_line="familiar-ai: idle" ;;
    esac
    check "context opens with the idle mode line" "line 1: familiar-ai: idle (the record is empty)" [ "$mode_line" = "familiar-ai: idle" ]
    check "blank line after the mode line" "a blank line 2" [ -z "$(printf '%s\n' "$context" | sed -n 2p)" ]
    case "$rulebook" in
        *'Nothing is edited or committed until planning_confirm_complete'*) pass "rulebook follows the mode line" ;;
        *) fail "rulebook follows the mode line" "the rulebook's first paragraph after the mode line" ;;
    esac
}

scenario_skills_stay_short() {
    skill="$ROOT/plugin/skills/plan/SKILL.md"
    plan_lines=$(wc -l < "$skill")
    LAST_INPUT="$skill"
    OUT="$plan_lines lines"
    check "plan skill at most 70 lines" "at most 70 lines" [ "$plan_lines" -le 70 ]
    description=$(grep -m1 '^description:' "$skill")
    missing=""
    for moment in 'refus' 'card' "judge's no" 'sealed test'; do
        if ! printf '%s' "$description" | grep -qiF "$moment"; then
            missing="$missing [$moment]"
        fi
    done
    OUT=$description
    check "plan skill description names its moments" "names:$missing" [ -z "$missing" ]
    setup_skill="$ROOT/plugin/skills/setup/SKILL.md"
    LAST_INPUT="$setup_skill"
    OUT=$(grep -iF -e "no release exists yet" -e "step 4's download" "$setup_skill")
    exactly "setup skill has no stale release note or step reference" ""
    OUT=$(grep -A2 -F -e "- uses: familiar-ai-oss/familiar-ci/action@v" "$setup_skill" | sed 's/^ *//')
    exactly "setup skill's pipeline step gives the action its version" "- uses: familiar-ai-oss/familiar-ci/action@v<version>
with:
version: <version>"
    # plan A (lane a-surf): step 8 names the adapter beside the plan and the one-time opt-in
    missing=""
    for want in '.familiar/adapter` is committed with the work' 'familiar-ai share on' 'familiar-ai share off' 'never run `familiar-ai share on` yourself'; do
        if ! tr -s ' \n' '  ' < "$setup_skill" | grep -qF -- "$want"; then
            missing="$missing [$want]"
        fi
    done
    OUT="missing:$missing"
    check "setup skill's step 8 names the adapter and the sharing opt-in" "missing:" [ -z "$missing" ]
}

scenario_removed_state() {
    need_binary || return
    set_up_project removal || return
    hook "edit before the plan" PreToolUse "$(edit_call)"
    need "git add" git_in add -A || return
    need "the commit of the plan" git_in commit -q -m "the plan so far" || return
    plan_before=$(cat "$PROJECT/.familiar/plan")
    rm -rf "$PROJECT/.familiar"
    hook "edit after rm -rf .familiar" PreToolUse "$(edit_call)"
    denied "edit after removal denied"
    field_like "denial says removed, names setup" permissionDecisionReason "*was removed*familiar-ai setup*"
    hook "stop after rm -rf .familiar" Stop "$(payload Stop "")"
    field_is "stop after removal blocked" decision block
    field_like "stop says the folder was removed" reason "*was removed*"
    check "refusals create nothing" "no .familiar" [ ! -e "$PROJECT/.familiar" ]
    hook "the setup call" PreToolUse "$(bash_in_project "familiar-ai setup")"
    exactly "setup command allowed after removal" "{}"
    cli setup
    like "setup restores folder and plan, starts an empty record" "*made .familiar at $PROJECT*restored .familiar/plan from git*.familiar/record starts empty*"
    LAST_INPUT="$PROJECT/.familiar/plan"
    OUT=$(cat "$PROJECT/.familiar/plan")
    exactly "restored plan matches the commit" "$plan_before"
    check "setup starts the record" "a .familiar/record" [ -f "$PROJECT/.familiar/record" ]
    hook "edit after setup" PreToolUse "$(edit_call)"
    denied "edit after the restore denied"
    field_like "restored project asks for the plan" permissionDecisionReason "*familiar-ai: the plan comes first*"
    need "claim intent_submitted" agent_cli claim intent_submitted --for 0 --text again || return
    rm -f "$PROJECT/.familiar/record"
    hook "edit after rm .familiar/record" PreToolUse "$(edit_call)"
    denied "edit after record removal denied"
    field_like "record removal denied as a removal" permissionDecisionReason "*was removed*"
    cli setup
    expect "setup restarts the record" ".familiar/record starts empty"
}

scenario_folder_removal_refused() {
    need_binary || return
    set_up_project courtesy || return
    for command in 'rm -rf .familiar' "rm -rf $PROJECT/.familiar" 'mv .familiar .familiar.off' 'git reset --hard HEAD'; do
        hook "$command" PreToolUse "$(bash_in_project "$command")"
        denied "folder removal denied: $command"
        field_like "denial says the folder is the person's: $command" permissionDecisionReason "*familiar-ai: Familiar's folder stays as it is: it is the person's to remove, not the agent's*"
    done
    for command in 'cat .familiar/plan' 'ls .familiar' 'familiar-ai setup'; do
        hook "$command" PreToolUse "$(bash_in_project "$command")"
        expect_no "folder read not refused as a removal: $command" "not the agent's"
    done
}

scenario_setup_marker() {
    need_binary || return
    set_up_project marker || return
    # The marker outside the repository tells a removal from a project never set up, with no plan in git.
    marker=$(printf '%s\n' "$OUT" | sed -n 's/^familiar-ai: marked this project as set up at //p')
    hook "edit before the plan" PreToolUse "$(edit_call)"
    rm -rf "$PROJECT/.familiar"
    hook "edit after rm -rf .familiar" PreToolUse "$(edit_call)"
    denied "edit after removal denied"
    field_like "denial says removed, names setup" permissionDecisionReason "*was removed*familiar-ai setup*"
    hook "edit on another machine" PreToolUse "$(edit_call)" FAMILIAR_AI_HOME="$TMP/another-machine"
    exactly "project with no marker not set up" "{}"
    cli setup
    like "setup restores the folder, finds the marker" "*made .familiar at $PROJECT*this project is already marked as set up at $marker*"
}

scenario_person_only_holds() {
    need_binary || return
    set_up_project cards || return
    answer="familiar-ai claim hold_answered --criterion_id c1 --hold anchor_moved --choice keep_going --typed ''"
    # CLAUDE_CODE_SESSION_ID is set in the agent's shell, as in Claude Code's Bash tool.
    agent_cli claim hold_answered --criterion_id c1 --hold anchor_moved --choice keep_going --typed ''
    exits "agent answer refused" 1
    like "refusal points to the person" "familiar-ai: not claimed yet · this card is the person's to answer: ask the person with AskUserQuestion; the hook turns that call into the question card and records their pick (in a terminal of their own the person can also run: $answer*"
    cli claim hold_answered --criterion_id c1 --hold not_stamped --choice keep_going --typed ''
    exits "non-card hold refused" 1
    like "refusal lists card holds" "familiar-ai: not claimed yet · hold_answered answers a card, and not_stamped is not a card hold; the card holds are anchor_moved, two_failures, two_tries_not_red, fix_elsewhere, anchor_untouched, two_test_broken, two_crashed, two_timed_out, environment_error, run_unclassified, retry_spent, crashed, command_broke, miscoped, reading_gap, setup_changed, review_disputed, cases_unconfirmed and no_test_stack*"
    cli claim hold_answered --criterion_id c1 --hold anchor_moved --choice keep_going --typed ''
    exits "person answer accepted" 0
    like "person answer recorded" "familiar-ai: claimed {*\"kind\":\"hold_answered\"*"
}

. "$ROOT/plugin/tests/test-run-judge.sh"

run_scenario "missing binary refuses tool calls" scenario_missing_binary_refuses 7
run_scenario "missing binary allows setup commands" scenario_missing_binary_allows_setup 52
run_scenario "missing binary never blocks" scenario_missing_binary_never_blocks 7
run_scenario "long commands refused quickly" scenario_long_commands 4
run_scenario "refusal follows the hook protocol" scenario_refusal_protocol 7
run_scenario "events relayed to the binary" scenario_events_relayed 19
run_scenario "failing binary refuses tool calls" scenario_failing_binary 4
run_scenario "unset project left alone" scenario_unset_project 25
run_scenario "setup through the skill command" scenario_setup 16
run_scenario "plan comes first" scenario_plan_comes_first 8
run_scenario "shell writes checked" scenario_shell_writes 11
run_scenario "session start shows the rulebook" scenario_session_start_rulebook 7
run_scenario "skills stay short" scenario_skills_stay_short 5
run_scenario "removed state restored" scenario_removed_state 14
run_scenario "folder removal refused" scenario_folder_removal_refused 11
run_scenario "setup marker outside the repository" scenario_setup_marker 4
run_scenario "agent cannot answer person-only holds" scenario_person_only_holds 6
run_scenario "judge reads the sealed test" scenario_judge 6

printf '\n'
if [ "$FAILS" = 0 ]; then
    printf 'ALL PASS (%d of %d checks)\n' "$TOTAL_CHECKS" "$EXPECTED_CHECKS"
    exit 0
else
    printf 'checks run: %d of %d\n' "$TOTAL_CHECKS" "$EXPECTED_CHECKS"
    printf '%d case(s) FAILED\n' "$FAILS"
    exit 1
fi
