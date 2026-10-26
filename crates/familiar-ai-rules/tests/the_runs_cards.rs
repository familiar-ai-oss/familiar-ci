//! The run's holds and cards (plan §6.1, §6.3, §6.4; lane S7): a run broken before the seal is
//! sent back (`red_for_another_reason`); a crash before the seal, an environment the agent cannot
//! fix, a run no rule reads, a run spent on its retry, a command that broke after it worked, and a
//! part its dependency covered are each the person's card; two attempts of one family are
//! `two_<family>`. Each answer does what its row says.

mod common;

use common::{raw, step, whys, AT, TENANT, TEST, TOUCHING};
use familiar_ai_facts::Log;
use familiar_ai_rules::criterion::{answers_of, sealed_at, Place, PAUSING};
use familiar_ai_rules::outcome::{result_family, Family};
use familiar_ai_rules::sample::criterion_fixture;
use familiar_ai_rules::{lifecycle, moves, plan, terminal, View};
use serde_json::{json, Value};

fn tests_first() -> Log {
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).unwrap_or_default();
    let log = step(&log, "start_check", None);
    let log = step(&log, "tests_only", None);
    step(&log, "test_written", None)
}

fn head(log: &Log) -> String {
    log.iter().rev().find(|f| f.kind == "commit_seen").and_then(|f| f.fields.get("commit")).and_then(Value::as_str).unwrap_or("").to_string()
}

/// A run of `command` at the latest commit, translated as `label`.
fn ran_with(log: &Log, command: &str, word: &str, label: &str, extra: Value) -> Log {
    let id = head(log);
    let rid = format!("r-{}", log.len());
    let mut log = raw(log, "run_reserved", json!({"run_id": rid, "criterion_id": "c1", "criterion_rev": "R1", "commit": id, "command": command, "resources": ["tests"], "wait": "60", "by": "binary"}));
    log = raw(&log, "run_ended", json!({"run_id": rid, "outcome": word, "end": "exited"}));
    let mut t = json!({"run_id": rid, "criterion_id": "c1", "commit": id, "by": "regex", "label": label, "evidence": "boom", "failing_tests": [], "classified_on": "full"});
    if let (Value::Object(m), Value::Object(x)) = (&mut t, extra) {
        m.extend(x);
    }
    log = raw(&log, "test_log_translation", t);
    for (fp, o) in [(TEST, word), (TOUCHING, "passed")] {
        log = raw(&log, "pass_please_pass", json!({"criterion_id": "c1", "file_path": fp, "commit": id, "outcome": o, "ran_in": "laptop", "with_anchor_reverted": false, "run_id": rid}));
    }
    step(&log, "fingerprint_stamped", None)
}

/// A re-run owed after an answer: the binary's run at the latest commit, translated as `label`,
/// stamped only when the moves offer the stamp.
fn rerun(log: &Log, word: &str, label: &str) -> Log {
    let id = head(log);
    let rid = format!("r-{}", log.len());
    let mut log = raw(log, "run_reserved", json!({"run_id": rid, "criterion_id": "c1", "criterion_rev": "R1", "commit": id, "command": "pytest -q", "resources": ["tests"], "wait": "60", "by": "binary"}));
    log = raw(&log, "run_ended", json!({"run_id": rid, "outcome": word, "end": "exited"}));
    log = raw(&log, "test_log_translation", json!({"run_id": rid, "criterion_id": "c1", "commit": id, "by": "regex", "label": label, "evidence": "boom", "failing_tests": [], "classified_on": "full"}));
    for (fp, o) in [(TEST, word), (TOUCHING, "passed")] {
        log = raw(&log, "pass_please_pass", json!({"criterion_id": "c1", "file_path": fp, "commit": id, "outcome": o, "ran_in": "laptop", "with_anchor_reverted": false, "run_id": rid}));
    }
    let v = View::new(&log);
    match moves::criterion_admissible(&v).into_iter().find(|m| m.label.as_deref() == Some("fingerprint_stamped")) {
        Some(m) => familiar_ai_rules::sample::apply_move(&log, &m, AT, TENANT).expect("stamp"),
        None => log,
    }
}

fn ran(log: &Log, word: &str, label: &str) -> Log {
    ran_with(log, "pytest -q", word, label, json!({}))
}

fn code(log: &Log, label: &str) -> Log {
    ran(&step(log, "code", None), "failed", label)
}

