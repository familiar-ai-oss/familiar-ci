//! The teeth run of a `new_work_no_spot` puts its `integration_point`
//! (`<file>[:<symbol>]`) back, and a new-work spot's teeth run puts its file
//! back, so greenfield work does not skip the teeth.
//! See docs/decisions.md#new-work-names-its-integration-point

use familiar_ai_facts::Log;
use familiar_ai_rules::sample::{apply_move, criterion_fixture, Move};
use familiar_ai_rules::{action, lifecycle, moves, plan, terminal, what_to_read, Anchor, View};
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";

fn step(log: &Log, label: &str, outcome: Option<&str>) -> Log {
    let v = View::new(log);
    let m: Option<Move> = moves::criterion_admissible(&v).into_iter().find(|m| {
        m.label.as_deref() == Some(label) && outcome.is_none_or(|o| m.fields.get("outcome").and_then(Value::as_str) == Some(o))
    });
    match m {
        Some(m) => apply_move(log, &m, AT, TENANT).unwrap_or_default(),
        None => panic!("no {label} move at:\n{}", terminal::status(&v)),
    }
}

fn raw(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { unreachable!() };
    familiar_ai_facts::append_raw(log, kind, m, AT, TENANT).expect("the door takes it")
}

fn intent_sha(log: &Log) -> String {
    View::new(log).last("intent_submitted").and_then(|f| f.fields.get("sha")).and_then(Value::as_str).unwrap_or("").to_string()
}

fn whole_file(c: &str, file_path: &str, sha: &str) -> Anchor {
    Anchor { criterion_id: c.into(), file_path: file_path.into(), line_start: 0, line_end: 0, sha: sha.into() }
}

#[test]
fn what_to_read_puts_back_the_whole_file_of_a_spot_and_of_an_integration_point() {
    let gf = criterion_fixture(&["c2"], true, true, AT, TENANT).expect("the greenfield fixture");
    assert!(what_to_read(&gf).anchors.contains(&whole_file("c2", "src/mail.py", &intent_sha(&gf))), "{:?}", what_to_read(&gf).anchors);
    let spot = criterion_fixture(&["c1"], true, false, AT, TENANT).expect("the fixture");
    let spot = raw(&spot, "new_work_spot_chosen", json!({ "criterion_id": "c1", "file_path": "src/login.py", "symbol": "check_password_strict", "beside": "check_password" }));
    assert_eq!(what_to_read(&spot).anchors, [whole_file("c1", "src/login.py", &intent_sha(&spot))]);
}

#[test]
fn the_integration_file_is_the_text_before_the_symbol() {
    let mut log: Log = Vec::new();
    for ip in ["src/app.py:main", "src/app.rs:App::run", "src/store.py"] {
        log = raw(&log, "new_work_no_spot", json!({ "criterion_id": "c9", "integration_point": ip }));
    }
    let files: Vec<Vec<String>> = log.iter().map(plan::file_paths_of).collect();
    assert_eq!(files, [vec!["src/app.py".to_string()], vec!["src/app.rs".to_string()], vec!["src/store.py".to_string()]]);
}

#[test]
fn new_work_needs_a_failed_teeth_run_to_be_proven() {
    let mut log = criterion_fixture(&["c2"], true, true, AT, TENANT).expect("the greenfield fixture");
    for (label, outcome) in [("tests_only", None), ("test_written", None), ("pass_please_pass", Some("failed")), ("fingerprint_stamped", None), ("code", None), ("pass_please_pass", Some("passed")), ("fingerprint_stamped", None)] {
        log = step(&log, label, outcome);
    }
    assert_eq!(lifecycle::state(&View::new(&log), "c2"), "green");
    for (label, outcome) in [("pipeline_run_seen", None), ("pass_please_pass", Some("failed")), ("fingerprint_stamped", None), ("pass_please_pass", Some("passed")), ("fingerprint_stamped", None)] {
        log = step(&log, label, outcome);
    }
    let v = View::new(&log);
    assert!(!action::teeth(&v, "c2"), "no exemption: the teeth run is owed");
    assert!(!action::proven(&v, "c2"));
    assert!(!lifecycle::proven_conditions(&v, "c2"));
    assert!(moves::criterion_admissible(&v).iter().any(|m| m.fields.get("with_anchor_reverted") == Some(&json!(true))), "the teeth run is the next move");
    let failed = step(&log, "pass_please_pass", Some("failed"));
    assert!(lifecycle::proven_conditions(&View::new(&failed), "c2"));
    assert!(action::proven(&View::new(&failed), "c2"));
    let passed = step(&log, "pass_please_pass", Some("passed"));
    assert!(action::no_teeth(&View::new(&passed), "c2"));
    assert!(!lifecycle::proven_conditions(&View::new(&passed), "c2"));
}
