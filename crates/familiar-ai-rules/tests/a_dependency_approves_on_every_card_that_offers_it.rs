//! S9a's two (the owner, 2026-10-16): (1) a waiting part's plain file edits are refused as its
//! commits are (stop 3 joins `wait_refusal`; Assumption 24 in full); (2) `a_dependency` counts as
//! the approval on `crashed`, `two_<outcome>` and `command_broke` alike, and Decided B puts it on
//! `two_failures` and the `two_<outcome>` cards: the binary drafts after any of them and the
//! re-earn rule keys on any of them.
#![allow(dead_code)]

use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::criterion::answers_of;
use familiar_ai_rules::dependency::{approvals, lines_owed, re_earn, restring};
use familiar_ai_rules::hooks::{decide, Decision, Event};
use familiar_ai_rules::sample::criterion_fixture;
use familiar_ai_rules::{criterion, lifecycle, moves, plan, View};
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";
const TOUCHING: &str = "tests/test_login_old.py";

fn raw(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { unreachable!() };
    append_raw(log, kind, m, AT, TENANT).unwrap_or_else(|e| panic!("append {kind}: {e}"))
}

fn test_of(c: &str) -> String {
    format!("tests/test_{c}.py")
}

fn place_of(c: &str) -> String {
    if c == "c1" { "src/login.py".to_string() } else { format!("src/dep_{c}.py") }
}

/// Each part's fingerprint never changes in these stories.
fn hash_of(c: &str) -> String {
    if c == "c1" { "S1".to_string() } else { format!("H{c}") }
}

/// A commit, observed for every part whose tests exist at it, as the binary writes it.
fn commit(log: &Log, id: &str, files: &[String], tests: bool, code: bool, overlap: &[&str]) -> Log {
    let mut log = raw(log, "commit_seen", json!({ "commit": id, "files": files, "touches_tests": tests, "touches_code": code, "anchor_overlap": overlap }));
    for c in plan::criteria(&View::new(&log)) {
        let seen = log.iter().any(|f| f.kind == "tests_observed" && f.criterion_id() == Some(c.as_str()));
        if seen || files.contains(&test_of(&c)) {
            log = raw(&log, "tests_observed", json!({ "criterion_id": c, "criterion_rev": "R", "commit": id, "tests_hash": hash_of(&c) }));
        }
    }
    log
}

fn tests_commit(log: &Log, id: &str, c: &str) -> Log {
    let log = commit(log, id, &[test_of(c)], true, false, &[]);
    if log.iter().any(|f| f.kind == "test_written" && f.criterion_id() == Some(c)) {
        return log;
    }
    raw(&log, "test_written", json!({ "criterion_id": c, "file_path": test_of(c), "test_name": "t1" }))
}

fn code_commit(log: &Log, id: &str, c: &str) -> Log {
    commit(log, id, &[place_of(c)], false, true, &[c])
}

/// B: a commit that touches no tests, no code and no anchor.
fn empty_commit(log: &Log, id: &str) -> Log {
    commit(log, id, &["README.md".to_string()], false, false, &[])
}

/// A run of `c` at `id`: reserved, ended, translated as `label`, the own test's result (and c1's
/// touching test, passed), naming the run; the stamp is `stamp`'s.
fn results(log: &Log, c: &str, id: &str, label: &str) -> Log {
    let word = if label == "passed" { "passed" } else { "failed" };
    let rid = format!("r-{c}-{id}-{}", log.len());
    let mut log = raw(log, "run_reserved", json!({ "run_id": rid, "criterion_id": c, "criterion_rev": "R", "commit": id, "command": "python -m pytest", "resources": ["tests"], "wait": "60", "by": "binary" }));
    log = raw(&log, "run_ended", json!({ "run_id": rid, "outcome": word, "end": "exited" }));
    log = raw(&log, "test_log_translation", json!({ "run_id": rid, "criterion_id": c, "commit": id, "by": "regex", "label": label, "evidence": "", "failing_tests": [], "classified_on": "full" }));
    log = raw(&log, "pass_please_pass", json!({ "criterion_id": c, "file_path": test_of(c), "commit": id, "outcome": word, "ran_in": "laptop", "with_anchor_reverted": false, "run_id": rid }));
    if c == "c1" {
        log = raw(&log, "pass_please_pass", json!({ "criterion_id": c, "file_path": TOUCHING, "commit": id, "outcome": "passed", "ran_in": "laptop", "with_anchor_reverted": false, "run_id": rid }));
    }
    log
}

