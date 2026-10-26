//! The investigator's finding on a test that never went red [draft words] (the owner,
//! 2026-10-16, S7b): owed while `two_tries_not_red` stands with no finding and fewer than two
//! misses since the person's last answer to it; it clears no hold, raises none, narrows no card
//! and relabels no run. After a `re_plan` on a `requirement_missed` finding, line 2 holds only
//! once a criterion claimed after the answer differs in text, and line 3 (the full path) once a
//! promise does.

use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::criterion::{sealed_at, PLAN_KINDS, Place};
use familiar_ai_rules::sample::{apply_move, criterion_fixture, Move};
use familiar_ai_rules::{finding, lifecycle, moves, plan, terminal, View};
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";

fn fact(log: &Log, kind: &str, fields: Value) -> Log {
    append_raw(log, kind, fields.as_object().cloned().unwrap_or_default(), AT, TENANT).unwrap_or_else(|e| panic!("{kind}: {e}"))
}

fn find(log: &Log, label: &str, outcome: Option<&str>) -> Option<Move> {
    let v = View::new(log);
    moves::criterion_admissible(&v).into_iter().find(|m| {
        m.label.as_deref() == Some(label) && outcome.is_none_or(|o| m.fields.get("outcome").and_then(Value::as_str) == Some(o))
    })
}

fn step(log: &Log, label: &str, outcome: Option<&str>) -> Log {
    match find(log, label, outcome) {
        Some(m) => apply_move(log, &m, AT, TENANT).unwrap_or_default(),
        None => panic!("no {label} move at:\n{}", terminal::status(&View::new(log))),
    }
}

fn tests_commit(log: &Log, label: &str, outcome: &str) -> Log {
    let mut log = step(log, label, None);
    if label == "tests_only" {
        log = step(&log, "test_written", None);
    }
    log = step(&log, "pass_please_pass", Some(outcome));
    log = step(&log, "pass_please_pass", Some("passed"));
    step(&log, "fingerprint_stamped", None)
}

fn whys(log: &Log) -> Vec<String> {
    lifecycle::whys(&View::new(log), "c1")
}

/// c1 (full path) held on two_tries_not_red: two tests-first runs that passed.
fn held() -> Log {
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).unwrap_or_default();
    let log = fact(&log, "model_seen", json!({ "agent_conversation_id": "a1", "model": "claude-haiku-4-5-20251001" }));
    let log = step(&log, "start_check", None);
    let log = tests_commit(&log, "tests_only", "passed");
    tests_commit(&log, "changed_test", "passed")
}

fn finding_of(category: &str) -> Value {
    let mut f = json!({
        "criterion_id": "c1", "category": category, "model": "claude-opus-4-6",
        "runs": [{"run_id": "r1", "commit": "k1", "label": "passed"}],
        "reason": "the sample's reason",
        "quotes": [{"role": "person", "said": 0, "text": "log in"}]
    });
    match category {
        "wrong_test" => f["guidance"] = json!("check the session"),
        "requirement_missed" => {
            f["missed"] = json!("a locked account");
            f["scope"] = json!("the password check");
        }
        _ => {}
    }
    f
}

fn answer(log: &Log, choice: &str) -> Log {
    fact(log, "hold_answered", json!({ "criterion_id": "c1", "hold": "two_tries_not_red", "choice": choice, "typed": "" }))
}

fn without_findings(log: &Log) -> Log {
    let mut out: Log = Vec::new();
    for f in log.iter().filter(|f| f.kind != "investigator_finding") {
        out = append_raw(&out, f.kind, f.fields.clone(), f.at, &f.tenant).unwrap_or_default();
    }
    out
}

fn card_choices(log: &Log) -> Vec<Vec<String>> {
    terminal::cards(&View::new(log)).into_iter().map(|k| k.choices.into_iter().map(|(c, _)| c).collect()).collect()
}

#[test]
fn it_is_owed_while_the_hold_stands_with_no_finding_and_two_misses_end_it() {
    let log = held();
    assert!(finding::owed(&View::new(&log), "c1"), "two_tries_not_red with no finding");
    let miss = |log: &Log| fact(log, "investigator_unfinished", json!({ "criterion_id": "c1", "conversation": "a1/d1", "why": "unverified" }));
    let once = miss(&log);
    assert!(finding::owed(&View::new(&once), "c1"), "one miss: still owed");
    let twice = miss(&once);
    assert!(!finding::owed(&View::new(&twice), "c1"), "two misses: the bare card");
    for category in ["wrong_test", "behavior_exists", "requirement_missed"] {
        let found = fact(&log, "investigator_finding", finding_of(category));
        let v = View::new(&found);
        assert!(!finding::owed(&v, "c1"), "{category}");
        assert_eq!(finding::finding(&v, "c1").and_then(finding::category), Some(category));
    }
}

