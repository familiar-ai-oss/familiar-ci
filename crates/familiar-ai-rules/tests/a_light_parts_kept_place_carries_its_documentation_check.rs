//! A light part answered keep_going on anchor_untouched: the binary's kept
//! place is followed by its documentation check of every spot (door/anchor.rs
//! `binary_checks` → `documentation::write_one`), as the re-anchor carries it.
//! Without it the part is unplanned with no admissible move (found by the
//! checks crate's W3; the owner approved the fix 2026-10-18).

use familiar_ai_facts::Log;
use familiar_ai_rules::sample::{apply_move, criterion_fixture};
use familiar_ai_rules::{lifecycle, moves, terminal, View};
use serde_json::Value;

const AT: f64 = 0.0;
const TENANT: &str = "_default_";

fn step(log: &Log, label: &str, outcome: Option<&str>, choice: Option<&str>) -> Log {
    let v = View::new(log);
    let m = moves::criterion_admissible(&v).into_iter().find(|m| {
        m.label.as_deref() == Some(label)
            && outcome.is_none_or(|o| m.fields.get("outcome").and_then(Value::as_str) == Some(o))
            && choice.is_none_or(|c| m.fields.get("choice").and_then(Value::as_str) == Some(c))
    });
    match m {
        Some(m) => apply_move(log, &m, AT, TENANT).expect("lands"),
        None => panic!("no {label} move among {:?} at:\n{}", moves::criterion_admissible(&v).iter().map(|m| (m.label.clone(), m.kind.clone())).collect::<Vec<_>>(), terminal::status(&v)),
    }
}

/// tests_only → test_written → pass:passed → stamp → code → re_plan → re_anchor
/// → start_check → changed_test → pass:failed → stamp → code_outside_anchor →
/// pass:passed → stamp → keep_going → landed_place, all of c2 (light).
fn kept() -> Log {
    let mut log = criterion_fixture(&["c2"], true, false, AT, TENANT).expect("fixture");
    for (label, outcome, choice) in [
        ("start_check", None, None),
        ("tests_only", None, None),
        ("test_written", None, None),
        ("pass_please_pass", Some("passed"), None),
        ("fingerprint_stamped", None, None),
        ("code", None, None),
        ("hold_answered", None, Some("re_plan")),
        ("re_anchor", None, None),
        ("start_check", None, None),
        ("changed_test", None, None),
        ("pass_please_pass", Some("failed"), None),
        ("fingerprint_stamped", None, None),
        ("code_outside_anchor", None, None),
        ("pass_please_pass", Some("passed"), None),
        ("fingerprint_stamped", None, None),
        ("hold_answered", None, Some("keep_going")),
        ("landed_place", None, None),
    ] {
        log = step(&log, label, outcome, choice);
    }
    log
}

#[test]
fn after_the_kept_place_a_light_part_is_offered_its_documentation_check_both_ways() {
    let log = kept();
    let v = View::new(&log);
    let docs: Vec<_> = moves::binary_moves(&v).into_iter().filter(|m| m.kind == "anchor_is_documentation").collect();
    for want in [true, false] {
        assert!(
            docs.iter().any(|m| m.fields.get("matched").and_then(Value::as_bool) == Some(want)),
            "the binary's check with matched {want}: {docs:?}\n{}",
            terminal::status(&v)
        );
    }
    for m in &docs {
        let after = apply_move(&log, m, AT, TENANT).expect("lands");
        assert!(!moves::criterion_admissible(&View::new(&after)).is_empty(), "not stuck after {m:?}");
    }
}

#[test]
fn after_the_kept_place_a_light_part_is_not_stuck() {
    let log = kept();
    let v = View::new(&log);
    assert!(
        !moves::criterion_admissible(&v).is_empty(),
        "{}: {:?}\n{}",
        lifecycle::state(&v, "c2"),
        lifecycle::holds(&v, "c2"),
        terminal::status(&v)
    );
}
