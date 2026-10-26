#!/bin/sh
# What a tests-first commit's run is read as (plan §6.1, S10), through the real hooks: the family
# decides seal, refusal or card. It takes test-agent.sh's helpers (everything above its first
# scenario) so that script stays as it is. From the worktree root: sh plugin/tests/test-outcomes.sh
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
H=$(mktemp)
sed '/^# -\{20,\}$/,$d' "$ROOT/plugin/tests/test-agent.sh" > "$H"
. "$H"
rm -f "$H"

# py_project <name> [app.py text]: a Python project, planned: src/app.py's greet is the place, unittest the stack
py_project() {
    fresh_project "$1"
    printf '%b' "${2:-def greet():\n    return \"hi\"\n}" > "$PROJECT/src/app.py"
    printf 'def helper():\n    return "x"\n' > "$PROJECT/src/other.py"
    need "git add" git_in add -A
    need "the code commit" git_in commit -q -m "the code"
    claim intent_submitted --for 0 --text "greet with hello"
    claim behavior --behavior_id b1 --text "the greeting"
    claim promise --promise_id p1 --behavior_id b1 --accepts "a name" --gives_back "a greeting" --refuses "nothing" --cases '[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]'
    claim criterion --criterion_id c1 --text "greet says hello" --path full --behavior_id b1 --promise_id p1
    cases_kept c1  # the cases card passed (plan cases-card, 2026-10-25) [draft names]
    claim place_chosen --criterion_id c1 --places '[{"file_path":"src/app.py","symbol":"greet","line_start":1,"line_end":2}]'
    claim test_planned --criterion_id c1 --file_path tests/test_greeting.py --will create
    claim test_stack_detected --codebase_id agent-walk --project_root . --test_stack unittest --test_command "python3 -m unittest discover -s tests"
    claim criterion_head --criterion_id c1
    claim criterion_assigned --criterion_id c1
    claim fix_location_category --criterion_id c1 --category this_repository --where ''
    classify_places
}

# classify_places: definition change (the owner's plan of 2026-10-21, step 3) [draft]: each site the
# binary listed (src/app.sh's greet beside src/app.py's), classified before the tests-first commit
classify_places() {
    for at in $(cd "$PROJECT" && familiar-ai status 2>/dev/null | sed -n 's/^  unclassified: \([^ ]*\).*/\1/p'); do
        claim place_classified --criterion_id c1 --file "${at%:*}" --line "${at##*:}" --verdict not_affected --reason "another greet, in a file this part does not change"
    done
}

# tests_first <test body>: the planned test, committed alone; the hook's words in $OUT
tests_first() {
    mkdir -p "$PROJECT/tests"
    printf 'import sys, unittest\nsys.path.insert(0, "src")\n%s\n' "$1" > "$PROJECT/tests/test_greeting.py"
    need "git add" git_in add -A
    need "the tests-first commit" git_in commit -q -m "c1: tests first"
    hook "the tests-first commit landed" PostToolUse post_tool_use_bash "command=git commit -m 'c1: tests first'"
}

# sh_first <name> <test text>: the sh walk's project, its test committed alone and run
sh_first() {
    fresh_project "$1"
    plan_the_project
    mkdir -p "$PROJECT/tests"
    printf '%s\n' "$2" > "$PROJECT/tests/test_c1.sh"
    need "git add" git_in add -A
    need "the tests-first commit" git_in commit -q -m "c1: tests first"
    hook "the tests-first commit landed" PostToolUse post_tool_use_bash "command=git commit -m 'c1: tests first'"
}

# code <text> [file]: a code commit, and its run
code() {
    printf '%b' "$1" > "$PROJECT/${2:-src/app.py}"
    need "the code commit" git_in commit -q -am "c1: code"
    hook "the code commit landed" PostToolUse post_tool_use_bash "command=git commit -am 'c1: code'"
}

ASSERT='import app
class T(unittest.TestCase):
    def test(self): self.assertEqual(app.greet(), "hello")'
CRASH='import os, signal
class T(unittest.TestCase):
    def test(self): os.kill(os.getpid(), signal.SIGSEGV)'
SEALED="they fail, as they should before the fix · stamped"
BROKEN="did not fail for the reason it tests"