fn answer(log: &Log, hold: &str, choice: &str) -> Log {
    raw(log, "hold_answered", json!({"criterion_id": "c1", "hold": hold, "choice": choice, "typed": ""}))
}

fn has(log: &Log, why: &str) -> bool {
    whys(log).iter().any(|w| w == why)
}

fn card_choices(log: &Log, hold: &str) -> Vec<String> {
    let v = View::new(log);
    terminal::cards(&v).into_iter().find(|c| c.hold == hold).map(|c| c.choices.into_iter().map(|(ch, _)| ch).collect()).unwrap_or_default()
}

fn sealed() -> Log {
    ran(&tests_first(), "failed", "tests_failed")
}

#[test]
fn the_new_holds_are_the_persons_cards_with_their_choices_and_they_pause() {
    // Decided B (the owner, 2026-10-15): a_dependency joins the two_<outcome> cards
    let all: [(&str, &[&str]); 9] = [
        ("two_test_broken", &["try_again", "re_plan", "a_dependency", "release"]),
        ("two_crashed", &["try_again", "re_plan", "a_dependency", "release"]),
        ("two_timed_out", &["try_again", "re_plan", "a_dependency", "release"]),
        ("environment_error", &["try_again", "re_plan", "release"]),
        // H-4 (the owner, 2026-10-19): the person confirms what Familiar can't read, each at its stage
        ("run_unclassified", &["try_again", "its_the_red", "its_the_pass", "re_plan", "release"]),
        ("retry_spent", &["try_again", "re_plan", "release"]),
        ("crashed", &["its_the_bug", "a_dependency", "release"]),
        ("command_broke", &["fix_what_broke_it", "a_dependency", "release"]),
        ("miscoped", &["covered_by_dependency", "re_plan", "release"]),
    ];
    for (hold, choices) in all {
        assert_eq!(answers_of(hold), Some(choices), "{hold}");
        assert!(PAUSING.contains(&hold), "{hold}");
    }
    assert!(answers_of("red_for_another_reason").is_none(), "a refusal, not a card");
}

#[test]
fn a_broken_or_slow_test_before_the_seal_is_sent_back_never_sealed() {
    for label in ["collection_error", "no_tests_collected", "setup_error", "timed_out"] {
        let log = ran(&tests_first(), "failed", label);
        assert!(has(&log, "red_for_another_reason"), "{label}: {:?}", whys(&log));
        assert!(sealed_at(&View::new(&log), "c1", Place::Laptop).is_none(), "{label}");
        assert!(terminal::cards(&View::new(&log)).is_empty(), "{label}: no card");
        // a new tests commit that goes red clears it
        let log = ran(&step(&log, "changed_test", None), "failed", "tests_failed");
        assert!(!has(&log, "red_for_another_reason"), "{label}: {:?}", whys(&log));
    }
}

#[test]
fn a_crash_before_the_seal_is_the_card_and_its_the_bug_seals_on_it() {
    let log = ran(&tests_first(), "failed", "crashed");
    assert!(has(&log, "crashed"), "{:?}", whys(&log));
    assert_eq!(card_choices(&log, "crashed"), ["its_the_bug", "a_dependency", "release"]);
    let v = View::new(&log);
    let q = terminal::cards(&v).into_iter().find(|c| c.hold == "crashed").expect("card").question;
    assert!(q.contains("crashed before any fix") && q.contains("the crash is not at"), "{q}");
    assert!(lifecycle::paused(&v, "c1"));
    let log = answer(&log, "crashed", "its_the_bug");
    let v = View::new(&log);
    assert!(sealed_at(&v, "c1", Place::Laptop).is_some(), "sealed on the crash");
    assert!(!has(&log, "crashed"), "{:?}", whys(&log));
    assert_eq!(lifecycle::state(&v, "c1"), "sealed");
}

#[test]
fn a_dependency_on_the_crash_card_is_recorded_and_the_card_closes() {
    let log = answer(&ran(&tests_first(), "failed", "crashed"), "crashed", "a_dependency");
    assert!(!has(&log, "crashed"));
    assert!(sealed_at(&View::new(&log), "c1", Place::Laptop).is_none(), "not sealed: the dependency comes first");
}

