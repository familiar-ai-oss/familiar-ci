//! The discipline shown at the start of every session in a set-up project. The plugin's `plan` skill carries no copy of it; `plugin/tests/test-run.sh` and `tests/card_picker_required.rs` keep it that way.

/// The discipline with the example command's Python as found on PATH (`example::python`).
pub fn discipline() -> &'static str {
    static TEXT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    TEXT.get_or_init(|| DISCIPLINE.replace("'python -m pytest", &format!("'{} -m pytest", crate::teach::example::python())))
}

const DISCIPLINE: &str = "\
familiar-ai: proof that the work is done. The record (.familiar/record) holds the facts;
.familiar/plan mirrors them and .familiar/adapter says how each test runner is read; both are committed with the work. The binary writes all three; never edit them.
The record is a database file: read it with familiar-ai status (or familiar-ai status --json), never open it directly. The plan file is its readable mirror.
Nothing is edited or committed until planning_confirm_complete: the plan's eleven lines hold. Under pytest the test comes before the place: once lines 1-3 and 6-10 hold, write the planned test (only it), run familiar-ai try, and take line 4's place from the list the failing test ran, best match first. t, at and tenant are stamped by the binary; never put them in a claim. Claim them in this order, each
shaped exactly like its example, with your own values (familiar-ai claim lists the kinds; familiar-ai claim criterion, no fields, prints that kind's form); all at once: familiar-ai plan takes the lines as a JSON array on stdin (familiar-ai plan with nothing on stdin prints the array's form); one call, the status comes back with it:
  1 the intent      familiar-ai claim intent_submitted --text 'greet with hello'   (the sha is the head, read by the binary)
  2 a criterion     familiar-ai claim criterion --criterion_id c1 --text 'greet says hello' --path full --behavior_id b1 --promise_id p1   (--path light only when the anchor is documentation: a markdown file or a docstring; the binary checks it; everything else is --path full and also needs line 3)  (claim line 3 first: the full criterion names its behavior and promise)
  3 full path only  familiar-ai claim behavior --behavior_id b1 --text 'greeting by name'
                    familiar-ai claim promise --promise_id p1 --behavior_id b1 --accepts 'a name' --gives_back 'a greeting' --refuses 'an empty name' --cases '[{\"case_id\":\"n1\",\"kind\":\"normal\",\"short\":\"a plain name is greeted\",\"given\":\"the name Ada\",\"expect\":\"hello Ada\"},{\"case_id\":\"n2\",\"kind\":\"normal\",\"short\":\"a name with a space is greeted in full\",\"given\":\"a name with a space, Ada Lovelace\",\"expect\":\"hello Ada Lovelace\"},{\"case_id\":\"e1\",\"kind\":\"edge\",\"short\":\"a very long name is greeted in full\",\"given\":\"a name of 200 characters\",\"expect\":\"greeted in full\"},{\"case_id\":\"o1\",\"kind\":\"opposite\",\"short\":\"an empty name is refused\",\"given\":\"an empty name\",\"expect\":\"refused, no greeting\"}]'   (the cases the tests make: normal, the behavior asked for; opposite, the same call with the trigger absent, or --opposite_none with the reason there is none; edge, a boundary the person's words name; short, the line the person reads)
  4 its anchor      familiar-ai claim place_chosen --criterion_id c1 --places '[{\"file_path\":\"src/app.py\",\"symbol\":\"greet\",\"line_start\":10,\"line_end\":20}]'
                    (anchor the whole function or class the fix will touch (its full line range), not one line; the fix must land inside it; under pytest a function from the try's list, or one off it with familiar-ai claim off_list_reason --criterion_id c1 and --reason, one line of your own on why the fix lies where the failing test never ran;
                    new work beside a symbol: new_work_spot_chosen; new work with no spot: new_work_no_spot --integration_point src/app.py:main, where it is reached from or stored)
  5 the check       the binary checks the lines at the sha and writes anchor_sha_matched; a false match is the hold anchor_not_found
  6 its tests       familiar-ai claim test_planned --criterion_id c1 --file_path tests/test_greet.py --will create   (named after what it tests, never c1; or modify; one claim per test file)
  7 the command     familiar-ai claim test_stack_detected --project_root . --test_stack pytest --test_command 'python -m pytest -q tests/test_greet.py'
                    (--project_root . is the whole project; src/ covers only the anchors under src/; --codebase_id left out is the repository folder's name;
                    the binary runs the command from the project's root after each commit: not passed = failed)
  8 touching tests  familiar-ai claim touching_test_declared --criterion_id c1 --file_path tests/test_old.py --test_name test_greet   (existing tests the change touches; none is allowed)
  9 the string      familiar-ai claim criterion_head --criterion_id c1   (each further one names the head: familiar-ai claim criterion_strung_on_head --criterion_id c2 --head c1, a third --criterion_id c3 --head c1; turns go in the order claimed)
 10 who holds it    familiar-ai claim criterion_assigned --criterion_id c1   (this conversation, from CLAUDE_CODE_SESSION_ID)
 11 where it lives  familiar-ai claim fix_location_category --criterion_id c1 --category '' --where ''   (choose the category, never copied: this_repository when the fix is made in this checkout, else dependency, environment or request, and --where names where it lives; the person is asked, no test and no code)
                    (the category names where the failing code lives: when the behavior to change is implemented in a dependency (an installed package, a compiled extension, a vendored library, e.g. pydantic-core's Rust for a pydantic issue) it is dependency, --where names the package and the symbol; this_repository only when the lines to change are in this checkout)
Then, for the criterion whose turn it is (testing):
  - commit its planned tests ALONE, no code (the plan and adapter files may ride along); the binary runs the test command: they must FAIL; it stamps them: sealed
  - a judge (one rung up, at least opus) reads them against the person's words: yes = sealed (red, covered), then the code; no = change the test, commit it alone
  - commit the code on its anchor; the binary runs the tests again: they must pass under the same stamp: green
  - never edit a sealed test without announcing it first: familiar-ai claim test_change_announced --criterion_id c1 --reason 'the assertion named the wrong file'
  - a card (anchor_moved, two_failures, two_tries_not_red, fix_elsewhere, anchor_untouched, no_test_stack, repro_unknown: how to make it happen) is the person's to answer. When a card is open, ask the person with AskUserQuestion; the hook turns that call into the question card and records their pick; never claim hold_answered yourself   (no_test_stack: or you detect the stack yourself, line 7; never wait for the person on it)
  - two_failures has a second way out besides the card: a spawn of the Agent tool by its holder runs as familiar-ai:investigator, one rung up the ladder haiku, sonnet, opus, fable, a second fable; the criterion moves to it, its stop waits for green, and green sends it back
  - once a full-path promise is whole, before its test: the lens panel, five Agent calls in parallel then one for the synthesis, each runs as familiar-ai:lens with the binary's brief; then the person's cases card (cases_unconfirmed) and the panel's questions are asked with AskUserQuestion, and the cases they pick are added to the promise
familiar-ai status shows where every criterion stands; every refusal names the line that is missing, the command that claims it, and the kind's form.
An edit outside the anchor is told, not refused. Stopping is refused until every criterion is green, proven or released.";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::teach::form::example_command;

    #[test]
    fn the_discipline_is_at_most_thirty_two_lines_and_names_the_doors_kinds() {
        assert!(discipline().lines().count() <= 32, "{}", discipline().lines().count());
        for k in [
            "intent_submitted",
            "criterion",
            "behavior",
            "promise",
            "place_chosen",
            "new_work_spot_chosen",
            "new_work_no_spot",
            "test_planned",
            "test_stack_detected",
            "touching_test_declared",
            "criterion_head",
            "criterion_strung_on_head",
            "criterion_assigned",
            "fix_location_category",
            "test_change_announced",
            "hold_answered",
            "familiar-ai:investigator",
            // the cases card (the owner's plan of 2026-10-25) [draft name]; the lens panel (the
            // owner's plan of 2026-10-26, approved; updated with the definition, it was the case-finder)
            "familiar-ai:lens",
            "cases_unconfirmed",
            "planning_confirm_complete",
            "familiar-ai status",
        ] {
            assert!(discipline().contains(k), "{k}");
        }
        assert!(!discipline().contains("state:"), "no field named state");
        // plan A-1 (lane a-surf): the adapter is the binary's and is committed with the work, as the plan is
        assert!(discipline().contains(".familiar/adapter says how each test runner is read; both are committed with the work"), "{}", discipline());
        assert!(discipline().contains("(the plan and adapter files may ride along)"), "{}", discipline());
    }

    #[test]
    fn the_ten_lines_are_the_forms_own_examples_and_carry_no_placeholder() {
        assert!(!discipline().contains('<') && !discipline().contains('>'), "a placeholder in angle brackets gets pasted literally");
        for kind in [
            "intent_submitted",
            "criterion",
            "behavior",
            "promise",
            "place_chosen",
            "test_planned",
            "test_stack_detected",
            "touching_test_declared",
            "criterion_head",
            "criterion_strung_on_head",
            "criterion_assigned",
            "fix_location_category",
            "test_change_announced",
        ] {
            let example = example_command(kind).expect(kind);
            assert!(discipline().contains(&example), "{kind}: the discipline's line is not the form's example:\n  {example}");
        }
        assert!(discipline().contains("--project_root . is the whole project"));
        // definition change (the owner, 2026-10-15): every link names the string's head, a third part too
        assert!(discipline().contains("a third --criterion_id c3 --head c1; turns go in the order claimed"), "line 9 shows a third part on the head");
    }
}