# the families of the plan's §6.1 table: which run is a seal, which is refused, which is a card
scenario_outcomes() {
    py_project collect
    tests_first 'import nosuchmodule_zz'
    expect "a collection error at the tests-first commit is refused" "the tests could not be collected"
    expect "it is not sealed" "$BROKEN"
    case "$OUT" in *"$SEALED"*) fail "a collection error is not sealed" "no: $SEALED" ;; *) pass "a collection error is not sealed" ;; esac

    py_project assertion
    tests_first "$ASSERT"
    expect "an assertion seals" "$SEALED"
    code 'def greet(:\n'
    code 'def greet(::\n'
    expect "two fixes that break the test hold two_collection_error" "two tries ended with the test broken; this needs you"
    hook "stop while held on two_collection_error" Stop stop
    expect "the stop is refused, the card is the person's" "the person answers its card: try again, plan it again, a dependency comes first or release it"

    py_project inplace 'def greet():\n    raise NotImplementedError("later")\n'
    tests_first 'import app
class T(unittest.TestCase):
    def test(self): app.greet()'
    expect "an exception inside the place's function seals" "$SEALED"

    py_project elsewhere
    tests_first 'import other
class T(unittest.TestCase):
    def test(self): other.nope()'
    expect "an exception outside the place is refused" "$BROKEN"

    # an error raised in the project's code outside the planned function: can't tell, the person's
    # card; it stays the person's when the code lands first and the run passes (the bench's 13363)
    py_project beside 'def greet():\n    return make()\n\ndef make():\n    raise ValueError("not yet")\n'
    tests_first "$ASSERT"
    expect "an error outside the planned function is the person's card" "the run ended in a way no rule reads; this needs you"
    code 'def greet():\n    return "hello"\n'
    cli status
    expect "the card stays the person's after the code lands" "the run ended in a way no rule reads; this needs you"

    py_project missing
    tests_first 'from app import farewell
class T(unittest.TestCase):
    def test(self): farewell()'
    expect "a missing name that is not the place's is refused" "$BROKEN"

    py_project crash
    tests_first "$CRASH"
    expect "a crash is the person's card" "the test crashed before any fix; this needs you"
    cli claim hold_answered --criterion_id c1 --hold crashed --choice its_the_bug --typed ''
    exits "the agent's answer to the crashed card is refused" 1
    person_cli claim hold_answered --criterion_id c1 --hold crashed --choice its_the_bug --typed ''
    expect "its_the_bug seals on the crash" "$SEALED"
    expect "an answer with no signed approval is not verified" "crashed its_the_bug: answered, not verified"

    py_project dependency
    tests_first "$CRASH"
    person_cli claim hold_answered --criterion_id c1 --hold crashed --choice a_dependency --typed ''
    exits "a_dependency is the person's answer to a crash" 0
    expect "it reads answered, not verified" "crashed a_dependency: answered, not verified"

    py_project environment 'import nosuchdep_zz\ndef greet():\n    return "hi"\n'
    tests_first "$ASSERT"
    expect "a missing dependency is the environment's card" "the tests could not run here; this needs you"

    py_project killed
    tests_first 'import os, signal
class T(unittest.TestCase):
    def test(self): os.kill(os.getpid(), signal.SIGKILL)'
    expect "a run killed twice is the retry-spent card" "the run was killed or its runner crashed, twice; this needs you"
}

# the cards of a shell test's run, and the person's side of the record
scenario_cards() {
    sh_first unclassified 'exit 3'
    expect "a run no rule reads is a card" "the run ended in a way no rule reads; this needs you"
    hook "stop while held" Stop stop
    expect "the stop is refused" '"decision":"block"'

    sh_first broke '. src/app.sh >/dev/null || exit $?
[ "$(greet)" = hello ]'
    expect "a failing assertion seals" "$SEALED"
    code 'greet() { echo hello;\n' src/app.sh
    expect "a command that worked and breaks is a card" "the test command worked before and fails now; this needs you"
    # the shell's own line names it: bash's `src/app.sh: line 2: syntax error: ...`, dash's (Ubuntu's sh)
    # `tests/test_c1.sh: 2: src/app.sh: Syntax error: ...`
    if [ -n "$(sh -c 'echo "${BASH_VERSION-}"')" ]; then
        expect "naming the syntax error" "syntax error"
    else
        expect "naming the syntax error" "Syntax error"
    fi
    hook "stop while held on command_broke" Stop stop
    expect "the stop is refused" '"decision":"block"'

    cli approve --criterion_id c1 --hold command_broke --choice release
    exits "approve is refused from the agent's shell" 1
    expect "it is the person's" "familiar-ai approve is the person's"
    hook "approve through the hook" PreToolUse pre_tool_use_bash "command=familiar-ai approve --criterion_id c1 --hold command_broke --choice release"
    expect "the hook denies it" '"permissionDecision":"deny"'

    hook "await-run after a call that reserved no run" await-run post_tool_use_bash "command=ls"
    exactly "await-run exits 0 at once, saying nothing" ""
}

