//! `claim` lists the fact kinds, `claim <kind>` prints a kind's form without
//! a project, and a refused claim is followed by the kind's form.

mod common;

use common::{planned_to_line_7, run_binary};

#[test]
// definition change (the owner, 2026-10-20): existing_test_announced [draft] is claimable
// after test_change_announced (18 became 19)
// definition change (the owner's plan of 2026-10-21, step 3, places) [draft]: place_classified is
// claimable after new_work_no_spot (19 became 20)
// definition change (the owner's plan of 2026-10-21, step 2, a clarifying question on a signal)
// [draft]: one_reading and reading_alternatives are claimable after intent_submitted (20 became 22)
// definition change (the owner's plan of 2026-10-23; owner approved 2026-10-23) [draft]:
// off_list_reason is claimable after new_work_no_spot (22 became 23)
fn claim_alone_lists_the_twenty_two_kinds_and_claim_kind_prints_its_form_without_a_project() {
    // runs in this crate's directory, where no `.familiar` exists up to the root
    let output = run_binary(&["claim"], "");
    assert_eq!(common::exit_code(&output), 0, "{}", common::stderr(&output));
    let out = common::stdout(&output);
    assert_eq!(common::stderr(&output), "");
    let kinds: Vec<&str> = out.lines().skip(1).take(23).map(|l| l.split_whitespace().next().unwrap_or("")).collect();
    assert_eq!(
        kinds,
        [
            "intent_submitted",
            "one_reading",
            "reading_alternatives",
            "criterion",
            "behavior",
            "promise",
            "place_chosen",
            "new_work_spot_chosen",
            "new_work_no_spot",
            "off_list_reason",
            "place_classified",
            "test_planned",
            "test_stack_detected",
            "touching_test_declared",
            "criterion_head",
            "criterion_strung_on_head",
            "criterion_assigned",
            "fix_location_category",
            "test_change_announced",
            "existing_test_announced",
            "hold_answered",
            "criterion_transfer",
            "person_said_answered",
        ],
        "{out}"
    );
    for kind in kinds {
        let output = run_binary(&["claim", kind], "");
        assert_eq!(common::exit_code(&output), 0, "{kind}: {}", common::stderr(&output));
        let out = common::stdout(&output);
        assert!(out.starts_with(&format!("familiar-ai claim {kind}: ")), "{out}");
        assert!(out.contains("\nexample:\n  familiar-ai claim "), "{out}");
        assert!(!out.contains('<'), "no placeholder: {out}");
    }
    let output = run_binary(&["claim", "test_stack_detected"], "");
    let out = common::stdout(&output);
    assert!(out.contains("--project_root  folder  required         . is the whole project"), "{out}");
    assert!(out.contains("--codebase_id   text    may be left out  left out: the repository folder's name"), "{out}");
    // definition change (lane l1, 2026-10-15): a test file is named after what it tests, never a plan id
    // definition change (the owner, 2026-10-15, §9 Q3): the example runs python -m pytest, python3 when no python is on PATH
    assert!(out.contains(&format!("familiar-ai claim test_stack_detected --project_root . --test_stack pytest --test_command '{py} -m pytest -q tests/test_greet.py'", py = familiar_ai::teach::example::python())), "{out}");

    // a kind the door does not know: refused, then the listing
    let output = run_binary(&["claim", "criterion_behavior_chosen"], "");
    assert_eq!(common::exit_code(&output), 1);
    assert_eq!(common::stdout(&output), "");
    let err = common::stderr(&output);
    assert!(err.starts_with("familiar-ai: not claimed yet · append: unknown kind 'criterion_behavior_chosen'\n"), "{err}");
    assert!(err.contains("\n  intent_submitted ") && err.contains("\n  criterion_transfer "), "{err}");
}

#[test]
fn a_stack_claimed_with_a_dot_root_covers_an_anchor_in_a_subfolder_and_the_id_is_the_folders_name() {
    let p = planned_to_line_7("stack-dot");
    // definition change (the owner's plan of 2026-10-23; owner approved 2026-10-23) [draft]: a pytest
    // part's place holds line 4 after a red try that lists it, the binary's listing
    let listed = serde_json::json!({ "criterion_id": "c1", "run_id": "try-1", "sha": "base", "red": true, "path_size": 1, "ranker": "words", "query": "the schema", "top": [{"rank": 1, "file_path": "pydantic/json_schema.py", "symbol": "schema", "line_start": 1, "line_end": 2}] });
    familiar_ai_facts::Record::open(&p.dir.join(".familiar/record")).expect("the record").append_raw("trace_listed", listed.as_object().cloned().unwrap_or_default(), 0.0, familiar_ai::TENANT).expect("the try's listing");
    let folder = p.dir.file_name().expect("name").to_string_lossy().into_owned();
    // definition change (lane l1, 2026-10-15): a test file is named after what it tests, never a plan id
    let (code, out, err) = p.run(&["claim", "test_stack_detected", "--project_root", ".", "--test_stack", "pytest", "--test_command", "pytest -q tests/test_schema.py"]);
    assert_eq!(code, 0, "{err}");
    let claimed = out.lines().next().expect("the claimed line");
    assert!(claimed.contains("\"project_root\":\"\""), "the dot is the empty prefix: {claimed}");
    assert!(claimed.contains(&format!("\"codebase_id\":\"{folder}\"")), "{claimed}");
    assert!(out.contains("claim it: familiar-ai claim criterion_head"), "line 7 holds, line 9 is next: {out}");
    assert!(!out.contains("claim it: familiar-ai claim test_stack_detected"), "the stack applied: {out}");
    let (code, json, err) = p.run(&["status", "--json"]);
    assert_eq!(code, 0, "{err}");
    let status: serde_json::Value = serde_json::from_str(&json).expect("status --json");
    assert_eq!(status["criteria"][0]["holds"], serde_json::json!(["planning_incomplete"]), "the stack applied, the plan still open: {status}");
}

