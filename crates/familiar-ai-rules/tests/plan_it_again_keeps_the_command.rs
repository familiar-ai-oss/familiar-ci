//! "plan it again keeps the command of the place it is leaving until the new
//! place is chosen. Answering anchor_moved with re_plan unchooses the place for
//! planning, the checks and the holds (plan::anchor_of stays as it is); it does
//! not take away the part's test command." (the owner, 2026-10-12)
//!
//! c1 is scoped under the stack root `src/`, held `anchor_moved`, answered
//! `re_plan`: its command and its scope stand as they were. After the new
//! `place_chosen` under another root they read from the new place.

use familiar_ai_facts::Log;
use familiar_ai_rules::sample::{apply_move, criterion_fixture, Move};
use familiar_ai_rules::{criterion, lifecycle, moves, plan, terminal, View};
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";

fn step(log: &Log, label: &str, choice: Option<&str>) -> Log {
    let v = View::new(log);
    let m: Option<Move> = moves::criterion_admissible(&v).into_iter().find(|m| {
        m.label.as_deref() == Some(label) && choice.is_none_or(|c| m.fields.get("choice").and_then(Value::as_str) == Some(c))
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

/// c1 scoped under `src/`, its start check saw its lines moved: the card is up.
fn moved() -> Log {
    let log = criterion_fixture(&["c1", "c2"], true, false, AT, TENANT).unwrap_or_default();
    let log = step(&log, "outside_edit_seen", None);
    let v = View::new(&log);
    assert!(lifecycle::whys(&v, "c1").iter().any(|w| w == "anchor_moved"));
    assert!(criterion::own_command(&v, "c1").is_none(), "c1's command is derived, not claimed");
    assert!(criterion::scoped(&v, "c1"), "c1 is scoped under the stack root");
    log
}

#[test]
fn plan_it_again_keeps_the_command_until_the_new_place_is_chosen() {
    let before = moved();
    let (cmd, scoped) = {
        let v = View::new(&before);
        (criterion::command(&v, "c1"), criterion::scoped(&v, "c1"))
    };
    assert!(cmd.is_some());

    let log = step(&before, "hold_answered", Some("re_plan"));
    let v = View::new(&log);
    assert!(plan::anchor_of(&v, "c1").is_none(), "the place is unchosen for planning");
    assert_eq!(criterion::command(&v, "c1"), cmd, "plan it again keeps the command");
    assert_eq!(criterion::scoped(&v, "c1"), scoped, "plan it again keeps the scope");

    // another root with its own command; c1's old place is not under it
    let log = raw(&log, "test_stack_detected", json!({ "codebase_id": "cb1", "project_root": "lib/", "test_stack": "python_pytest", "test_command": "pytest -q" }));
    assert_eq!(criterion::command(&View::new(&log), "c1"), cmd, "still the place it is leaving");

    let log = raw(
        &log,
        "place_chosen",
        json!({ "criterion_id": "c1", "places": [{ "file_path": "lib/auth.py", "symbol": "check", "line_start": 1, "line_end": 9 }] }),
    );
    let v = View::new(&log);
    assert!(plan::anchor_of(&v, "c1").is_some(), "the new place is chosen");
    assert_eq!(criterion::command(&v, "c1").as_deref(), Some("pytest -q tests/test_c1.py"), "read from the new place");
    assert!(criterion::scoped(&v, "c1"));
}
