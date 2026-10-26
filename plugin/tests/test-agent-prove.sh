# Sourced by test-agent.sh, which runs scenario_prove with its helpers.
# Tests the completion messages before the push and after the pull request is open.
# A local git remote and a fake gh (pr view: view.exit, view.json; pr checks: checks.json).

scenario_prove() {
    fresh_project prove
    plan_the_project
    mkdir -p "$PROJECT/tests"
    printf 'sh src/app.sh | grep -qx hello\n' > "$PROJECT/tests/test_c1.sh"
    need "git add" git_in add -A
    need "the tests-first commit" git_in commit -q -m "c1: tests first"
    hook "the tests-first commit landed" PostToolUse post_tool_use_bash "command=git commit -m 'c1: tests first'"
    printf '#!/bin/sh\ngreet() { echo hello; }\ngreet\n' > "$PROJECT/src/app.sh"
    need "git add" git_in add -A
    need "the code commit" git_in commit -q -m "c1: code"
    hook "the code commit landed" PostToolUse post_tool_use_bash "command=git commit -am 'c1: code'"
    expect "code commit passes the sealed test" "they pass · stamped"

    GHBIN="$TMP/ghbin"
    mkdir -p "$GHBIN"
    cat > "$GHBIN/gh" <<'EOF'
#!/bin/sh
here="$(cd "$(dirname "$0")" && pwd)"
echo "$*" >> "$here/calls"
case "$1 $2" in
    "pr view")
        code=$(cat "$here/view.exit" 2>/dev/null || echo 1)
        if [ "$code" = 0 ]; then
            cat "$here/view.json" 2>/dev/null
        fi
        exit "$code"
        ;;
    "pr checks")
        cat "$here/checks.json" 2>/dev/null
        exit 0
        ;;
