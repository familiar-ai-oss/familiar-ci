//! The person's keep_going on `anchor_untouched` appends the place where the
//! change landed to the plan, and keeps the proof. The owner (2026-10-13): "at
//! keep_going where there is actually a change outside the original plan we
//! have to append an anchor to the plan and use the existing infrastructure. so
//! we update the plan, with a stamp that the user basically overrode it."
//!
//! Right after the answer the binary's `place_chosen` names the files the code
//! landed in since the seal, carrying the answer's hold and choice (the stamp);
//! it is offered only then and only as the binary's move. It does not move the
//! part's epoch: the red seal, the stamps and the answers before it still
//! count, and the part goes green with the change where it landed. An agent's
//! `place_chosen` while held is refused; re_plan is unchanged.
//! See docs/decisions.md#holds-are-the-persons

mod common;
use common::*;
use familiar_ai_facts::Log;
use familiar_ai_rules::criterion::{epoch, sealed_at, Place};
use familiar_ai_rules::sample::{apply_move, Move};
use familiar_ai_rules::{moves, plan, terminal, View};
use serde_json::{json, Value};

/// c1 sealed red; the fix lands outside its anchor (in src/login.py, off its
/// lines) and the tests pass: held on anchor_untouched.
fn untouched() -> Log {
    let log = step(&sealed(), "code_outside_anchor", None);
    let log = run(&log, "passed");
    assert!(whys(&log).contains(&"anchor_untouched".to_string()), "{:?}", whys(&log));
    log
}

fn answer(log: &Log, choice: &str) -> Log {
    let m = moves::criterion_admissible(&View::new(log)).into_iter().find(|m| {
        m.kind == "hold_answered"
            && m.fields.get("hold").and_then(Value::as_str) == Some("anchor_untouched")
            && m.fields.get("choice").and_then(Value::as_str) == Some(choice)
    });
    match m {
        Some(m) => apply_move(log, &m, AT, TENANT).unwrap_or_default(),
        None => panic!("no {choice} answer at:\n{}", terminal::status(&View::new(log))),
    }
}

/// The `place_chosen` moves offered for c1.
fn places_offered(log: &Log) -> Vec<Move> {
    moves::criterion_admissible(&View::new(log)).into_iter().filter(|m| m.kind == "place_chosen").collect()
}

/// A place_chosen carrying the person's keep_going on anchor_untouched.
fn stamped(m: &Move) -> bool {
    m.fields.get("hold").and_then(Value::as_str) == Some("anchor_untouched") && m.fields.get("choice").and_then(Value::as_str) == Some("keep_going")
}

/// The binary's moves to the end: its runs and stamps at the tip.
fn settle(mut log: Log) -> Log {
    for _ in 0..20 {
        let b = moves::binary_moves(&View::new(&log));
        let Some(m) = b.into_iter().find(|m| m.kind != "pass_please_pass" || m.fields.get("outcome").and_then(Value::as_str) == Some("passed")) else {
            return log;
        };
        log = apply_move(&log, &m, AT, TENANT).expect("the binary's move lands");
    }
    log
}

#[test]
fn while_held_no_place_carrying_the_persons_answer_is_admissible() {
    let log = untouched();
    assert!(places_offered(&log).iter().all(|m| !stamped(m)), "{:?}", places_offered(&log));
    let agents = Move {
        kind: "place_chosen".into(),
        fields: json!({ "criterion_id": "c1", "places": [{ "file_path": "src/login.py", "symbol": "", "line_start": 63, "line_end": 63 }], "hold": "anchor_untouched", "choice": "keep_going" })
            .as_object()
            .cloned()
            .unwrap_or_default(),
        more: Some(Vec::new()),
        label: Some("re_anchor".into()),
    };
    assert!(moves::criterion_append(&log, &agents, AT, TENANT).is_err(), "never place_chosen to get past a hold");
}

#[test]
fn right_after_keep_going_the_binary_names_the_place_where_the_change_landed() {
    let log = answer(&untouched(), "keep_going");
    let offered = places_offered(&log);
    assert_eq!(offered.len(), 1, "{offered:?}");
    let m = &offered[0];
    assert!(stamped(m), "the place carries the stamp: {m:?}");
    assert!(moves::binary_moves(&View::new(&log)).iter().any(|b| moves::same_move(b, m)), "only the binary writes it");
    let files: Vec<&str> = m.fields["places"].as_array().into_iter().flatten().filter_map(|p| p["file_path"].as_str()).collect();
    assert_eq!(files, ["src/login.py"], "the file the code landed in since the seal");
    let check = m.more.as_ref().and_then(|more| more.iter().find(|f| f.kind == "anchor_sha_matched"));
    assert!(check.is_some_and(|c| c.fields.get("checked_at") == Some(&json!("planning"))), "its planning check lands with it: {m:?}");
    let log = apply_move(&log, m, AT, TENANT).expect("lands");
    assert!(places_offered(&log).iter().all(|m| !stamped(m)), "once: {:?}", places_offered(&log));
}

#[test]
fn the_kept_place_keeps_the_epoch_and_the_seal_and_the_part_goes_green() {
    let before = answer(&untouched(), "keep_going");
    let v0 = View::new(&before);
    let (e0, seal0) = (epoch(&v0, "c1"), sealed_at(&v0, "c1", Place::Laptop).map(|s| s.t));
    assert!(seal0.is_some());
    let m = places_offered(&before).into_iter().find(stamped).expect("the binary's place");
    let log = apply_move(&before, &m, AT, TENANT).expect("lands");
    let v = View::new(&log);
    assert!(plan::anchor_of(&v, "c1").is_some_and(|a| a.fields.get("hold") == Some(&json!("anchor_untouched"))), "the anchor is the kept place");
    assert_eq!(epoch(&v, "c1"), e0, "the kept place does not move the epoch");
    assert_eq!(sealed_at(&v, "c1", Place::Laptop).map(|s| s.t), seal0, "the red seal still counts");
    let log = settle(log);
    assert!(whys(&log).is_empty(), "nothing stuck, not_stamped included: {:?}", whys(&log));
    assert_eq!(state(&log), "green", "{}", terminal::status(&View::new(&log)));
}

#[test]
fn re_plan_is_unchanged_the_place_is_chosen_again_and_the_proof_starts_over() {
    let held = untouched();
    let e0 = epoch(&View::new(&held), "c1");
    let log = answer(&held, "re_plan");
    assert!(places_offered(&log).iter().all(|m| !stamped(m)), "{:?}", places_offered(&log));
    assert!(plan::anchor_of(&View::new(&log), "c1").is_none());
    let log = step(&log, "re_anchor", None);
    let v = View::new(&log);
    assert!(epoch(&v, "c1") > e0, "a re-anchor starts the proof again");
    assert!(sealed_at(&v, "c1", Place::Laptop).is_none());
    assert_eq!(state(&log), "testing");
}
