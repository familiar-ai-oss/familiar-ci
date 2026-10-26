//! The reviewer at green (the owner's plan of 2026-10-20, step 3) [draft names]: the binary writes
//! `fix_verdict` once a reviewer read a green fix, and a "no" only when it re-read every quote at
//! the green commit. The rules read it two ways and leave green itself as it was:
//! - `review::standing_no`: the latest verdict on the part is a "no", and the person has not kept
//!   the fix as it is; the binary refuses the stop while one stands. A later verdict, at a later
//!   commit, replaces it; a "not judged" never blocks.
//! - the hold `review_disputed`: two verified "no"s on one part are the person's card, answered
//!   `keep_going` (the fix stays as it is, no "no" stands any more), `try_again` (one more review
//!   after another fix; one more "no" is the card again) or `release`. It pauses the part.

use familiar_ai_facts::Log;
use familiar_ai_rules::criterion::{answers_of, PAUSING};
use familiar_ai_rules::sample::{apply_move, criterion_fixture};
use familiar_ai_rules::{lifecycle, moves, review, terminal, View};
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";

fn fact(log: &Log, kind: &str, fields: Value) -> Log {
    familiar_ai_facts::append_raw(log, kind, fields.as_object().cloned().unwrap_or_default(), AT, TENANT).unwrap_or_else(|e| panic!("{kind}: {e}"))
}

fn step(log: &Log, label: &str, outcome: Option<&str>) -> Log {
    let v = View::new(log);
    let m = moves::criterion_admissible(&v)
        .into_iter()
        .find(|m| m.label.as_deref() == Some(label) && outcome.is_none_or(|o| m.fields.get("outcome").and_then(Value::as_str) == Some(o)));
    match m {
        Some(m) => apply_move(log, &m, AT, TENANT).unwrap_or_default(),
        None => panic!("no {label} move at:\n{}", terminal::status(&v)),
    }
}

/// A commit's run: the own test with `outcome`, the touching test passing, then the stamp.
fn ran(log: &Log, outcome: &str) -> Log {
    let log = step(log, "pass_please_pass", Some(outcome));
    let log = step(&log, "pass_please_pass", Some("passed"));
    step(&log, "fingerprint_stamped", None)
}

fn head(log: &Log) -> String {
    log.iter().rev().find(|f| f.kind == "commit_seen").and_then(|f| f.fields.get("commit")).and_then(Value::as_str).unwrap_or("").to_string()
}

/// c1 sealed red, then green at a code commit.
fn green() -> Log {
    let log = criterion_fixture(&["c1"], true, false, AT, TENANT).unwrap_or_default();
    let log = step(&log, "start_check", None);
    let log = step(&log, "tests_only", None);
    let log = step(&log, "test_written", None);
    let log = ran(&log, "failed");
    let log = ran(&step(&log, "code", None), "passed");
    assert_eq!(state(&log), "green", "{}", terminal::status(&View::new(&log)));
    log
}

fn state(log: &Log) -> &'static str {
    lifecycle::state(&View::new(log), "c1")
}

fn whys(log: &Log) -> Vec<String> {
    lifecycle::whys(&View::new(log), "c1")
}

fn verdict(log: &Log, covers: &str) -> Log {
    let mut f = json!({ "criterion_id": "c1", "commit": head(log), "model": "claude-opus-4-6", "covers": covers, "reason": "the reviewer's reason" });
    match covers {
        "no" => {
            f["gap"] = json!("the precedence the docstring documents");
            f["quotes"] = json!([{ "file_path": "src/login.py", "line": 3, "text": "Values are read from the file first." }]);
        }
        "not_judged" => {
            f["why"] = json!("unverified");
            f["detail"] = json!("quote 1: src/login.py line 3 does not read as quoted");
        }
        _ => {}
    }
    fact(log, "fix_verdict", f)
}

fn answer(log: &Log, choice: &str) -> Log {
    fact(log, "hold_answered", json!({ "criterion_id": "c1", "hold": review::HOLD, "choice": choice, "typed": "" }))
}

fn standing(log: &Log) -> bool {
    review::standing_no(&View::new(log), "c1").is_some()
}

