# Sourced by test-agent.sh, which runs scenario_investigator with its helpers.
# After two failed tries a stronger model takes the criterion over: first as the agent's rewritten spawn, then launched by the binary (a fake claude).

scenario_investigator() {
    fresh_project investigator
    plan_the_project
    INV=a0693f4e
    AS_INV="@agent_id=$INV @agent_type=familiar-ai:investigator"

    hook "the session's model is switched to haiku" PostModelSwitch post_model_switch "@session_id=$SESSION" "@to_model=claude-haiku-4-5-20251001"
    mkdir -p "$PROJECT/tests"
    printf 'grep -q hello src/app.sh\n' > "$PROJECT/tests/test_c1.sh"
    need "git add" git_in add -A
    need "the tests-first commit" git_in commit -q -m "c1: tests first"
    hook "the tests-first commit landed" PostToolUse post_tool_use_bash "command=git commit -m 'c1: tests first'"
    expect "tests-first commit is sealed" "they fail, as they should before the fix · stamped"
    for try in hey howdy; do
        printf '#!/bin/sh\ngreet() { echo %s; }\ngreet\n' "$try" > "$PROJECT/src/app.sh"
        need "the code commit ($try)" git_in commit -q -am "c1: $try"
        hook "a code commit that fails ($try)" PostToolUse post_tool_use_bash "command=git commit -am 'c1: $try'"
    done
    expect "two failed tries hold the criterion" "two tries failed; a stronger model is taking over"
    hook "stop while held on two_failures" Stop stop
    expect "stop names the person's card choices" "the person answers its card: try again, take it over yourself, a dependency comes first or release it"
    expect "stop names sonnet as the investigator for haiku" "; or a stronger model (sonnet) takes it over"

    hook "the holder spawns a subagent (captured asking haiku)" PreToolUse pre_tool_use_agent_spawn_on_haiku "@session_id=$SESSION"
    expect "spawn is allowed as rewritten" '"permissionDecision":"allow"'
    expect "spawn is rewritten to the investigator" '"subagent_type":"familiar-ai:investigator"'
    expect "spawn is rewritten to sonnet" '"model":"sonnet"'
    expect "spawn keeps the agent's other input" '"updatedInput":{"description":"Run echo command","prompt":"Run the shell command: echo hi"'
    expect "spawn reason states the takeover" ': two tries failed; a stronger model is taking over. This spawn runs as familiar-ai:investigator on sonnet'
    expect "person sees the takeover start" '"systemMessage":"familiar-ai: a stronger model (sonnet) is taking \"greet says hello\" over"'

    hook "the investigator starts" SubagentStart subagent_start_investigator "@session_id=$SESSION" "@agent_type=familiar-ai:investigator"
    expect "investigator start names its criterion" "is the stronger model taking \\\"greet says hello\\\" over from $SESSION, where two tries failed."
    expect "investigator start names the test command" "familiar-ai runs sh tests/test_c1.sh after each commit"
    cli status
    expect "status shows the criterion with the subagent" "familiar-ai: \"greet says hello\" · the test fails, as it should before the fix · the code comes next · with a subagent"

    hook "a second spawn" PreToolUse pre_tool_use_agent_spawn_on_haiku "@session_id=$SESSION"
    expect "second spawn is refused while one runs" "one runs at a time in a conversation, so this spawn waits for it"
    hook "the holder's edit" PreToolUse pre_tool_use_edit "file_path=$PROJECT/src/app.sh"
    expect "holder's edit waits for the investigator" "this conversation's edits and commits wait for it to come back"
    # shellcheck disable=SC2086
    hook "the investigator's edit" PreToolUse pre_tool_use_edit "file_path=$PROJECT/src/app.sh" $AS_INV
    expect "investigator's edit is allowed" "{}"
    # shellcheck disable=SC2086
    hook "the investigator stops before green" SubagentStop subagent_stop_investigator "@session_id=$SESSION" $AS_INV
    expect "investigator stop before green is refused" '"decision":"block"'

    printf '#!/bin/sh\ngreet() { echo hello; }\ngreet\n' > "$PROJECT/src/app.sh"
    need "the investigator's fix commit" git_in commit -q -am "c1: the fix"
    # shellcheck disable=SC2086
    hook "the investigator's fix landed" PostToolUse post_tool_use_bash "command=git commit -am 'c1: the fix'" $AS_INV
    expect "investigator's fix passes the sealed test" "they pass · stamped"
    # shellcheck disable=SC2086
    hook "the investigator stops at green" SubagentStop subagent_stop_investigator "@session_id=$SESSION" $AS_INV
    expect "investigator stop at green is allowed" "{}"
    hook "the spawn returns" PostToolUse post_tool_use_agent_investigator "@session_id=$SESSION"
    cli status
    expect "status shows the criterion back with the holder" "familiar-ai: \"greet says hello\" · locally green · unscoped · scope the command to prove in parallel
"
    hook "the holder edits again" PreToolUse pre_tool_use_edit "file_path=$PROJECT/src/app.sh"
    case "$OUT" in
        *"wait for it to come back"*)
            fail "holder's edit no longer waits" "no: wait for it to come back"
            ;;
        *)
            pass "holder's edit no longer waits"
            ;;
    esac
    hook "the holder stops" Stop stop
    case "$OUT" in
        *'"decision"'*)
            fail "holder's stop at green is allowed with completion context" "no decision"
            ;;
        *'"additionalContext":"familiar-ai: every criterion is locally green. CI proves it on the pull request. The branch is not pushed.'*)
            pass "holder's stop at green is allowed with completion context"
            ;;
        *)
            fail "holder's stop at green is allowed with completion context" "to contain: \"additionalContext\":\"familiar-ai: every criterion is locally green. CI proves it on the pull request. The branch is not pushed."
            ;;
    esac

    # The binary launches the investigator itself from the second failed try's
    # PostToolUse, after moving the criterion to it; the fake plays its session
    # through the real hooks.
    if verbose; then
        printf '\n=== the binary launches the investigator ===\n'
    fi
    mkdir -p "$TMP/invbin"
    cat > "$TMP/invbin/claude" <<'FAKE'
