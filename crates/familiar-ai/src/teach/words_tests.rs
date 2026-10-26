//! Tests that the model's terminal words are shown in the person's words, with claim lines kept.

use super::*;
use familiar_ai_rules::sample::criterion_fixture;

fn fixture() -> Log {
    criterion_fixture(&["c1", "c2"], true, false, 0.0, "t").expect("fixture")
}

#[test]
fn every_hold_has_its_pinned_words() {
    let log = fixture();
    let v = View::new(&log);
    let table = [
        ("planning_incomplete", "the plan comes first"),
        ("proof_tampered", "the sealed test was changed; fix the code without editing the test, or commit the changed test alone and see it fail"),
        ("tests_not_red", "the test must fail on the code as it is, committed alone before any fix; reverting a fix does not count"),
        ("not_stamped", "the code landed before its test was sealed"),
        ("tests_and_code_mixed", "tests and code are in one commit; the tests go first, alone"),
        ("anchor_untouched", "the change landed outside the place the plan named"),
        ("anchor_moved", "the code at the plan's place changed after the plan was made"),
        ("command_changed", "the test command changed after the test was sealed"),
        ("no_teeth", "the test still passes with the change taken back out"),
        ("regression", "a test that passed before fails now"),
        ("released", "released by you"),
    ];
    for (why, words) in table {
        assert_eq!(hold(&v, Some("c1"), why), words, "{why}");
    }
    assert_eq!(hold(&v, None, "something_new"), "something new", "an unknown hold still reads");
}

#[test]
fn two_failures_says_who_takes_it_and_this_needs_you_when_the_ladder_is_spent() {
    let log = fixture();
    let v = View::new(&log);
    assert_eq!(hold(&v, None, "two_failures"), "two tries failed; a stronger model is taking over");
    // the fixture's holder runs no model the ladder knows of: the rung is fable's
    let rung = ladder::investigator_rung(&v, "c1");
    let expected = if rung.is_some() { "two tries failed; a stronger model is taking over" } else { TWO_FAILURES_NEEDS_YOU };
    assert_eq!(hold(&v, Some("c1"), "two_failures"), expected);
}

#[test]
fn a_criterion_is_its_own_text_in_quotes_and_a_whole_id_only() {
    let log = fixture();
    let v = View::new(&log);
    assert_eq!(quoted(&v, "c1"), "\"a right password opens a session\"");
    assert_eq!(quoted(&v, "c9"), "c9", "no text: the id");
    assert_eq!(
        named(&v, "criterion c1 must be green; Criterion c2 waits; criterion c10 is not c1"),
        "\"a right password opens a session\" must be green; \"a reset link reaches the person\" waits; criterion c10 is not c1"
    );
}

#[test]
fn a_held_refusal_is_one_line_with_what_comes_next_and_the_claim_lines_stay() {
    let log = fixture();
    let model = "refused · criterion c1 · held: tests_not_red\n  its tests passed at the tests-first commit; they must fail before the code is written\n  to clear it: commit its tests alone, with no code, and see them fail (commit_seen)\n  write a test that fails on the code as it is now; familiar-ai runs python -m pytest after each commit\n  claim it: familiar-ai claim test_change_announced --criterion_id c1 --reason 'x'";
    assert_eq!(
        refusal(&log, model),
        "familiar-ai: \"a right password opens a session\" · the test must fail on the code as it is, committed alone before any fix; reverting a fix does not count · next: commit its tests alone, with no code, and see them fail\n  write a test that fails on the code as it is now; familiar-ai runs python -m pytest after each commit\n  claim it: familiar-ai claim test_change_announced --criterion_id c1 --reason 'x'"
    );
    let run = "refused · the run · held: planning_incomplete\n  the plan is not complete yet\n  to clear it: name the command that runs the tests (test_stack_detected)\n  claim it: familiar-ai claim test_stack_detected --project_root .";
    assert_eq!(
        refusal(&log, run),
        "familiar-ai: the plan comes first · next: name the command that runs the tests\n  claim it: familiar-ai claim test_stack_detected --project_root ."
    );
}