esac
exit 1
EOF
    chmod +x "$GHBIN/gh"
    PROVE_OLD_PATH="$PATH"
    export PATH="$GHBIN:$PATH"

    PUSH='familiar-ai: every criterion is locally green. CI proves it on the pull request. The branch is not pushed. The person has not been asked whether to push and open the pull request. The push begins with familiar-ai tidy, which keeps one seal commit and one fix commit per criterion and drops the failed attempts.'
    OPEN_PR='familiar-ai: every criterion is locally green. CI proves it on the pull request. The branch is pushed and has no pull request. The person has not been asked whether to open the pull request.'

    statusline() {
        LAST_INPUT=$(printf '{"session_id":"%s","cwd":"%s","workspace":{"current_dir":"%s","project_dir":"%s"}}' "$SESSION" "$PROJECT" "$PROJECT" "$PROJECT")
        OUT=$(printf '%s' "$LAST_INPUT" | familiar-ai statusline 2>"$TMP/err")
        LAST_ERR=$(cat "$TMP/err")
        if verbose; then
            printf '\n$ familiar-ai statusline\n%s\n' "$OUT"
        fi
        case "$OUT" in
            *CI*|*next:*)
                fail "status line has no CI and no next step" "no CI and no next:"
                ;;
            *)
                pass "status line has no CI and no next step"
                ;;
        esac
    }

    # The status line carries the ask and the map, never CI: the Stop and prompt texts below and the band carry CI.
    statusline
    exactly "status line carries the ask and the map" 'prove | Familiar | ON | your ask: greet with hello · 1 green'
    hook "stop, every criterion green" Stop stop
    exactly "stop message tells the agent to ask the person" '{"hookSpecificOutput":{"hookEventName":"Stop","additionalContext":"'"$PUSH"'"}}'
    hook "the stop the hook continued once" Stop stop_after_a_block "@session_id=$SESSION"
    exactly "continued stop shows the person the next step" '{"systemMessage":"familiar-ai: locally green · next: push and open the pull request, and CI proves it"}'
    hook "the person's next prompt" UserPromptSubmit user_prompt_submit "@prompt=anything else?"
    exactly "next prompt repeats completion context" '{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"'"$PUSH"'"}}'
    read_file "$GHBIN/calls"
    case "$OUT" in
        *pr*)
            fail "gh is not called before the push" "no pr call to gh"
            ;;
        *)
            pass "gh is not called before the push"
            ;;
    esac
    hook "stop after that prompt" Stop stop
    case "$OUT" in
        *'"decision"'*)
            fail "stop after the prompt is allowed" "no decision"
            ;;
        *)
            pass "stop after the prompt is allowed"
            ;;
    esac

    need "the local remote" git init -q --bare "$TMP/prove-upstream.git"
    need "git remote add" git_in remote add origin "$TMP/prove-upstream.git"
    need "the push" git_in push -q -u origin main
    statusline
    exactly "a push leaves the status line unchanged" 'prove | Familiar | ON | your ask: greet with hello · 1 green'
    hook "stop, pushed, no pull request" Stop stop
    exactly "stop after the push says there is no pull request" '{"hookSpecificOutput":{"hookEventName":"Stop","additionalContext":"'"$OPEN_PR"'"}}'

    printf '0\n' > "$GHBIN/view.exit"
    printf '{"number":7,"headRefOid":"%s"}' "$(git_in rev-parse HEAD)" > "$GHBIN/view.json"
    statusline
    exactly "a pull request leaves the status line unchanged" 'prove | Familiar | ON | your ask: greet with hello · 1 green'
    hook "stop, the pull request open" Stop stop
    exactly "stop with the pull request open shows CI is proving it" '{"systemMessage":"familiar-ai: CI is proving it"}'
    hook "the person's prompt" UserPromptSubmit user_prompt_submit "@prompt=and now?"
    exactly "prompt with the pull request open repeats CI is proving it" '{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"familiar-ai: CI is proving it"}}'
    printf '[{"name":"build","bucket":"pass"}]' > "$GHBIN/checks.json"
    statusline
    exactly "an unrelated passing check leaves the status line unchanged" 'prove | Familiar | ON | your ask: greet with hello · 1 green'
    printf '[{"name":"build","bucket":"pass"},{"name":"familiar-verify","bucket":"pending","state":"IN_PROGRESS"}]' > "$GHBIN/checks.json"
    statusline
    exactly "pending CI leaves the status line unchanged" 'prove | Familiar | ON | your ask: greet with hello · 1 green'
    printf '[{"name":"build","bucket":"pass"},{"name":"familiar-verify","bucket":"pass","state":"SUCCESS"}]' > "$GHBIN/checks.json"
    statusline
    exactly "passed CI leaves the status line unchanged" 'prove | Familiar | ON | your ask: greet with hello · 1 green'
    hook "stop, CI passed" Stop stop
    exactly "stop after passed CI shows proven" '{"systemMessage":"familiar-ai: proven ✓"}'
    hook "the person's prompt, CI passed" UserPromptSubmit user_prompt_submit "@prompt=and now?"
    exactly "prompt after passed CI repeats proven" '{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"familiar-ai: proven ✓"}}'
    printf '{"number":7,"headRefOid":"0000000000000000000000000000000000000000"}' > "$GHBIN/view.json"
    statusline
    exactly "CI passed on another commit leaves the status line unchanged" 'prove | Familiar | ON | your ask: greet with hello · 1 green'
    printf '{"number":7,"headRefOid":"%s"}' "$(git_in rev-parse HEAD)" > "$GHBIN/view.json"
    printf '[{"name":"familiar-verify","bucket":"fail","state":"FAILURE","description":"familiar-ai: its tests fail at the head"}]' > "$GHBIN/checks.json"
    statusline
    exactly "failed CI leaves the status line unchanged" 'prove | Familiar | ON | your ask: greet with hello · 1 green'
    hook "stop, CI failed" Stop stop
    exactly "stop after failed CI names what CI found" '{"systemMessage":"familiar-ai: CI found: its tests fail at the head (see the PR)"}'
    hook "the person's prompt, CI failed" UserPromptSubmit user_prompt_submit "@prompt=and now?"
    exactly "prompt after failed CI repeats what CI found" '{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"familiar-ai: CI found: its tests fail at the head (see the PR)"}}'
    printf '[{"name":"build","bucket":"pass"},{"name":"lint","bucket":"fail"}]' > "$GHBIN/checks.json"
    statusline
    exactly "failed lint leaves the status line unchanged" 'prove | Familiar | ON | your ask: greet with hello · 1 green'

    export PATH="$PROVE_OLD_PATH"
}