fn stamp(log: &Log, c: &str, id: &str) -> Log {
    raw(log, "fingerprint_stamped", json!({ "criterion_id": c, "commit": id, "proof_not_tampered": hash_of(c), "ran_in": "laptop" }))
}

fn run(log: &Log, c: &str, id: &str, label: &str) -> Log {
    stamp(&results(log, c, id, label), c, id)
}

fn start_check(log: &Log, c: &str) -> Log {
    let (symbol, from, to) = if c == "c1" { ("check_password".to_string(), 40, 62) } else { (format!("fn_{c}"), 1, 9) };
    raw(log, "anchor_sha_matched", json!({ "criterion_id": c, "file_path": place_of(c), "symbol": symbol, "line_start": from, "line_end": to, "sha": "base", "lines_hash": "L0", "matched": true, "checked_at": "start" }))
}

/// The person's `a_dependency` on `c`'s crashed card; its `t` is the approval.
fn approve(log: &Log, c: &str) -> (Log, u64) {
    let log = raw(log, "hold_answered", json!({ "criterion_id": c, "hold": "crashed", "choice": "a_dependency", "typed": "" }));
    let t = log.last().map(|f| f.t).unwrap_or(0);
    (log, t)
}

/// The draft and the dependency's lines in one batch, as S9c's picker writes them: `d` strung
/// just before `x`.
fn draft(log: &Log, approval: u64, d: &str, x: &str) -> Log {
    let mut log = raw(log, "dependency_drafted", json!({ "approval": approval, "criterion_id": d, "text": format!("{d}: what {x} needs"), "test_file": test_of(d) }));
    log = raw(&log, "criterion", json!({ "criterion_id": d, "text": format!("{d}: what {x} needs"), "path": "full", "behavior_id": "b1", "promise_id": "p1" }));
    let place = json!({ "file_path": place_of(d), "symbol": format!("fn_{d}"), "line_start": 1, "line_end": 9 });
    log = raw(&log, "place_chosen", json!({ "criterion_id": d, "places": [place] }));
    log = raw(&log, "anchor_sha_matched", json!({ "criterion_id": d, "file_path": place_of(d), "symbol": format!("fn_{d}"), "line_start": 1, "line_end": 9, "sha": "base", "lines_hash": "L0", "matched": true, "checked_at": "planning" }));
    log = raw(&log, "test_planned", json!({ "criterion_id": d, "file_path": test_of(d), "will": "create" }));
    for (kind, fields) in restring(&View::new(&log), d, x) {
        log = raw(&log, kind, Value::Object(fields));
    }
    log = raw(&log, "criterion_assigned", json!({ "criterion_id": d, "agent_conversation_id": "a1" }));
    raw(&log, "fix_location_category", json!({ "criterion_id": d, "category": "this_repository", "where": "" }))
}

/// c1 started, its test committed alone at k1, and the run there crashed (t1).
fn crashed() -> Log {
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).expect("fixture");
    let log = start_check(&log, "c1");
    let log = tests_commit(&log, "k1", "c1");
    run(&log, "c1", "k1", "crashed")
}

/// After the crash: the approval, the draft of c3 before c1, c3 started, its test red at k2.
fn drafted() -> Log {
    let (log, t2) = approve(&crashed(), "c1");
    let log = draft(&log, t2, "c3", "c1");
    let log = start_check(&log, "c3");
    let log = tests_commit(&log, "k2", "c3");
    run(&log, "c3", "k2", "tests_failed")
}

fn v(log: &Log) -> View<'_> {
    View::new(log)
}

fn tests_first_ids(log: &Log, c: &str) -> Vec<String> {
    criterion::tests_first_commit(&v(log), c).map(|t| t.ids).unwrap_or_default()
}