/// Another code commit: the last code commit's facts again under a new id, which the rules read
/// as the same content, so the part stays green at the new commit.
fn fixed_again(log: &Log) -> Log {
    let at = log.iter().rposition(|f| f.kind == "commit_seen" && f.fields.get("touches_code") == Some(&json!(true))).expect("a code commit");
    let old = log[at].fields.get("commit").cloned().unwrap_or_default();
    let id = format!("fix-{}", log.len());
    let mut out = log.clone();
    for f in log[at..].iter().filter(|f| ["commit_seen", "tests_observed"].contains(&f.kind) && f.fields.get("commit") == Some(&old)) {
        let mut fields = Value::Object(f.fields.clone());
        fields["commit"] = json!(id);
        out = fact(&out, f.kind, fields);
    }
    assert_eq!(head(&out), id);
    assert_eq!(state(&out), "green", "{}", terminal::status(&View::new(&out)));
    out
}

#[test]
fn the_hold_is_a_pausing_card_of_three_choices() {
    assert_eq!(review::HOLD, "review_disputed");
    assert_eq!(answers_of(review::HOLD), Some(&["keep_going", "try_again", "release"][..]));
    assert!(PAUSING.contains(&review::HOLD));
}

#[test]
fn a_no_stands_and_green_is_unchanged() {
    let log = green();
    assert!(!standing(&log), "no verdict, nothing stands");
    let no = verdict(&log, "no");
    assert!(standing(&no));
    assert_eq!(state(&no), "green", "green itself is the rules' as it was");
    assert!(whys(&no).is_empty(), "one no is no card: {:?}", whys(&no));
    assert_eq!(review::standing_no(&View::new(&no), "c1").map(|f| f.t), no.last().map(|f| f.t));
}

#[test]
fn covers_or_not_judged_stand_nothing_and_a_later_verdict_replaces_a_no() {
    let log = green();
    assert!(!standing(&verdict(&log, "yes")));
    assert!(!standing(&verdict(&log, "not_judged")), "not judged never blocks");
    let no = verdict(&log, "no");
    let again = fixed_again(&no);
    assert!(standing(&again), "the no stands until the new commit is reviewed");
    assert!(!standing(&verdict(&again, "yes")));
    assert!(!standing(&verdict(&again, "not_judged")), "a later not judged replaces it and never blocks");
}

#[test]
fn two_nos_are_the_persons_card_and_keep_going_keeps_the_fix() {
    let once = verdict(&green(), "no");
    let twice = verdict(&fixed_again(&once), "no");
    assert_eq!(whys(&twice), [review::HOLD]);
    assert!(review::disputed(&View::new(&twice), "c1"));
    assert_eq!(state(&twice), "held");
    let v = View::new(&twice);
    let card = terminal::cards(&v).into_iter().find(|c| c.hold == review::HOLD).expect("the card");
    assert_eq!(card.choices.iter().map(|(c, _)| c.as_str()).collect::<Vec<_>>(), ["keep_going", "try_again", "release"]);
    assert!(card.question.contains("the precedence the docstring documents"), "{}", card.question);
    // the person keeps the fix as it is: nothing stands, the part is green
    let kept = answer(&twice, "keep_going");
    assert!(whys(&kept).is_empty() && !standing(&kept) && review::accepted(&View::new(&kept), "c1"));
    assert_eq!(state(&kept), "green");
}

#[test]
fn try_again_is_one_more_review_after_another_fix() {
    let once = verdict(&green(), "no");
    let twice = verdict(&fixed_again(&once), "no");
    let again = answer(&twice, "try_again");
    assert!(whys(&again).is_empty(), "{:?}", whys(&again));
    assert!(standing(&again), "the no stands until another fix is reviewed");
    let fixed = fixed_again(&again);
    assert!(!standing(&verdict(&fixed, "yes")));
    let third = verdict(&fixed, "no");
    assert_eq!(whys(&third), [review::HOLD], "one more no is the card again");
}

#[test]
fn not_judged_never_counts_toward_the_card() {
    let once = verdict(&green(), "no");
    let log = verdict(&fixed_again(&once), "not_judged");
    let log = verdict(&fixed_again(&log), "not_judged");
    assert!(whys(&log).is_empty(), "{:?}", whys(&log));
    assert!(!standing(&log));
}