#[test]
fn the_cards_of_the_environment_an_unread_run_and_a_spent_retry() {
    for (label, hold) in [("environment_error", "environment_error"), ("unclassified", "run_unclassified"), ("killed", "retry_spent"), ("runner_crash", "retry_spent")] {
        let log = code(&sealed(), label);
        assert!(has(&log, hold), "{label}: {:?}", whys(&log));
        // H-4: after the seal the can't-tell card also offers its_the_pass
        let want: &[&str] = if hold == "run_unclassified" { &["try_again", "its_the_pass", "re_plan", "release"] } else { &["try_again", "re_plan", "release"] };
        assert_eq!(card_choices(&log, hold), want, "{label}");
        // try_again: the same commit is owed again for exactly one re-run
        let log = answer(&log, hold, "try_again");
        assert!(!has(&log, hold), "{label}");
        let v = View::new(&log);
        assert_eq!(moves::reruns(&v, "c1").len(), 1, "{label}: one re-run");
    }
    // an environment error at the tests-first commit is the card too
    let log = ran(&tests_first(), "failed", "environment_error");
    assert!(has(&log, "environment_error"), "{:?}", whys(&log));
}

#[test]
fn a_command_that_never_worked_is_sent_back_and_one_that_broke_is_the_card() {
    let log = ran_with(&tests_first(), "pytset -q", "failed", "usage_error", json!({}));
    assert!(has(&log, "red_for_another_reason"), "never valid: {:?}", whys(&log));
    assert!(!has(&log, "command_broke"));
    // valid before: a red run with the command, then a code commit whose run is a usage error
    let log = ran_with(&step(&sealed(), "code", None), "pytest -q", "failed", "usage_error", json!({}));
    assert!(has(&log, "command_broke"), "{:?}", whys(&log));
    assert!(!has(&log, "red_for_another_reason"));
    assert_eq!(card_choices(&log, "command_broke"), ["fix_what_broke_it", "a_dependency", "release"]);
    // fix_what_broke_it locks the command until a run that is not the command's breakage
    let log = answer(&log, "command_broke", "fix_what_broke_it");
    let v = View::new(&log);
    assert!(!has(&log, "command_broke"));
    assert!(lifecycle::command_locked(&v, "c1"));
    // only the stack root covering the locked part's files (§6.3: "for this criterion")
    assert_eq!(lifecycle::command_locked_at(&v, "src/").as_deref(), Some("c1"));
    assert_eq!(lifecycle::command_locked_at(&v, "web/"), None);
    assert!(!moves::criterion_admissible(&v).iter().any(|m| m.kind == "test_stack_detected"), "no new command while locked");
    let log = code(&log, "tests_failed");
    assert!(!lifecycle::command_locked(&View::new(&log), "c1"), "freed by a run that is not the breakage");
}

#[test]
fn two_attempts_of_one_family_are_its_card_and_try_again_gives_exactly_two_more() {
    let log = code(&code(&sealed(), "collection_error"), "setup_error");
    assert!(has(&log, "two_test_broken"), "{:?}", whys(&log));
    assert!(!has(&log, "two_failures"));
    assert_eq!(card_choices(&log, "two_test_broken"), ["try_again", "re_plan", "a_dependency", "release"]);
    let log = answer(&log, "two_test_broken", "try_again");
    assert!(!has(&log, "two_test_broken"));
    let log = code(&log, "collection_error");
    assert!(!has(&log, "two_test_broken"), "one more try: {:?}", whys(&log));
    let log = code(&log, "collection_error");
    assert!(has(&log, "two_test_broken"), "the second: {:?}", whys(&log));
    // two crashes after the seal, and two runs too slow
    assert!(has(&code(&code(&sealed(), "crashed"), "crashed"), "two_crashed"));
    assert!(has(&code(&code(&sealed(), "timed_out"), "timed_out"), "two_timed_out"));
}

#[test]
fn re_plan_on_a_runs_card_chooses_the_place_again() {
    for hold in ["two_test_broken", "environment_error", "run_unclassified", "retry_spent"] {
        let log = answer(&code(&sealed(), "environment_error"), hold, "re_plan");
        assert!(plan::unchosen(&View::new(&log), "c1").is_some(), "{hold}");
    }
}

// miscoped: see the_miscoped_card_stands_until_answered.rs (S9a's re-earn is its definition).

/// The latest run's id: the card's run in these logs.
fn last_run(log: &Log) -> String {
    log.iter().rev().find(|f| f.kind == "run_reserved").and_then(|f| f.fields.get("run_id")).and_then(Value::as_str).unwrap_or("").to_string()
}

