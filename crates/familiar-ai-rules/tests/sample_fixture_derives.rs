//! The rules over the sample fixture with planning confirmed: the states it
//! derives, the hooks' decisions, what the commit reader is told and the
//! terminal's words.

use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::sample::{apply_move, criterion_fixture, Move, INTENT_SHA};
use familiar_ai_rules::{action, criterion, decide, hooks, lifecycle, moves, plan, terminal, what_to_read, Clears, Decision, Event, View};
use serde_json::json;

const AT: f64 = 0.0;
const TENANT: &str = "_default_";

fn fixture() -> Log {
    criterion_fixture(&["c1", "c2"], true, false, AT, TENANT).unwrap_or_default()
}

/// Every admissible move of the criterion's world applied until the label
/// names the move, choosing the first match; `None` when no such move.
fn step(log: &Log, label: &str, outcome: Option<&str>) -> Option<Log> {
    let v = View::new(log);
    let m: Move = moves::criterion_admissible(&v)
        .into_iter()
        .find(|m| m.label.as_deref() == Some(label) && outcome.is_none_or(|o| m.fields.get("outcome").and_then(|x| x.as_str()) == Some(o)))?;
    apply_move(log, &m, AT, TENANT).ok()
}

#[test]
fn the_fixture_is_confirmed_and_the_string_waits_on_its_head() {
    let log = fixture();
    let v = View::new(&log);
    assert_eq!(plan::plan_state(&v), "confirmed");
    assert!(plan::PLAN_LINES.iter().all(|n| plan::line(&v, n) == Some(true)));
    assert_eq!(lifecycle::state(&v, "c1"), "testing");
    assert_eq!(lifecycle::state(&v, "c2"), "waiting");
    assert_eq!(plan::string_before(&v, "c2").as_deref(), Some("c1"));
    assert_eq!(lifecycle::runnable_criteria(&v), ["c1"]);
    assert_eq!(action::check_state(&v), "not_run");
}

#[test]
fn before_planning_every_event_is_held_on_planning_incomplete_naming_the_first_missing_line() {
    let log: Log = Vec::new();
    match decide(&log, &Event::Stops { main: true }, AT) {
        Decision::Refuse(r) => {
            assert_eq!(r.hold.as_deref(), Some("planning_incomplete"));
            // §3 line 1 is the first missing: an intent_submitted supplies it
            assert_eq!(r.clears, Clears::Moves(vec![json!({ "kind": "intent_submitted" }).as_object().cloned().unwrap_or_default()]));
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_tests_commit_is_the_turn_of_the_head_only_and_must_be_tests_only() {
    let log = fixture();
    let on = |c: &str, tc: bool| Event::TestsCommit { criterion_id: c.into(), touches_code: tc };
    assert!(decide(&log, &on("c1", false), AT).allowed());
    match decide(&log, &on("c1", true), AT) {
        Decision::Refuse(r) => assert_eq!(r.hold.as_deref(), Some("tests_and_code_mixed")),
        other => panic!("expected tests_and_code_mixed, got {other:?}"),
    }
    match decide(&log, &on("c2", false), AT) {
        Decision::Refuse(r) => {
            assert_eq!(r.hold, None);
            assert_eq!(r.state, Some("waiting"));
            assert_eq!(r.criterion_id.as_deref(), Some("c2"));
        }
        other => panic!("expected a not-yet waiting refusal, got {other:?}"),
    }
}

#[test]
fn the_tests_first_commit_seen_red_and_stamped_seals_the_criterion() {
    let mut log = fixture();
    let v = View::new(&log);
    let m = moves::criterion_admissible(&v).into_iter().find(|m| m.label.as_deref() == Some("start_check"));
    log = apply_move(&log, &m.unwrap_or_else(|| Move::plain("intent_submitted", json!({}))), AT, TENANT).unwrap_or_default();
    log = step(&log, "tests_only", None).unwrap_or_default();
    log = step(&log, "test_written", None).unwrap_or_default();
    log = step(&log, "pass_please_pass", Some("failed")).unwrap_or_default(); // the test
    log = step(&log, "pass_please_pass", Some("passed")).unwrap_or_default(); // the touching test
    log = step(&log, "fingerprint_stamped", None).unwrap_or_default();
    let v = View::new(&log);
    assert!(criterion::sealed_at(&v, "c1", criterion::Place::Laptop).is_some(), "{}", terminal::status(&v));
    assert_eq!(lifecycle::state(&v, "c1"), "sealed");
    // an edit of the sealed test is refused until announced
    let edit = Event::FileEdit { criterion_id: Some("c1".into()), file_path: "tests/test_c1.py".into() };
    match decide(&log, &edit, AT) {
        Decision::Refuse(r) => assert_eq!(r.hold.as_deref(), Some("proof_tampered")),
        other => panic!("expected proof_tampered, got {other:?}"),
    }
    let announced = append_raw(&log, "test_change_announced", json!({ "criterion_id": "c1", "reason": "x" }).as_object().cloned().unwrap_or_default(), AT, TENANT).unwrap_or_default();
    assert!(decide(&announced, &edit, AT).allowed());
}

#[test]
fn what_to_read_names_the_plan_tests_the_active_tests_and_the_anchors() {
    let log = fixture();
    let w = what_to_read(&log);
    assert_eq!(w.plan_test_files, vec!["tests/test_c1.py", "tests/test_c2.py"]);
    assert_eq!(w.runnable_tests, vec![("c1".to_string(), vec!["tests/test_c1.py".to_string()])]);
    assert_eq!(w.anchors.len(), 2);
    assert_eq!(w.anchors[0].criterion_id, "c1");
    assert_eq!(w.anchors[0].file_path, "src/login.py");
    assert_eq!((w.anchors[0].line_start, w.anchors[0].line_end), (40, 62));
    assert_eq!(w.anchors[0].sha, INTENT_SHA);
}

#[test]
fn the_terminal_reads_the_whole_picture_and_a_not_yet_refusal() {
    let log = fixture();
    let v = View::new(&log);
    assert_eq!(terminal::status(&v), "plan · confirmed\ncriterion c1 · testing · path full · agent a1\ncriterion c2 · waiting · path light · agent a1\nCI: no run for base"); // the CI line comes last
    assert!(terminal::cards(&v).is_empty());
    let txt = terminal::refusal(&v, &Event::DeclaresFinished { criterion_id: "c2".into() }).unwrap_or_default();
    assert!(txt.starts_with("not yet · criterion c2 · waiting"), "{txt}");
    assert!(txt.contains("criterion c1 must be green or proven; it is testing"), "{txt}");
    assert_eq!(terminal::lines(&View::new(&Vec::new()), &v), vec!["criterion c1 · testing", "criterion c2 · waiting"]);
}

#[test]
fn the_events_of_a_log_are_the_models_in_its_order() {
    let log = fixture();
    let v = View::new(&log);
    let evs = hooks::events(&v);
    assert_eq!(evs.first(), Some(&Event::FirstEditOrCommit));
    assert_eq!(evs.last(), Some(&Event::AboutToStart { criterion_id: "c2".into() }));
    assert_eq!(evs.iter().filter(|e| matches!(e, Event::Stops { .. })).count(), 1);
}
