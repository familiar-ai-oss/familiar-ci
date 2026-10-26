//! A criterion held on `two_failures` has a second way out besides the
//! person's card: an investigator, a fresh subagent one rung up the ladder
//! haiku → sonnet → opus → fable → a second fable, who hands back at green.

use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::sample::{apply_move, criterion_fixture, Move};
use familiar_ai_rules::{ladder, lifecycle, moves, terminal, Event, View};
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";

fn fact(log: &Log, kind: &str, fields: Value) -> Log {
    append_raw(log, kind, fields.as_object().cloned().unwrap_or_default(), AT, TENANT).unwrap_or_default()
}

/// The first admissible move with that label (and that outcome), applied.
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

/// One commit run through by the binary: its test with `outcome`, the
/// touching test passed, the stamp.
fn commit(log: &Log, label: &str, outcome: &str) -> Log {
    let mut log = step(log, label, None);
    if label == "tests_only" {
        log = step(&log, "test_written", None);
    }
    log = step(&log, "pass_please_pass", Some(outcome));
    log = step(&log, "pass_please_pass", Some("passed"));
    step(&log, "fingerprint_stamped", None)
}

/// c1 held on two_failures, its holder `a1` on `model` (none: unknown).
fn held_twice(model: Option<&str>) -> Log {
    let mut log = criterion_fixture(&["c1", "c2"], true, false, AT, TENANT).unwrap_or_default();
    if let Some(m) = model {
        log = fact(&log, "model_seen", json!({ "agent_conversation_id": "a1", "model": m }));
    }
    log = step(&log, "start_check", None);
    log = commit(&log, "tests_only", "failed");
    log = commit(&log, "code", "failed");
    commit(&log, "code", "failed")
}

fn two_failures_clears(log: &Log) -> Vec<Value> {
    let v = View::new(log);
    let hs = lifecycle::holds(&v, "c1");
    let h = hs.iter().find(|h| h.why == "two_failures").unwrap_or_else(|| panic!("not held: {}", terminal::status(&v)));
    h.clears.iter().cloned().map(Value::Object).collect()
}

#[test]
fn the_rung_is_read_by_family_word_and_a_subagent_shares_the_session() {
    assert_eq!(ladder::family_of(Some("claude-haiku-4-5-20251001")), Some("haiku"));
    assert_eq!(ladder::family_of(Some("claude-sonnet-5-5")), Some("sonnet"));
    assert_eq!(ladder::family_of(Some("Claude-OPUS-5")), Some("opus"));
    assert_eq!(ladder::family_of(Some("fable")), Some("fable"));
    assert_eq!(ladder::family_of(Some("gpt-5")), None);
    assert_eq!(ladder::family_of(None), None);
    assert_eq!(ladder::session_of(Some("s1/a9")), Some("s1"));
    assert!(ladder::subagent_of(Some("s1/a9"), Some("s1")));
    assert!(ladder::subagent_of(Some("s1/a9"), Some("s1/a2")), "a subagent's own subagent is session/agent too");
    assert!(!ladder::subagent_of(Some("s2/a9"), Some("s1")));
    assert!(!ladder::subagent_of(Some("a2"), Some("a1")));
}

#[test]
fn a_haiku_holder_held_twice_is_offered_a_sonnet_investigator_beside_the_card() {
    let log = held_twice(Some("claude-haiku-4-5-20251001"));
    let clears = two_failures_clears(&log);
    assert_eq!(clears.len(), 5, "{clears:?}");
    assert_eq!(clears[4], json!({ "kind": "criterion_transfer", "criterion_id": "c1", "from_conversation": "a1", "rung": "sonnet" }));
    let v = View::new(&log);
    assert_eq!(ladder::investigator_rung(&v, "c1"), Some("sonnet"));
    let txt = terminal::refusal(&v, &Event::Stops { main: true }).unwrap_or_default();
    assert!(
        txt.ends_with("  to clear it: the person answers its card: try_again, take_over, a_dependency or release (hold_answered); or an investigator one rung up takes it: a fresh sonnet subagent (criterion_transfer)"),
        "{txt}"
    );
}