# review_fake: a fake claude first on PATH, the judge on. The judge always covers; a call with
# read-only tools is the reviewer's, which answers what review_says last wrote.
review_fake() {
    mkdir -p "$TMP/reviewbin"
    cat > "$TMP/reviewbin/claude" <<'FAKE'
#!/bin/sh
d=$(dirname "$0")
cat > /dev/null
case "$*" in
*'--tools Read,Grep'*) printf '.\n' >> "$d/reviews"; cat "$d/review" ;;
*) printf '%s\n' '{"type":"result","is_error":false,"result":"{\"answers_intent\": true, \"gap\": \"\", \"reason\": \"it runs greet.\"}","modelUsage":{"claude-opus-4-6":{}}}' ;;
esac
FAKE
    chmod +x "$TMP/reviewbin/claude"
    export PATH="$TMP/reviewbin:$PATH"
    unset FAMILIAR_AI_JUDGE
}

# review_says <file> <line> <text>: the reviewer's no, quoting <text> at <file>:<line>
review_says() {
    python3 -c '
import json, sys
a = {"covers": False, "gap": "greet saying hello to the name it is given", "quotes": [{"file": sys.argv[1], "line": int(sys.argv[2]), "text": sys.argv[3]}], "reason": "greet never reads its argument."}
print(json.dumps({"type": "result", "is_error": False, "result": json.dumps(a), "modelUsage": {"claude-opus-4-6": {}}}))
' "$1" "$2" "$3" > "$TMP/reviewbin/review"
}

# green_project <name>: the sh walk's project, sealed red and covered, then green
green_project() {
    sh_first "$1" 'sh src/app.sh | grep -qx hello'
    expect "$1: the seal is covered" "sealed (red, covered)"
    printf '#!/bin/sh\ngreet() { echo hello; }\ngreet\n' > "$PROJECT/src/app.sh"
    need "the code commit" git_in commit -q -am "c1: code"
    hook "the code commit landed" PostToolUse post_tool_use_bash "command=git commit -am 'c1: code'"
    expect "$1: green" "they pass · stamped"
}

# the reviewer at green (the owner's plan of 2026-10-20, step 3): a no blocks only with quotes the
# binary finds in the repo at the green commit
scenario_review() {
    review_fake
    green_project fabricated
    review_says src/app.sh 2 'greet() { echo "hello $1"; }'
    hook "stop at green, a no with a quote not in the repo" Stop stop
    case "$OUT" in *'"decision"'*) fail "a fabricated quote never blocks" "no decision" ;; *) pass "a fabricated quote never blocks" ;; esac
    expect "the person is told it is not judged" "the reviewer's no is not judged: quote 1: src/app.sh line 2 does not read as quoted"

    green_project real
    review_says src/app.sh 2 'greet() { echo hello; }'
    hook "stop at green, a no with a real quote" Stop stop
    expect "a real quote blocks the stop" '"decision":"block"'
    expect "the refusal names the gap" "the reviewer (opus) found the fix does not yet do greet saying hello to the name it is given"
    expect "and its proof" 'its proof: src/app.sh:2 \"greet() { echo hello; }\"'
    read_file "$TMP/reviewbin/reviews"
    exactly "one review per green commit" "$(printf '.\n.')"
    hook "stop again, no new commit" Stop stop
    expect "the no still stands" '"decision":"block"'
    read_file "$TMP/reviewbin/reviews"
    exactly "not asked again for the same commit" "$(printf '.\n.')"
}

run_scenario "outcome families and routes" scenario_outcomes
run_scenario "run cards, approve, await-run" scenario_cards
run_scenario "the reviewer at green blocks only with quotes in the repo" scenario_review

printf '\n'
if [ "$FAILS" = 0 ]; then printf 'ALL PASS\n'; exit 0; fi
printf '%d expectation(s) FAILED\n' "$FAILS"
exit 1
