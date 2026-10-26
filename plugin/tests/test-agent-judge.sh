# Sourced by test-agent.sh, which runs its two scenarios with its helpers.
# A fake claude tests the judge's verdicts, resealing and an invalid response.

# judge_fake: a fake claude first on PATH, the judge on
judge_fake() {
    JUDGE_SCHEMA='{"type":"object","properties":{"answers_intent":{"type":"boolean"},"gap":{"type":"string"},"reason":{"type":"string"},"cases":{"type":"array","items":{"type":"object","properties":{"case_id":{"type":"string"},"test":{"type":"string"},"ok":{"type":"boolean"}},"required":["case_id","test","ok"],"additionalProperties":false}}},"required":["answers_intent","gap","reason","cases"],"additionalProperties":false}'
    mkdir -p "$TMP/judgebin"
    # the fake logs each argument on its own line in <>, and ends each call with a line "."
    cat > "$TMP/judgebin/claude" <<'FAKE'
#!/bin/sh
d=$(dirname "$0")
printf '<%s>\n' "$@" >> "$d/calls"
printf '.\n' >> "$d/calls"
cat > /dev/null
cat "$d/answer"
FAKE
    chmod +x "$TMP/judgebin/claude"
    # answer <text>: the model's reply, escaped into the result's JSON string
    answer() {
        escaped=$(printf '%s' "$1" | sed 's/[\\"]/\\&/g')
        printf '{"type":"result","is_error":false,"result":"%s","modelUsage":{"claude-opus-4-6":{}}}\n' "$escaped" > "$TMP/judgebin/answer"
    }
    # judge_calls: how many times the fake was called, into $OUT
    judge_calls() {
        LAST_INPUT="the calls in $TMP/judgebin/calls"
        LAST_ERR=""
        OUT=$(grep -cx '\.' "$TMP/judgebin/calls" 2>/dev/null)
    }
    JUDGE_OLD_PATH=$PATH
    export PATH="$TMP/judgebin:$PATH"
    unset FAMILIAR_AI_JUDGE
}