#[test]
fn a_finding_clears_nothing_raises_nothing_narrows_no_card_and_relabels_no_run() {
    for category in ["wrong_test", "behavior_exists", "requirement_missed"] {
        let log = fact(&held(), "investigator_finding", finding_of(category));
        let bare = without_findings(&log);
        let (v, b) = (View::new(&log), View::new(&bare));
        assert_eq!(whys(&log), whys(&bare), "{category}");
        assert!(whys(&log).iter().any(|w| w == "two_tries_not_red"), "{category}: the hold stands");
        assert_eq!(lifecycle::released(&v, "c1"), lifecycle::released(&b, "c1"), "{category}");
        assert_eq!(lifecycle::state(&v, "c1"), lifecycle::state(&b, "c1"), "{category}");
        assert!(sealed_at(&v, "c1", Place::Laptop).is_none(), "{category}: the passing run seals nothing");
        assert_eq!(card_choices(&log), [["try_again", "re_plan", "release"]], "{category}");
        assert_eq!(card_choices(&log), card_choices(&bare), "{category}");
    }
}

#[test]
fn a_finding_from_before_the_answer_does_not_serve_the_next_rise() {
    let log = fact(&held(), "investigator_finding", finding_of("wrong_test"));
    let log = answer(&log, "try_again");
    assert!(!finding::owed(&View::new(&log), "c1"), "the hold is answered");
    assert_eq!(finding::finding(&View::new(&log), "c1"), None, "the answer closes the finding it read");
    let log = tests_commit(&log, "changed_test", "passed");
    let log = tests_commit(&log, "changed_test", "passed");
    assert!(whys(&log).iter().any(|w| w == "two_tries_not_red"), "{:?}", whys(&log));
    assert!(finding::owed(&View::new(&log), "c1"), "the next rise is owed its own finding");
}

#[test]
fn after_a_re_plan_on_a_requirement_missed_finding_the_criterion_and_its_promise_are_claimed_again() {
    let log = fact(&held(), "investigator_finding", finding_of("requirement_missed"));
    let log = answer(&log, "re_plan");
    let v = View::new(&log);
    assert!(plan::unchosen(&v, "c1").is_some(), "the place is chosen again");
    assert!(finding::criterion_owed(&v, "c1") && finding::promise_owed(&v, "c1"));
    assert!(!plan::line_2(&v) && !plan::line_3(&v), "lines 2 and 3 wait for the new reading");
    let text = v.of("criterion", "c1").last().and_then(|f| f.fields.get("text")).and_then(Value::as_str).unwrap_or("").to_string();
    let claim = |log: &Log, text: &str| fact(log, "criterion", json!({ "criterion_id": "c1", "text": text, "path": "full", "behavior_id": "b1", "promise_id": "p1" }));
    let same = claim(&log, &text);
    assert!(!plan::line_2(&View::new(&same)), "the same words are not a new reading");
    let new = claim(&same, &format!("{text}, and a locked account is refused"));
    let v = View::new(&new);
    assert!(plan::line_2(&v) && !finding::criterion_owed(&v, "c1"), "a criterion that differs");
    assert!(!plan::line_3(&v), "the promise is still the old one");
    let promise = |log: &Log, refuses: &str| {
        fact(log, "promise", json!({ "promise_id": "p1", "behavior_id": "b1", "accepts": "a user name and a password", "gives_back": "a session", "refuses": refuses }))
    };
    let old = promise(&new, "a wrong password");
    assert!(!plan::line_3(&View::new(&old)), "the same promise again");
    let done = promise(&old, "a wrong password and a locked account");
    let v = View::new(&done);
    assert!(plan::line_2(&v) && plan::line_3(&v) && !finding::promise_owed(&v, "c1"));
    // the place is still chosen again: the rescope gate is beside it, not instead of it
    assert!(!plan::line_4(&v));
}