#[test]
fn a_not_yet_is_not_done_yet_with_the_next_step_beneath() {
    let log = fixture();
    let model = "not yet · criterion c1 · testing\n  it is its turn: the tests come first\n  first: its tests must be committed alone, seen failing and stamped\n  next: commit its tests alone, with no code, and see them fail (commit_seen)";
    assert_eq!(
        refusal(&log, model),
        "familiar-ai: not done yet · \"a right password opens a session\" · the test comes first\n  next: commit its tests alone, with no code, and see them fail"
    );
    let sealed = "not yet · criterion c1 · sealed\n  x\n  first: y\n  next: commit the code on its anchor (commit_seen)";
    assert!(refusal(&log, sealed).starts_with("familiar-ai: not done yet · \"a right password opens a session\" · the code comes next\n"));
    let waiting = "not yet · criterion c2 · waiting\n  x\n  first: criterion c1 must be green or proven; it is testing\n  next: —";
    assert_eq!(refusal(&log, waiting), "familiar-ai: not done yet · \"a reset link reaches the person\" · \"a right password opens a session\" comes first");
}

#[test]
fn the_clear_words_drop_the_kind_and_say_the_choices_and_the_rung_in_words() {
    let log = fixture();
    let v = View::new(&log);
    assert_eq!(
        clear(&v, "the person answers its card: re_plan, keep_going or release (hold_answered); or an investigator one rung up takes it: a fresh sonnet subagent (criterion_transfer)"),
        "the person answers its card: plan it again, keep going as planned or release it; or a stronger model (sonnet) takes it over"
    );
    assert_eq!(clear(&v, "make the first edit or commit (the agent's first edit or commit)"), "make the first edit or commit");
}

#[test]
fn text_that_is_not_the_terminals_is_led_and_named() {
    let log = fixture();
    assert_eq!(refusal(&log, "criterion c1 is with a stronger model\n  claim it: x c1"), "familiar-ai: \"a right password opens a session\" is with a stronger model\n  claim it: x c1");
    assert_eq!(lead("familiar-ai: x"), "familiar-ai: x");
    let root = std::path::Path::new("/nonexistent/familiar-ai-unit");
    let outside = "this edit is outside the anchor of c1; it is told, not refused";
    let head = format!("familiar-ai: this edit is outside the place the plan named for \"a right password opens a session\", {}; it goes ahead", anchor_words(&familiar_ai_rules::View::new(&log), "c1"));
    // with no edited file known, the place alone
    assert_eq!(told(&log, root, outside, None), head);
    // with the file, the way out: ask the person, then claim it as the new place
    let told = told(&log, root, outside, Some("src/other.py"));
    assert!(told.starts_with(&format!("{head}\n  if the fix belongs elsewhere: ask the person with AskUserQuestion, and on a yes claim the new place, said first: familiar-ai claim place_chosen --criterion_id c1 --places '[{{\"file_path\":\"src/other.py\",")), "{told}");
    assert!(told.ends_with("; the proof of \"a right password opens a session\" starts over at the new place"), "{told}");
}

#[test]
fn the_status_names_each_criterion_where_it_stands() {
    let log = fixture();
    assert_eq!(
        status_lines(&log, false),
        [
            "familiar-ai: the plan is confirmed",
            "familiar-ai: \"a right password opens a session\" · the test comes first",
            "familiar-ai: \"a reset link reaches the person\" · waits for \"a right password opens a session\"",
        ]
    );
    let empty: Log = Vec::new();
    assert_eq!(status_lines(&empty, false), ["familiar-ai: the plan is still being written"]);
    assert_eq!(state_lines(&empty, &log, false).len(), 2);
    assert!(state_lines(&log, &log, false).is_empty());
}