/// The person's answer on the can't-tell card, bound to the run it answers.
fn confirm(log: &Log, choice: &str, run_id: &str) -> Log {
    raw(log, "hold_answered", json!({"criterion_id": "c1", "hold": "run_unclassified", "choice": choice, "typed": "", "run_id": run_id}))
}

/// The admissible answers `choice` on the can't-tell card, as the moves offer them.
fn offered(log: &Log, choice: &str) -> Vec<Value> {
    let v = View::new(log);
    moves::criterion_admissible(&v)
        .into_iter()
        .filter(|m| m.kind == "hold_answered" && m.fields.get("hold").and_then(Value::as_str) == Some("run_unclassified") && m.fields.get("choice").and_then(Value::as_str) == Some(choice))
        .map(|m| Value::Object(m.fields.clone()))
        .collect()
}

fn own_result(log: &Log) -> familiar_ai_facts::Fact {
    log.iter().rev().find(|f| f.kind == "pass_please_pass" && f.fields.get("file_path").and_then(Value::as_str) == Some(TEST)).cloned().expect("a result")
}

#[test]
fn at_the_tests_first_commit_the_cant_tell_card_offers_its_the_red_and_it_seals_on_that_run() {
    // H-4 (the owner, 2026-10-19): the person confirms what Familiar can't read
    let log = ran(&tests_first(), "failed", "unclassified");
    assert!(has(&log, "run_unclassified"), "{:?}", whys(&log));
    assert_eq!(card_choices(&log, "run_unclassified"), ["try_again", "its_the_red", "re_plan", "release"]);
    assert!(offered(&log, "its_the_pass").is_empty(), "no pass before the seal");
    // the answer the moves offer is bound to the card's run
    let rid = last_run(&log);
    let bound = offered(&log, "its_the_red");
    assert_eq!(bound.len(), 1, "{bound:?}");
    assert_eq!(bound[0]["run_id"], json!(rid));
    assert!(sealed_at(&View::new(&log), "c1", Place::Laptop).is_none());
    // an its_the_red bound to no run, or to another run, seals nothing
    let loose = answer(&log, "run_unclassified", "its_the_red");
    assert!(sealed_at(&View::new(&loose), "c1", Place::Laptop).is_none(), "unbound");
    let other = confirm(&log, "its_the_red", "r-0");
    assert!(sealed_at(&View::new(&other), "c1", Place::Laptop).is_none(), "another run");
    // bound to the card's run: sealed there, the card closed
    let log = confirm(&log, "its_the_red", &rid);
    let v = View::new(&log);
    assert!(sealed_at(&v, "c1", Place::Laptop).is_some(), "sealed on the confirmed run");
    assert!(!has(&log, "run_unclassified"), "{:?}", whys(&log));
    assert_eq!(lifecycle::state(&v, "c1"), "sealed");
    // the reading itself stays can't-tell: the teeth read it as strictly as before
    assert_eq!(result_family(&v, "c1", &own_result(&log)), Family::Unknown);
}

#[test]
fn after_the_seal_the_cant_tell_card_offers_its_the_pass_and_it_is_green_at_that_run() {
    let log = ran(&step(&sealed(), "code", None), "passed", "unclassified");
    assert!(has(&log, "run_unclassified"), "{:?}", whys(&log));
    assert_eq!(card_choices(&log, "run_unclassified"), ["try_again", "its_the_pass", "re_plan", "release"]);
    assert!(offered(&log, "its_the_red").is_empty(), "no red after the seal");
    let rid = last_run(&log);
    let bound = offered(&log, "its_the_pass");
    assert_eq!(bound.len(), 1, "{bound:?}");
    assert_eq!(bound[0]["run_id"], json!(rid));
    assert_ne!(lifecycle::state(&View::new(&log), "c1"), "green");
    // its_the_pass bound to another run is no green
    let other = confirm(&log, "its_the_pass", "r-0");
    assert_ne!(lifecycle::state(&View::new(&other), "c1"), "green", "another run");
    let log = confirm(&log, "its_the_pass", &rid);
    let v = View::new(&log);
    assert!(!has(&log, "run_unclassified"), "{:?}", whys(&log));
    assert_eq!(lifecycle::state(&v, "c1"), "green");
    assert_eq!(result_family(&v, "c1", &own_result(&log)), Family::Unknown, "the reading is unchanged");
}

