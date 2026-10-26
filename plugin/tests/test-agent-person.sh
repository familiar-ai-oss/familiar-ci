# Sourced by test-agent.sh, which runs scenario_person with its helpers.
# A prompt is the person's words, not an intent: the agent claims the intent for the prompt it answers.

scenario_person() {
    fresh_project person

    hook "the person asks for a change" UserPromptSubmit user_prompt_submit "@prompt=make greet say hello"
    LAST_INPUT="the kinds in .familiar/record"
    OUT=$(grep -ao '"kind":"[a-z_]*"' "$PROJECT/.familiar/record")
    expect "prompt is recorded as the person's words" '"kind":"person_said"'
    case "$OUT" in
        *intent_submitted*)
            fail "prompt records no intent" "no intent_submitted"
            ;;
        *)
            pass "prompt records no intent"
            ;;
    esac
    hook "a task notification arrives" UserPromptSubmit user_prompt_submit_task_notification
    hook "the agent's first edit" PreToolUse pre_tool_use_edit "file_path=$PROJECT/src/app.sh"
    expect "first code edit asks for the intent" "familiar-ai: the plan comes first · line 1: the intent · src/app.sh is code"
    expect "refusal lists the person's prompt" '\"make greet say hello\"'
    case "$OUT" in
        *task-notification*)
            fail "refusal does not list the task notification" "no task-notification"
            ;;
        *)
            pass "refusal does not list the task notification"
            ;;
    esac
    prompt_id=$(printf '%s' "$OUT" | grep -o 'intent_submitted --for [0-9]*' | head -n 1 | sed 's/.* //')
    if [ -n "$prompt_id" ]; then
        pass "claim command names the prompt"
    else
        fail "claim command names the prompt" "to contain: intent_submitted --for <prompt id>"
    fi
    claim intent_submitted --for "$prompt_id" --text "greet says hello"
    read_file "$PROJECT/.familiar/plan"
    expect "intent records the prompt it answers" "\"for\":$prompt_id"
    cli status
    expect "status names the criterion claim next" "claim it: familiar-ai claim criterion"

    # definition change (the owner, 2026-10-11): light is for documentation only; a code anchor is the full path
    claim behavior --behavior_id b1 --text "the greeting"
    claim promise --promise_id p1 --behavior_id b1 --accepts "a name" --gives_back "a greeting" --refuses "nothing" --cases '[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]'
    claim criterion --criterion_id c1 --text "greet says hello" --path full --behavior_id b1 --promise_id p1
    cases_kept c1  # the cases card passed (plan cases-card, 2026-10-25) [draft names]
    claim place_chosen --criterion_id c1 --places '[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]'
    claim test_planned --criterion_id c1 --file_path tests/test_c1.sh --will create
    claim test_stack_detected --project_root . --test_stack sh --test_command "sh tests/test_c1.sh"
    claim criterion_head --criterion_id c1
    claim criterion_assigned --criterion_id c1
    claim fix_location_category --criterion_id c1 --category this_repository --where ''
    expect "the plan is confirmed" "familiar-ai: the plan is confirmed"
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
    hook "stop at green" Stop stop
    expect "stop at green is allowed with completion context" '{"hookSpecificOutput":{"hookEventName":"Stop","additionalContext":"familiar-ai: every criterion is locally green.'

    hook "the person types again" UserPromptSubmit user_prompt_submit "@prompt=also add a goodbye"
    expect "next prompt repeats completion context" '"additionalContext":"familiar-ai: every criterion is locally green.'
    hook "stop after the new prompt" Stop stop
    case "$OUT" in
        *'"decision"'*)
            fail "stop after the new prompt is allowed" "no decision"
            ;;
        *)
            pass "stop after the new prompt is allowed"
            ;;
    esac
    hook "the agent's next edit" PreToolUse pre_tool_use_edit "file_path=$PROJECT/src/app.sh"
    expect "next code edit asks for a new intent" "familiar-ai: the plan comes first · line 1: the intent · src/app.sh is code"
    expect "refusal lists the new prompt" '\"also add a goodbye\"'
}