/// A run's translation as the binary writes it (plan §1.1).
fn translation(label: &str, evidence: &str, failing: &[&str]) -> familiar_ai_facts::Fact {
    let fields = serde_json::json!({
        "run_id": "r-ab12-1", "criterion_id": "c1", "commit": "ab12", "by": "regex", "label": label,
        "evidence": evidence, "failing_tests": failing, "classified_on": "full",
    });
    familiar_ai_facts::append_raw(&Vec::new(), "test_log_translation", fields.as_object().cloned().unwrap_or_default(), 0.0, "t").expect("a translation")[0].clone()
}

#[test]
fn every_outcome_a_table_gives_has_its_line() {
    // plan §6.1 (S8): the lines come from OUTCOMES, one row per outcome
    let mut labels: Vec<&str> = crate::door::runner_tables::TABLES.iter().flat_map(|t| t.rows.iter().map(|r| r.outcome)).collect();
    labels.extend(["passed", "timed_out", "crashed", "killed", "unclassified"]);
    for label in labels {
        assert!(OUTCOMES.iter().any(|(l, _, _)| *l == label), "no OUTCOMES row for {label}");
    }
}

#[test]
fn an_outcome_line_says_what_ran_with_its_evidence_and_run() {
    let t = translation("environment_error", "sh: 1: pytset: not found", &[]);
    assert_eq!(outcome_line(&t).as_deref(), Some("familiar-ai: the test command did not run: sh: 1: pytset: not found · next: fix the command or the environment · run r-ab12-1"));
    let t = translation("tests_failed", "AssertionError: expected hello", &["test_greet", "test_bye"]);
    let line = outcome_line(&t).expect("a line");
    assert!(line.starts_with("familiar-ai: the tests fail: AssertionError: expected hello · failing: test_greet, test_bye"), "{line}");
    assert!(line.ends_with(" · run r-ab12-1"), "{line}");
    // the facts' form admits only these labels, and each has its row
    assert_eq!(OUTCOMES.len(), 13);
}

#[test]
fn the_started_line_names_each_run() {
    assert_eq!(started(&["r-ab12-1".to_string()]), "familiar-ai: tests started — run r-ab12-1; the result is told when it ends");
    assert_eq!(started(&["r-1".to_string(), "r-2".to_string()]), "familiar-ai: tests started — runs r-1, r-2; the result is told when it ends");
}

/// c1 at its tests-first commit, its run read can't-tell (H-4): `extra` joins the translation.
fn cant_tell_before_the_seal(extra: serde_json::Value) -> Log {
    use familiar_ai_rules::sample::apply_move;
    use serde_json::json;
    let step = |log: &Log, label: &str| -> Log {
        let v = View::new(log);
        let m = familiar_ai_rules::moves::criterion_admissible(&v).into_iter().find(|m| m.label.as_deref() == Some(label)).unwrap_or_else(|| panic!("no {label} move"));
        apply_move(log, &m, 0.0, "t").expect("applied")
    };
    let raw = |log: &Log, kind: &str, fields: serde_json::Value| -> Log {
        familiar_ai_facts::append_raw(log, kind, fields.as_object().cloned().unwrap_or_default(), 0.0, "t").expect(kind)
    };
    let mut log = criterion_fixture(&["c1"], true, false, 0.0, "t").expect("fixture");
    for s in ["start_check", "tests_only", "test_written"] {
        log = step(&log, s);
    }
    let id = log.iter().rev().find(|f| f.kind == "commit_seen").and_then(|f| f.fields.get("commit")).and_then(Value::as_str).unwrap_or("").to_string();
    log = raw(&log, "run_reserved", json!({"run_id": "r-1", "criterion_id": "c1", "criterion_rev": "R1", "commit": id, "command": "pytest -q", "resources": ["tests"], "wait": "60", "by": "binary"}));
    log = raw(&log, "run_ended", json!({"run_id": "r-1", "outcome": "failed", "end": "exited"}));
    let mut t = json!({"run_id": "r-1", "criterion_id": "c1", "commit": id, "by": "regex", "label": "unclassified", "evidence": "boom", "failing_tests": [], "classified_on": "full"});
    if let (Value::Object(m), Value::Object(x)) = (&mut t, extra) {
        m.extend(x);
    }
    log = raw(&log, "test_log_translation", t);
    for (fp, o) in [("tests/test_c1.py", "failed"), ("tests/test_login_old.py", "passed")] {
        log = raw(&log, "pass_please_pass", json!({"criterion_id": "c1", "file_path": fp, "commit": id, "outcome": o, "ran_in": "laptop", "with_anchor_reverted": false, "run_id": "r-1"}));
    }
    step(&log, "fingerprint_stamped")
}