#[test]
fn a_re_plan_on_another_category_asks_no_new_reading() {
    for category in ["wrong_test", "behavior_exists"] {
        let log = answer(&fact(&held(), "investigator_finding", finding_of(category)), "re_plan");
        let v = View::new(&log);
        assert!(!finding::criterion_owed(&v, "c1") && !finding::promise_owed(&v, "c1"), "{category}");
        assert!(plan::line_2(&v) && plan::line_3(&v), "{category}");
    }
    // with no finding (two misses, or the investigator off) the re_plan asks none either
    let log = answer(&held(), "re_plan");
    assert!(plan::line_2(&View::new(&log)));
}

#[test]
fn the_moves_offer_one_finding_per_category_and_a_miss_while_owed_and_the_rescope_claim_after() {
    let log = held();
    let v = View::new(&log);
    let found: Vec<String> = moves::criterion_admissible(&v)
        .into_iter()
        .filter(|m| m.label.as_deref() == Some("finding"))
        .filter_map(|m| m.fields.get("category").and_then(Value::as_str).map(str::to_string))
        .collect();
    assert_eq!(found, ["wrong_test", "behavior_exists", "requirement_missed"]);
    assert!(find(&log, "finding_missed", None).is_some());
    let log = step(&log, "finding", None);
    assert!(find(&log, "finding", None).is_none() && find(&log, "finding_missed", None).is_none(), "one finding per rise");
    let log = fact(&held(), "investigator_finding", finding_of("requirement_missed"));
    let log = answer(&log, "re_plan");
    let log = step(&log, "rescope", None);
    let v = View::new(&log);
    assert!(plan::line_2(&v) && plan::line_3(&v), "the rescope claims a new criterion and promise");
    let log = step(&log, "re_anchor", None);
    assert!(plan::planning_confirm_complete(&View::new(&log)));
}

/// Review 5a: the latest reading counts. A new criterion (or promise) claimed and then the old one
/// again leaves the gate holding.
#[test]
fn the_old_reading_claimed_again_after_a_new_one_holds_the_gate_again() {
    let log = answer(&fact(&held(), "investigator_finding", finding_of("requirement_missed")), "re_plan");
    let v = View::new(&log);
    let text = v.of("criterion", "c1").last().and_then(|f| f.fields.get("text")).and_then(Value::as_str).unwrap_or("").to_string();
    let claim = |log: &Log, text: &str| fact(log, "criterion", json!({ "criterion_id": "c1", "text": text, "path": "full", "behavior_id": "b1", "promise_id": "p1" }));
    let promise = |log: &Log, refuses: &str| {
        fact(log, "promise", json!({ "promise_id": "p1", "behavior_id": "b1", "accepts": "a user name and a password", "gives_back": "a session", "refuses": refuses }))
    };
    let new = promise(&claim(&log, &format!("{text}, and a locked account is refused")), "a wrong password and a locked account");
    assert!(plan::line_2(&View::new(&new)) && plan::line_3(&View::new(&new)));
    let back = claim(&new, &text);
    assert!(finding::criterion_owed(&View::new(&back), "c1") && !plan::line_2(&View::new(&back)), "the old words again");
    let back = promise(&new, "a wrong password");
    assert!(finding::promise_owed(&View::new(&back), "c1") && !plan::line_3(&View::new(&back)), "the old promise again");
}

/// The sample's rescope has two readings: what was missed taken in, and, after a later rescope,
/// taken out again, so a walk over it stays finite.
#[test]
fn the_sample_rescope_takes_what_was_missed_in_and_out_again() {
    let text = |log: &Log| View::new(log).of("criterion", "c1").last().and_then(|f| f.fields.get("text")).and_then(Value::as_str).unwrap_or("").to_string();
    let refuses = |log: &Log| View::new(log).kind("promise").last().and_then(|f| f.fields.get("refuses")).and_then(Value::as_str).unwrap_or("").to_string();
    let first = held();
    let (t0, r0) = (text(&first), refuses(&first));
    let once = step(&answer(&fact(&first, "investigator_finding", finding_of("requirement_missed")), "re_plan"), "rescope", None);
    assert_eq!((text(&once), refuses(&once)), (format!("{t0}, with a locked account"), format!("{r0}, and a locked account")));
    let twice = step(&answer(&fact(&once, "investigator_finding", finding_of("requirement_missed")), "re_plan"), "rescope", None);
    assert_eq!((text(&twice), refuses(&twice)), (t0, r0), "the second rescope takes it out again");
}

#[test]
fn the_finding_is_no_plan_kind() {
    assert!(!PLAN_KINDS.contains(&"investigator_finding"));
}