scenario_judge_rejected() {
    judge_fake
    fresh_project judge
    plan_the_project
    # the judge is one model above the holder's and never below opus, so a
    # model no stronger than the holder's never passes its own test
    hook "the session's model is switched to haiku" PostModelSwitch post_model_switch "@session_id=$SESSION" "@to_model=claude-haiku-4-5-20251001"
    answer '{"answers_intent": false, "gap": "greet saying hello when it runs", "reason": "it greps the file instead of running greet."}'
    mkdir -p "$PROJECT/tests"
    printf 'grep -q hello src/app.sh\n' > "$PROJECT/tests/test_c1.sh"
    need "git add" git_in add -A
    need "the tests-first commit" git_in commit -q -m "c1: tests first"
    hook "the tests-first commit landed" PostToolUse post_tool_use_bash "command=git commit -m 'c1: tests first'"
    expect "opus judge rejects the test with its reason" "familiar-ai: the judge (opus) read your request against the test: it does not cover greet saying hello when it runs; the test may be changed\\n  the judge's reason: it greps the file instead of running greet; the next commit of tests alone is sealed again"
    expect "person sees the rejection" '"systemMessage":"familiar-ai: the judge (opus) read your request against the test: it does not cover greet saying hello when it runs; the test may be changed"'
    expect "test remains sealed after rejection" "they fail, as they should before the fix · stamped"
    read_file "$TMP/judgebin/calls"
    exactly "one judge call, opus, the empty arguments passed separately" "$(printf '<%s>\n' -p --model opus --output-format json --json-schema "$JUDGE_SCHEMA" --setting-sources '' --strict-mcp-config --tools '' --max-budget-usd 0.50)
."
    read_file "$PROJECT/.familiar/plan"
    expect "judge's reason is recorded as the test change announcement" '"kind":"test_change_announced","criterion_id":"c1","reason":"judge: not exercised: greet saying hello when it runs; it greps the file instead of running greet."'
    hook "edit of the sealed test" PreToolUse pre_tool_use_edit "file_path=$PROJECT/tests/test_c1.sh"
    expect "rejected test may be edited" "{}"

    answer '{"answers_intent": true, "gap": "", "reason": "it runs greet and checks it says hello."}'
    printf 'sh src/app.sh | grep -qx hello\n' > "$PROJECT/tests/test_c1.sh"
    need "git add" git_in add -A
    need "the replaced test's commit" git_in commit -q -m "c1: the test runs greet"
    hook "the replaced test's commit landed" PostToolUse post_tool_use_bash "command=git commit -m 'c1: the test runs greet'"
    expect "replaced test is sealed and accepted" "familiar-ai: the judge (opus) read your request against the test: it covers it"
    judge_calls
    exactly "second seal adds one judge call" 2

    hook "stop before the code" Stop stop
    expect "stop before the code is refused" '"decision":"block"'
    expect "refused stop names the intent" "asked for: greet with hello"
    expect "refused stop names the test file" "tested by: tests/test_c1.sh"
    expect "refused stop names the judge's answer" "judge: it covers the request (opus)"
    cli status
    expect "status shows intent, test file and judge's answer" "familiar-ai: \"greet says hello\"
  asked for: greet with hello
  tested by: tests/test_c1.sh
  judge: it covers the request (opus)"

    printf '#!/bin/sh\ngreet() { echo hello; }\ngreet\n' > "$PROJECT/src/app.sh"
    need "git add" git_in add -A
    need "the code commit" git_in commit -q -m "c1: code"
    hook "the code commit landed" PostToolUse post_tool_use_bash "command=git commit -am 'c1: code'"
    expect "code commit passes the sealed test" "they pass · stamped"
    judge_calls
    exactly "two seals, two judge calls" 2
    hook "stop at green" Stop stop
    case "$OUT" in
        *'"decision"'*)
            fail "stop at green is allowed with completion context" "no decision"
            ;;
        *'"additionalContext":"familiar-ai: every criterion is locally green. CI proves it on the pull request. The branch is not pushed.'*)
            pass "stop at green is allowed with completion context"
            ;;
        *)
            fail "stop at green is allowed with completion context" "to contain: \"additionalContext\":\"familiar-ai: every criterion is locally green. CI proves it on the pull request. The branch is not pushed."
            ;;
    esac
    export PATH="$JUDGE_OLD_PATH"
    export FAMILIAR_AI_JUDGE=off
}

scenario_judge_invalid() {
    judge_fake
    fresh_project judge-garbage
    plan_the_project
    # opus's judge is fable, the model above it
    hook "the session's model is switched to opus" PostModelSwitch post_model_switch "@session_id=$SESSION" "@to_model=claude-opus-4-6"
    answer 'Yes, I think so.'
    mkdir -p "$PROJECT/tests"
    printf 'grep -q hello src/app.sh\n' > "$PROJECT/tests/test_c1.sh"
    need "git add" git_in add -A
    need "the tests-first commit" git_in commit -q -m "c1: tests first"
    hook "the tests-first commit landed" PostToolUse post_tool_use_bash "command=git commit -m 'c1: tests first'"
    expect "person is told the seal stands unjudged" '"systemMessage":"familiar-ai: the judge did not answer (exit 0, but its answer was not the one JSON object); the sealed test stands, not judged; its call is logged in .familiar/judge.log"'
    read_file "$PROJECT/.familiar/judge.log"
    expect "judge log includes the call's arguments and exit" "· criterion c1 · claude -p --model fable --output-format json --json-schema $JUDGE_SCHEMA --setting-sources '' --strict-mcp-config --tools '' --max-budget-usd 0.50 · exit 0 · "
    expect "judge log includes stdout section" 'stdout (first 2000 characters):'
    expect "judge log keeps the bad answer" 'Yes, I think so.'
    cli status
    expect "status shows the test not judged" "judge: not judged yet"

    export PATH="$JUDGE_OLD_PATH"
    export FAMILIAR_AI_JUDGE=off
}