#[test]
fn the_cant_tell_card_tells_its_run_in_short_sentences_on_the_persons_card_and_in_status() {
    // H-4 (the owner, 2026-10-19): what ran, the counts, the deciding line, the failing test and
    // where, why Familiar couldn't tell, where the full output is; never the log itself
    let log = cant_tell_before_the_seal(serde_json::json!({
        "evidence": "FAILED tests/test_c1.py::test_login - KeyError: 'id'",
        "failing_tests": ["tests/test_c1.py::test_login"],
        "location": {"file_path": "tests/test_c1.py", "line": 12, "function": "test_login", "kind": "exception"},
        "counts": {"passed": 3, "failed": 1},
    }));
    let v = View::new(&log);
    let card = familiar_ai_rules::terminal::cards(&v).into_iter().find(|c| c.hold == "run_unclassified").expect("the card");
    let summary: Vec<String> = card.to_json()["summary"].as_array().map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    assert!(summary.iter().any(|l| l.contains("1 failed, 3 passed")), "{summary:?}");
    // status --json's card carries the same sentences, for the pane
    let j = crate::teach::card_json(&v, &card, None);
    assert_eq!(j["summary"], serde_json::json!(summary), "{j}");
    assert!(j["choices"].as_array().is_some_and(|cs| cs.iter().any(|c| c == "its_the_red")), "{j}");
    // the card as the person reads it on the picker: its lines and each option's name and description
    let red = j["person"]["options"].as_array().and_then(|os| os.iter().find(|o| o["choice"] == "its_the_red")).expect("its_the_red for the person").clone();
    assert_eq!(red["label"], "It's the bug", "{red}");
    assert!(j["person"]["lines"].as_array().is_some_and(|ls| ls.iter().any(|l| l.as_str().is_some_and(|l| l.starts_with(crate::teach::person_words::WHAT_HAPPENED)))), "{j}");
    // the person's card: what happened, then the run's sentences, each on its own line
    let a = crate::teach::person_words::asked(&v, &card);
    let lines = crate::teach::person_words::lines(&a);
    let at = lines.iter().position(|l| l.starts_with(crate::teach::person_words::WHAT_HAPPENED)).expect("what happened");
    for (i, s) in summary.iter().enumerate() {
        assert_eq!(lines.get(at + 1 + i), Some(&format!("  {s}")), "{lines:#?}");
    }
    // its_the_red in the person's words, which name no field, id or sha
    let red = a.choices.iter().find(|(ch, _)| ch == "its_the_red").map(|(_, w)| w.clone()).expect("its_the_red on the person's card");
    assert!(red.contains("part 1"), "{red}");
}

