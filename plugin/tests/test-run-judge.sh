# Sourced by test-run.sh: a judge reads each sealed test against the request.
# A fake claude on PATH answers as the judge; the real model is never called.

scenario_judge() {
    # PostToolUse and Stop wait for the test run, the judge and the investigator; any other hook that opens the record may ask the judge.
    LAST_INPUT="$ROOT/plugin/hooks/hooks.json"
    OUT=$(python3 - "$ROOT/plugin/hooks/hooks.json" <<'EOF'
import json, sys
h = json.load(open(sys.argv[1]))["hooks"]
t = {ev: [x["timeout"] for e in h[ev] for x in e["hooks"]] for ev in h}
print("ok" if t.pop("PostToolUse") == [1800, 1920] and t.pop("Stop") == [1800] and t.pop("PreModelSwitch") == [30] and all(v == [200] for v in t.values()) else t)
EOF
)
    exactly "hook timeouts leave room for the judge" ok

    need_binary || return

    # the answer shape the judge is asked for: the plan's promise gives cases, so the contract's
    # (definition change, the contract required of every new promise, 2026-10-21 [draft names])
    judge_schema='{"type":"object","properties":{"answers_intent":{"type":"boolean"},"gap":{"type":"string"},"reason":{"type":"string"},"cases":{"type":"array","items":{"type":"object","properties":{"case_id":{"type":"string"},"test":{"type":"string"},"ok":{"type":"boolean"}},"required":["case_id","test","ok"],"additionalProperties":false}}},"required":["answers_intent","gap","reason","cases"],"additionalProperties":false}'
    for walk in on off; do
        mkdir -p "$TMP/judge-$walk/bin"
        cat > "$TMP/judge-$walk/bin/claude" <<'FAKE'
#!/bin/sh
printf '%s\n' "$*" >> "$(dirname "$0")/../calls"
cat > /dev/null
printf '%s\n' '{"type":"result","is_error":false,"result":"{\"answers_intent\": true, \"gap\": \"\", \"reason\": \"it checks greet.\"}","modelUsage":{"claude-fable-5":{}}}'
FAKE
        chmod +x "$TMP/judge-$walk/bin/claude"
    done

    set_up_project judge-on/p || return
    HOOK_PATH="$TMP/judge-on/bin:$REAL_PATH"
    plan_the_project || return
    need "commit the tests" commit_the_tests || return
    hook "tests-first commit landed" PostToolUse "$(payload PostToolUse ',"tool_name":"Bash","tool_input":{"command":"git commit"},"tool_response":{}')" FAMILIAR_AI_JUDGE=on
    field_is "agent told the verdict before the seal" additionalContext 'familiar-ai: the judge (fable) read your request against the test: it covers it\nfamiliar-ai: ran the tests of \"greet says hello\": they fail, as they should before the fix · sealed (red, covered)\n  the code goes on src/app.sh greet lines 1–3; the test stays as it is'
    field_is "person sees the verdict" systemMessage "familiar-ai: the judge (fable) read your request against the test: it covers it"
    LAST_INPUT="$TMP/judge-on/calls"
    OUT=$(cat "$TMP/judge-on/calls" 2>&1)
    exactly "judge asked once, on fable for an unknown model" "-p --model fable --output-format json --json-schema $judge_schema --setting-sources  --strict-mcp-config --tools  --max-budget-usd 0.50"

    set_up_project judge-off/p || return
    HOOK_PATH="$TMP/judge-off/bin:$REAL_PATH"
    plan_the_project || return
    need "commit the tests" commit_the_tests || return
    hook "tests-first commit landed" PostToolUse "$(payload PostToolUse ',"tool_name":"Bash","tool_input":{"command":"git commit"},"tool_response":{}')" FAMILIAR_AI_JUDGE=off
    exactly "judge off: the seal is stamped unjudged" '{"hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":"familiar-ai: ran the tests of \"greet says hello\": they fail, as they should before the fix · stamped\n  the code goes on src/app.sh greet lines 1–3; the test stays as it is"}}'
    check "judge off: claude never called" "no calls" [ ! -e "$TMP/judge-off/calls" ]
}