fn owed(log: &Log, c: &str) -> Vec<String> {
    moves::owed(&v(log), c).into_iter().map(|w| w.commit).collect()
}

fn re_earn_at(log: &Log, c: &str) -> Option<String> {
    re_earn(&v(log), c).map(|r| r.commit)
}

fn seal_commit(log: &Log, c: &str) -> Option<String> {
    criterion::seal_raw(&v(log), c, criterion::Place::Laptop).and_then(|s| s.fields.get("commit")).and_then(Value::as_str).map(str::to_string)
}

fn state(log: &Log, c: &str) -> &'static str {
    lifecycle::state(&v(log), c)
}

/// The person's `a_dependency` on `c`'s `hold` card.
fn approve_on(log: &Log, c: &str, hold: &str) -> (Log, u64) {
    let log = raw(log, "hold_answered", json!({ "criterion_id": c, "hold": hold, "choice": "a_dependency", "typed": "" }));
    let t = log.last().map(|f| f.t).unwrap_or(0);
    (log, t)
}

/// c1 started, its test committed alone at k1 and red there: sealed.
fn sealed() -> Log {
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).expect("fixture");
    let log = start_check(&log, "c1");
    let log = tests_commit(&log, "k1", "c1");
    run(&log, "c1", "k1", "tests_failed")
}

#[test]
fn a_dependency_is_a_choice_on_two_failures_and_every_two_outcome() {
    for hold in ["two_failures", "two_test_broken", "two_crashed", "two_timed_out", "crashed", "command_broke"] {
        assert!(answers_of(hold).is_some_and(|cs| cs.contains(&"a_dependency")), "{hold}");
    }
    for hold in ["two_tries_not_red", "environment_error", "anchor_moved"] {
        assert!(!answers_of(hold).is_some_and(|cs| cs.contains(&"a_dependency")), "{hold}");
    }
}

#[test]
fn a_dependency_counts_as_the_approval_on_every_card_that_offers_it() {
    for hold in ["crashed", "command_broke", "two_failures", "two_test_broken", "two_crashed", "two_timed_out"] {
        let (log, t2) = approve_on(&sealed(), "c1", hold);
        let v = View::new(&log);
        let got: Vec<u64> = approvals(&v, "c1").iter().map(|a| a.t).collect();
        assert_eq!(got, [t2], "{hold}");
        let m = moves::binary_moves(&v).into_iter().find(|m| m.kind == "dependency_drafted");
        assert!(m.is_some_and(|m| m.fields.get("approval") == Some(&json!(t2))), "{hold}: the binary drafts");
    }
    // a choice the card does not offer is no approval
    let (log, _) = approve_on(&sealed(), "c1", "two_tries_not_red");
    assert!(approvals(&View::new(&log), "c1").is_empty());
}

/// The re-earn rule keys on the two_failures approval as on the crashed one: c1 sealed at k1, the
/// person's a_dependency, c3 drafted before c1, red at k2 and green at k3: c1's red moves to k3.
#[test]
fn the_re_earn_keys_on_a_two_failures_approval() {
    let (log, t2) = approve_on(&sealed(), "c1", "two_failures");
    let log = draft(&log, t2, "c3", "c1");
    let log = start_check(&log, "c3");
    let log = tests_commit(&log, "k2", "c3");
    let log = run(&log, "c3", "k2", "tests_failed");
    let log = code_commit(&log, "k3", "c3");
    let log = run(&log, "c3", "k3", "passed");
    assert_eq!(re_earn_at(&log, "c1").as_deref(), Some("k3"));
    assert_eq!(tests_first_ids(&log, "c1"), ["k3"]);
}