#[test]
fn the_agent_may_not_reword_or_leave_out_a_fact_of_the_run_and_a_picked_name_is_its_choice() {
    // 2026-10-20: what happened and the run's lines are the binary's, word for word; the agent's
    // own sentence is the only free text; the picker shows a short name with when to pick it
    let log = cant_tell_before_the_seal(serde_json::json!({
        "evidence": "FAILED tests/test_c1.py::test_login - KeyError: 'id'",
        "failing_tests": ["tests/test_c1.py::test_login"],
        "location": {"file_path": "tests/test_c1.py", "line": 12, "function": "test_login", "kind": "exception"},
        "counts": {"passed": 3, "failed": 1},
    }));
    let v = View::new(&log);
    let card = familiar_ai_rules::terminal::cards(&v).into_iter().find(|c| c.hold == "run_unclassified").expect("the card");
    let pw = crate::teach::person_words::asked(&v, &card);
    let whole = format!("{}\n  {} \"My test shows the bug.\"\n  {}", crate::teach::person_words::lines(&pw).join("\n"), crate::teach::person_words::AGENTS_WORDS, crate::teach::person_words::WHAT_NOW);
    assert!(crate::teach::person_words::missing(&pw, &whole).is_empty(), "{:?}", crate::teach::person_words::missing(&pw, &whole));
    // a run line reworded: refused, naming the line
    let counts = pw.run.iter().find(|l| l.starts_with("Counts:")).expect("the counts line").clone();
    let reworded = whole.replace(&counts, "Counts: some tests failed.");
    let missing = crate::teach::person_words::missing(&pw, &reworded);
    assert!(missing.iter().any(|m| m.contains(&counts)), "{missing:?}");
    // what happened left out: refused
    let without = whole.replace(&pw.happened, "it broke.");
    assert!(crate::teach::person_words::missing(&pw, &without).iter().any(|m| m.starts_with("what happened")), "refused");
    // the picker: a short name, the line as its description; the name picked is the choice
    let options = crate::teach::picker::card_options(&v, &card);
    let red = options.as_array().and_then(|os| os.iter().find(|o| o["label"] == "It's the bug")).expect("It's the bug").clone();
    assert!(red["description"].as_str().is_some_and(|d| d.contains("→")), "{red}");
    assert_eq!(crate::teach::picker::choice_of(&v, &card, "It's the bug").as_deref(), Some("its_the_red"));
}

// the contract [draft names]: a promise that gives cases without an opposite one leaves line 3
// missing, and its row says what the cases want with the promise to claim again
#[test]
fn the_checklist_says_what_a_promises_cases_want() {
    let log = criterion_fixture(&["c1"], true, false, 0.0, "t").expect("fixture");
    let root = std::env::temp_dir();
    let project = crate::teach::example::Project { log: &log, root: &root };
    assert!(plan_checklist(&project)[3].starts_with("  ✓ 3 full path only  "), "{:?}", plan_checklist(&project));
    let case = |id: &str, kind: &str| serde_json::json!({ "case_id": id, "kind": kind, "given": "g", "expect": "e" });
    let promise = |cases: Vec<serde_json::Value>| {
        let p = serde_json::json!({ "promise_id": "p1", "behavior_id": "b1", "accepts": "a", "gives_back": "g", "refuses": "r", "cases": cases });
        familiar_ai_facts::append_raw(&log, "promise", p.as_object().cloned().unwrap_or_default(), 1.0, "t").expect("promise")
    };
    let short = promise(vec![case("n1", "normal")]);
    let project = crate::teach::example::Project { log: &short, root: &root };
    let rows = plan_checklist(&project);
    assert!(
        rows[3].starts_with("    3 full path only  promise p1's cases want an opposite case (the same call with the trigger absent), or --opposite_none with the reason there is none · claim it again: familiar-ai claim promise --promise_id p1 --behavior_id b1 --accepts a --gives_back g --refuses r --cases "),
        "{rows:?}"
    );
    let whole = promise(vec![case("n1", "normal"), case("o1", "opposite")]);
    let project = crate::teach::example::Project { log: &whole, root: &root };
    let rows = plan_checklist(&project);
    assert!(rows[3].starts_with("  ✓ 3 full path only  ") && rows[3].ends_with(", promise p1 (cases n1, o1)"), "{rows:?}");
}