#[test]
fn a_stacks_root_is_straightened_on_claim_and_one_outside_the_project_is_refused_with_the_form() {
    let p = planned_to_line_7("stack-roots");
    let abs = p.dir.join("pydantic").to_string_lossy().into_owned();
    for (given, stored) in [("./", ""), ("pydantic", "pydantic/"), (abs.as_str(), "pydantic/"), (p.dir.to_str().expect("utf-8"), "")] {
        let (code, out, err) = p.run(&["claim", "test_stack_detected", "--project_root", given, "--test_stack", "pytest", "--test_command", "pytest -q"]);
        assert_eq!(code, 0, "{given}: {err}");
        let claimed = out.lines().next().expect("the claimed line");
        assert!(claimed.contains(&format!("\"project_root\":\"{stored}\"")), "{given}: {claimed}");
    }
    // src/ does not cover pydantic/json_schema.py: stored as given, line 7 still missing
    let (code, out, _) = p.run(&["claim", "test_stack_detected", "--project_root", "src", "--test_stack", "pytest", "--test_command", "pytest -q"]);
    assert_eq!(code, 0);
    assert!(out.lines().next().expect("claimed").contains("\"project_root\":\"src/\""), "{out}");
    for outside in ["/work/repo", "../elsewhere"] {
        let (code, out, err) = p.run(&["claim", "test_stack_detected", "--project_root", outside, "--test_stack", "pytest", "--test_command", "pytest -q"]);
        assert_eq!(code, 1, "{outside}");
        assert_eq!(out, "");
        let first = err.lines().next().expect("one plain line");
        assert!(first.starts_with("familiar-ai: not claimed yet · project_root ") && first.contains("the project"), "{first}");
        assert!(err.contains("familiar-ai claim test_stack_detected: line 7"), "the form follows: {err}");
        assert!(err.contains("--project_root . --test_stack pytest"), "{err}");
    }
}

#[test]
fn a_door_refusal_is_followed_by_the_kinds_form_and_a_bare_number_is_taken_as_text() {
    let p = planned_to_line_7("door-forms");
    // a missing field: the facts door's line, then the form
    let (code, _, err) = p.run(&["claim", "criterion", "--criterion_id", "c2"]);
    assert_eq!(code, 1);
    assert!(err.starts_with("familiar-ai: not claimed yet · ") && err.contains("criterion is missing the required field 'text'\n"), "{err}");
    assert!(err.contains("familiar-ai claim criterion: line 2") && err.contains("--path          one of full|light  required"), "{err}");
    assert!(err.contains("example:\n  familiar-ai claim criterion --criterion_id c2 --text 'greet says hello' --path full --behavior_id b1 --promise_id p1"), "the project's next free id: {err}");
    // a key of another kind: the same
    let (code, _, err) = p.run(&["claim", "behavior", "--criterion_id", "c1", "--text", "x"]);
    assert_eq!(code, 1);
    assert!(err.contains("does not register the key 'criterion_id'") && err.contains("familiar-ai claim behavior: line 3"), "{err}");
    // a bare number where text is wanted: the text
    // definition change (the owner, 2026-10-15): every link names its string's head, so a head that is no head is refused at the claim
    let (code, out, err) = p.run(&["claim", "criterion_strung_on_head", "--criterion_id", "2", "--head", "1"]);
    assert_eq!(code, 1, "{out}");
    assert!(err.starts_with("familiar-ai: not claimed yet · 1 is not a head; it is on no string: claim criterion_head --criterion_id 1 first"), "{err}");
    assert!(err.contains("familiar-ai claim criterion_strung_on_head: line 9"), "the form follows: {err}");
    // the binary's own kind: refused before the door, with the listing
    let (code, _, err) = p.run(&["claim", "commit_seen", "--commit", "abc"]);
    assert_eq!(code, 1);
    assert!(err.contains("written by familiar-ai itself") && err.contains("\n  intent_submitted "), "{err}");
}