/// Assumption 24 in full: once the batch lands, the waiting part's edits are refused as its
/// commits are, its test file and its place alike, and the dependency's own edits are allowed.
#[test]
fn a_waiting_parts_edits_are_refused_as_its_commits_are() {
    let (log, t2) = approve(&crashed(), "c1");
    let log = draft(&log, t2, "c3", "c1");
    let v = v(&log);
    assert_eq!(lifecycle::state(&v, "c1"), "waiting");
    for file in ["src/login.py", "tests/test_c1.py"] {
        match decide(&v, &Event::FileEdit { criterion_id: Some("c1".into()), file_path: file.into() }) {
            Decision::Refuse(r) => {
                assert_eq!(r.state, Some("waiting"), "{file}");
                assert_eq!(r.criterion_id.as_deref(), Some("c1"), "{file}");
            }
            d => panic!("{file}: a waiting part's edit is refused, got {d:?}"),
        }
    }
    let ok = decide(&v, &Event::FileEdit { criterion_id: Some("c3".into()), file_path: place_of("c3") });
    assert!(matches!(ok, Decision::Allow { .. }), "{ok:?}");
}

/// The part an edit is for: the conversation's lane, else a waiting part it holds.
#[test]
fn an_edit_is_for_the_waiting_part_its_conversation_holds_when_it_holds_no_other() {
    let (log, t2) = approve(&crashed(), "c1");
    let log = draft(&log, t2, "c3", "c1");
    let log = raw(&log, "criterion_assigned", json!({ "criterion_id": "c1", "agent_conversation_id": "s/w" }));
    let v = v(&log);
    assert_eq!(lifecycle::lane(&v, Some("s/w")), None);
    assert_eq!(lifecycle::edit_lane(&v, Some("s/w")).as_deref(), Some("c1"));
    assert_eq!(lifecycle::edit_lane(&v, Some("a1")).as_deref(), Some("c3"), "the lane first");
    assert_eq!(lifecycle::edit_lane(&v, Some("nobody")), None);
}

/// Review 1 (S9c): an edit of a waiting part's place or test is refused whichever part the
/// edit is made for; the dependency's own place is not.
#[test]
fn an_edit_of_a_waiting_parts_place_is_refused_whichever_part_it_is_for() {
    let (log, t2) = approve(&crashed(), "c1");
    let log = draft(&log, t2, "c3", "c1");
    let v = v(&log);
    for (for_, file) in [(Some("c3"), "src/login.py"), (None, "src/login.py"), (Some("c3"), "tests/test_c1.py")] {
        match decide(&v, &Event::FileEdit { criterion_id: for_.map(str::to_string), file_path: file.into() }) {
            Decision::Refuse(r) => {
                assert_eq!(r.state, Some("waiting"), "{file}");
                assert_eq!(r.criterion_id.as_deref(), Some("c1"), "{file}");
            }
            d => panic!("{for_:?} {file}: refused, got {d:?}"),
        }
    }
    let ok = decide(&v, &Event::FileEdit { criterion_id: Some("c3".into()), file_path: place_of("c3") });
    assert!(matches!(ok, Decision::Allow { .. }), "{ok:?}");
}

/// Review 4 (S9c): after a refused plan, the lines are owed again only once the person answered
/// the environment_error card try_again; re_plan and release owe nothing.
#[test]
fn a_refused_plans_lines_are_owed_again_only_after_try_again() {
    let (log, t2) = approve(&crashed(), "c1");
    let log = raw(&log, "dependency_drafted", json!({ "approval": t2, "criterion_id": "c3", "text": "c3", "test_file": "tests/test_c3.py" }));
    let a = |log: &Log| log.iter().find(|f| f.t == t2).cloned().expect("approval");
    assert!(lines_owed(&v(&log), &a(&log)), "a draft with no lines and no miss: owed (the crash between 5 and 6)");
    let log = raw(&log, "dependency_unanswered", json!({ "approval": t2, "why": "the plan with the dependency is incomplete: line_5" }));
    assert!(!lines_owed(&v(&log), &a(&log)), "the card stands");
    for (choice, owed) in [("re_plan", false), ("release", false), ("try_again", true)] {
        let l = raw(&log, "hold_answered", json!({ "criterion_id": "c1", "hold": "environment_error", "choice": choice, "typed": "" }));
        assert_eq!(lines_owed(&v(&l), &a(&l)), owed, "{choice}");
    }
}