#!/bin/sh
d=$(dirname "$0")
printf '%s\n' "$*" >> "$d/calls"
cat > "$d/prompt"
familiar-ai status > "$d/status-at-start" 2>&1
U=""
prev=""
for a in "$@"; do
    if [ "$prev" = "--session-id" ]; then
        U="$a"
    fi
    prev="$a"
done
P=$(pwd)
printf '#!/bin/sh\ngreet() { echo hello; }\ngreet\n' > src/app.sh
git -c user.name=t -c user.email=t@example.com -c commit.gpgsign=false commit -q -am "c1: the investigator's fix" >/dev/null 2>&1
printf '{"hook_event_name":"PostToolUse","session_id":"%s","cwd":"%s","tool_name":"Bash","tool_input":{"command":"git commit -am fix"},"tool_response":{}}' "$U" "$P" | familiar-ai hook PostToolUse > "$d/fix-hook" 2>&1
printf '{"hook_event_name":"Stop","session_id":"%s","cwd":"%s"}' "$U" "$P" | familiar-ai hook Stop > "$d/stop-hook" 2>&1
printf '{"type":"result","subtype":"success","is_error":false,"session_id":"%s","total_cost_usd":0.1,"modelUsage":{"claude-sonnet-4-6":{}},"result":"done"}\n' "$U"
FAKE
    chmod +x "$TMP/invbin/claude"
    INV_OLD_PATH=$PATH
    export PATH="$TMP/invbin:$PATH"
    unset FAMILIAR_AI_INVESTIGATOR

    fresh_project investigator-post
    plan_the_project
    hook "the session's model is switched to haiku" PostModelSwitch post_model_switch "@session_id=$SESSION" "@to_model=claude-haiku-4-5-20251001"
    mkdir -p "$PROJECT/tests"
    printf 'grep -q hello src/app.sh\n' > "$PROJECT/tests/test_c1.sh"
    need "git add" git_in add -A
    need "the tests-first commit" git_in commit -q -m "c1: tests first"
    hook "the tests-first commit landed" PostToolUse post_tool_use_bash "command=git commit -m 'c1: tests first'"
    expect "tests-first commit is sealed" "they fail, as they should before the fix · stamped"
    printf '#!/bin/sh\ngreet() { echo hey; }\ngreet\n' > "$PROJECT/src/app.sh"
    need "the first code commit" git_in commit -q -am "c1: hey"
    hook "a code commit that fails (hey)" PostToolUse post_tool_use_bash "command=git commit -am 'c1: hey'"
    if [ -e "$TMP/invbin/calls" ]; then
        fail "one failed try launches nothing" "no calls to claude: $(cat "$TMP/invbin/calls")"
    else
        pass "one failed try launches nothing"
    fi
    printf '#!/bin/sh\ngreet() { echo howdy; }\ngreet\n' > "$PROJECT/src/app.sh"
    need "the second code commit" git_in commit -q -am "c1: howdy"
    hook "the second failing commit (howdy)" PostToolUse post_tool_use_bash "command=git commit -am 'c1: howdy'"
    expect "holder's turn ends" '"continue":false'
    expect "stop reason says the investigator proved it" '"stopReason":"familiar-ai: escalated to the investigator — a stronger model (sonnet) proved \"greet says hello\""'
    expect "agent context says the investigator proved it" '"additionalContext":"familiar-ai: escalated to the investigator — a stronger model (sonnet) proved \"greet says hello\""'
    read_file "$TMP/invbin/calls"
    expect "investigator launched on sonnet with this plugin" "-p --model sonnet --plugin-dir $ROOT/plugin --output-format json"
    read_file "$TMP/invbin/status-at-start"
    expect "criterion moved to the subagent before the launch" "· with a subagent"
    read_file "$TMP/invbin/fix-hook"
    expect "investigator's fix passes through its own hooks" "they pass · stamped"
    cli status
    expect "status shows the criterion back with the holder" "familiar-ai: \"greet says hello\" · locally green · unscoped · scope the command to prove in parallel
"
    hook "the holder stops" Stop stop
    expect "holder's stop at green has completion context" '"additionalContext":"familiar-ai: every criterion is locally green.'
    read_file "$TMP/invbin/calls"
    if [ "$(wc -l < "$TMP/invbin/calls" | tr -d ' ')" = 1 ]; then
        pass "holder's stop launches no second investigator"
    else
        fail "holder's stop launches no second investigator" "one call to claude"
    fi
    export PATH="$INV_OLD_PATH"
    export FAMILIAR_AI_INVESTIGATOR=off
}