#[test]
fn the_investigator_takes_it_restarts_the_attempts_and_hands_it_back_when_green() {
    let log = held_twice(Some("claude-haiku-4-5-20251001"));
    let log = step(&log, "investigator", None);
    let v = View::new(&log);
    let tail: Vec<(&str, Value)> = log[log.len() - 2..].iter().map(|f| (f.kind, Value::Object(f.fields.clone()))).collect();
    assert_eq!(tail[0], ("model_seen", json!({ "agent_conversation_id": "a1/i1", "model": "sonnet" })));
    assert_eq!(tail[1], ("criterion_transfer", json!({ "criterion_id": "c1", "from_conversation": "a1", "to_conversation": "a1/i1" })));
    assert_eq!(ladder::chain_of(&v, "c1").len(), 1);
    assert_eq!(lifecycle::attempts(&v, "c1"), 0, "the investigator starts its own two");
    assert_eq!(lifecycle::state(&v, "c1"), "sealed");
    let lateral = |m: &Move| m.label.as_deref() == Some("criterion_transfer") && m.fields.get("criterion_id") == Some(&json!("c1"));
    assert!(!moves::criterion_admissible(&v).iter().any(lateral), "no lateral transfer of c1 while its investigator is open");
    // the investigator's first act is its own claim
    assert!(lifecycle::first_act(&v, "c1"));
    let log = step(&log, "fix_location_category", None);
    let log = commit(&log, "code", "passed");
    let v = View::new(&log);
    assert_eq!(lifecycle::state(&v, "c1"), "green");
    let log = step(&log, "start_check", None); // c2's turn: the referee's check comes first
    let log = step(&log, "transfer_back", None);
    let v = View::new(&log);
    assert_eq!(familiar_ai_rules::plan::assigned_to(&v, "c1"), Some("a1"));
    assert!(ladder::chain_of(&v, "c1").is_empty());
    // after the hand-back the holder's earlier claim stands again; nothing is appended
    assert!(!lifecycle::first_act(&v, "c1"));
    assert_eq!(lifecycle::standing_claim(&v, "c1").map(|f| f.t), Some(log.iter().position(|f| f.kind == "fix_location_category").unwrap_or(0) as u64));
}

#[test]
fn a_rung_not_one_up_is_a_plain_transfer_that_clears_nothing() {
    let log = held_twice(Some("claude-haiku-4-5-20251001"));
    let log = step(&log, "not_one_rung_up", None);
    let v = View::new(&log);
    assert!(ladder::chain_of(&v, "c1").is_empty());
    assert!(lifecycle::holds(&v, "c1").iter().any(|h| h.why == "two_failures"));
}

#[test]
fn model_unknown_sends_fable_and_off_the_ladder_only_the_card_naming_the_model() {
    let unknown = held_twice(None);
    assert_eq!(ladder::investigator_rung(&View::new(&unknown), "c1"), Some("fable"));
    let off = held_twice(Some("gpt-5"));
    let v = View::new(&off);
    assert_eq!(ladder::investigator_rung(&v, "c1"), None);
    assert_eq!(two_failures_clears(&off).len(), 4, "the card alone");
    let card = terminal::cards(&v).into_iter().find(|k| k.hold == "two_failures").map(|k| k.question).unwrap_or_default();
    assert_eq!(card, "criterion c1: two attempts in a row ended with its tests failing; it ran on gpt-5, a model off the ladder, so no investigator is sent. What now?");
}

#[test]
fn a_fable_holder_gets_a_second_fable_once_then_only_the_card() {
    let log = held_twice(Some("claude-fable-5"));
    assert_eq!(ladder::investigator_rung(&View::new(&log), "c1"), Some("fable"));
    let log = step(&log, "investigator", None);
    let log = step(&log, "fix_location_category", None);
    let log = commit(&log, "code", "failed");
    let log = commit(&log, "code", "failed");
    let v = View::new(&log);
    assert!(lifecycle::holds(&v, "c1").iter().any(|h| h.why == "two_failures"));
    assert_eq!(ladder::investigator_rung(&v, "c1"), None, "two fables in the chain: the card");
    assert_eq!(two_failures_clears(&log).len(), 4);
}