#[test]
fn the_cant_tell_card_says_what_ran_the_counts_the_deciding_line_where_why_and_the_full_output() {
    let extra = json!({
        "evidence": "FAILED tests/test_c1.py::test_login - KeyError: 'id'",
        "failing_tests": ["tests/test_c1.py::test_login"],
        "location": {"file_path": "tests/test_c1.py", "line": 12, "function": "test_login", "kind": "exception"},
        "counts": {"passed": 3, "failed": 1},
    });
    let log = ran_with(&tests_first(), "pytest -q", "failed", "unclassified", extra);
    let commit = head(&log);
    let v = View::new(&log);
    let card = terminal::cards(&v).into_iter().find(|c| c.hold == "run_unclassified").expect("the card");
    let said = summary(&card).join("\n");
    let full = format!(".familiar/runs/{commit}.log");
    for piece in [
        "pytest -q",
        "1 failed, 3 passed",
        "FAILED tests/test_c1.py::test_login - KeyError: 'id'",
        "tests/test_c1.py:12",
        "test_login",
        "the output's summary wasn't recognised",
        full.as_str(),
    ] {
        assert!(said.contains(piece), "{piece:?} missing from {said}");
        assert!(card.text.contains(piece), "{piece:?} missing from the card's text {}", card.text);
    }
    // a run with no counts and no failing test says so, and never dumps the log
    let log = ran(&tests_first(), "failed", "unclassified");
    let card = terminal::cards(&View::new(&log)).into_iter().find(|c| c.hold == "run_unclassified").expect("the card");
    let lines = summary(&card);
    let said = lines.join("\n");
    assert!(said.contains("pytest -q") && said.contains("boom"), "{said}");
    assert!(lines.iter().all(|l| !l.contains('\n') && l.chars().count() <= 240), "{said}");
    // another card carries no run summary
    let log = ran(&tests_first(), "failed", "crashed");
    let card = terminal::cards(&View::new(&log)).into_iter().find(|c| c.hold == "crashed").expect("the card");
    assert!(summary(&card).is_empty());
}

/// The card's summary of its run, as `status --json` carries it: one short sentence a line.
fn summary(card: &terminal::Card) -> Vec<String> {
    card.to_json()["summary"].as_array().map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default()
}

#[test]
fn a_red_the_person_confirmed_counts_for_the_tamper_check_as_a_red_read() {
    // the owner, 2026-10-20: a confirmed red clears "changed after the seal" and is remembered,
    // so a later unannounced change away from it is caught
    let log = ran(&step(&sealed(), "changed_test", None), "failed", "unclassified");
    assert!(has(&log, "proof_tampered") && has(&log, "run_unclassified"), "{:?}", whys(&log));
    let log = confirm(&log, "its_the_red", &last_run(&log));
    assert!(!has(&log, "proof_tampered"), "the confirmed red clears it: {:?}", whys(&log));
    assert!(sealed_at(&View::new(&log), "c1", Place::Laptop).is_some());
    let log = ran(&step(&log, "code", None), "passed", "passed");
    assert!(!has(&log, "proof_tampered"), "{:?}", whys(&log));
    // after green a test changes only announced; announced or not, a change not seen red is tampering
    let log = ran(&step(&step(&log, "announce_after_green", None), "changed_test", None), "passed", "passed");
    assert!(has(&log, "proof_tampered"), "a change away from the confirmed red is caught: {:?}", whys(&log));
}

#[test]
fn a_cant_tell_red_at_the_tests_first_commit_stays_the_persons_card_after_code_lands() {
    // the bench's 13363 (2026-10-20): the code landed before anyone answered, the run passed, and
    // the card was gone: the part sat at "the test comes first" with nothing to answer
    let log = ran(&tests_first(), "failed", "unclassified");
    let red = last_run(&log);
    // the agent commits its code while the card is open: the commit lands whatever the moves offer
    let code = moves::commit_moves(&View::new(&log), "c1").into_iter().find(|m| m.label.as_deref() == Some("code")).expect("a code commit");
    let log = familiar_ai_rules::sample::apply_move(&log, &code, AT, TENANT).expect("the commit lands");
    let log = ran(&log, "passed", "passed");
    assert!(has(&log, "run_unclassified"), "the card stays: {:?}", whys(&log));
    let bound = offered(&log, "its_the_red");
    assert_eq!(bound.len(), 1, "{bound:?}");
    assert_eq!(bound[0]["run_id"], json!(red), "bound to the tests-first run");
    let log = confirm(&log, "its_the_red", &red);
    assert!(sealed_at(&View::new(&log), "c1", Place::Laptop).is_some(), "sealed on the confirmed run");
    assert!(!has(&log, "run_unclassified"), "{:?}", whys(&log));
    assert_eq!(lifecycle::state(&View::new(&log), "c1"), "green", "{:?}", whys(&log));
}

fn run_lines(log: &Log) -> Vec<String> {
    terminal::run_summary(&View::new(log), "c1")
}

#[test]
fn the_cards_call_path_is_the_reporters_frames_with_the_planned_code_marked_by_its_lines() {
    // 2026-10-20: every line of the path is a recorded frame; the planned code is marked only when
    // a frame's file is the place's and its line is inside the place's lines (src/login.py 40-62)
    let frames = json!([
        {"file_path": TEST, "line": 7, "function": "test_login"},
        {"file_path": "src/login.py", "line": 50, "function": "check_password"},
        {"file_path": "src/util.py", "line": 3, "function": "a"},
        {"file_path": "src/util.py", "line": 9, "function": "b"},
        {"file_path": "src/hashing.py", "line": 4, "function": "digest"}
    ]);
    let log = ran_with(&tests_first(), "pytest -q", "failed", "tests_failed", json!({"by": "reporter", "counts": {"failed": 1}, "frames": frames, "location": {"file_path": "src/hashing.py", "line": 4, "kind": "exception"}}));
    let s = run_lines(&log);
    let at = s.iter().position(|l| l == "Call path, from the test to the error:").unwrap_or_else(|| panic!("{s:#?}"));
    assert_eq!(
        s[at + 1..at + 6].to_vec(),
        [
            format!("  {TEST}:7 in test_login"),
            "  src/login.py:50 in check_password (the code the agent planned to fix)".to_string(),
            "  … 2 more calls …".to_string(),
            "  src/hashing.py:4 in digest (the error was raised here)".to_string(),
            "The error passes through check_password, the code the agent planned to fix.".to_string(),
        ]
    );
    // the planned file at a line outside the planned lines is not the planned code
    let outside = json!([{"file_path": TEST, "line": 7}, {"file_path": "src/login.py", "line": 90, "function": "other"}]);
    let log = ran_with(&tests_first(), "pytest -q", "failed", "tests_failed", json!({"by": "reporter", "counts": {"failed": 1}, "frames": outside, "location": {"file_path": "src/login.py", "line": 90, "kind": "exception"}}));
    let s = run_lines(&log);
    assert!(s.contains(&"  src/login.py:90 in other (the error was raised here)".to_string()), "{s:#?}");
    assert!(s.contains(&"The error does not pass through check_password, the code the agent planned to fix.".to_string()), "{s:#?}");
    // no frames recorded (the output tables): said so, nothing guessed
    let log = ran(&tests_first(), "failed", "unclassified");
    assert!(run_lines(&log).contains(&"Familiar has no call path for this run.".to_string()), "{:#?}", run_lines(&log));
}

#[test]
fn after_run_it_again_the_card_is_the_newer_runs_and_never_binds_to_the_answered_one() {
    // the checks crate's W15 (2026-10-20): a can't-tell red at the tests-first commit answered
    // try_again; the code lands and its run can't tell either: the card is that newer run's, and the
    // answered tests-first run is never what a confirm binds to (it is not at the tests-first commit,
    // so "it's the bug" is not offered at all)
    let log = ran(&tests_first(), "failed", "unclassified");
    let first = last_run(&log);
    let log = answer(&log, "run_unclassified", "try_again");
    let code = moves::commit_moves(&View::new(&log), "c1").into_iter().find(|m| m.label.as_deref() == Some("code")).expect("a code commit");
    let log = familiar_ai_rules::sample::apply_move(&log, &code, AT, TENANT).expect("the commit lands");
    let log = rerun(&log, "failed", "unclassified");
    let second = last_run(&log);
    assert_ne!(first, second);
    assert!(has(&log, "run_unclassified"), "{:?}", whys(&log));
    let card_run = familiar_ai_rules::outcome::card_run(&View::new(&log), "c1", "run_unclassified").and_then(|t| t.fields.get("run_id").cloned());
    assert_eq!(card_run, Some(json!(second)), "the card is the newer run's");
    assert!(offered(&log, "its_the_red").is_empty(), "never bound to the answered run: {:?}", offered(&log, "its_the_red"));
}
