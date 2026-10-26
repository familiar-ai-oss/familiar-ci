//! A cleared hold reads "verified" only when its `hold_answered` names an
//! approval the binary verified against the allowed signers available
//! locally, for the same card (criterion, hold, choice); any other answer
//! reads "not verified" (the owner, 2026-10-17 "Unsigned answers flagged";
//! 2026-10-18 "The local record is advisory; CI is the wall"). The rules stay
//! pure: the binary passes in which approvals verified.

mod common;
use common::*;
use familiar_ai_facts::Log;
use familiar_ai_rules::holds::{answers, Approved};
use familiar_ai_rules::View;
use serde_json::json;

/// c1 sealed red, its fix outside the anchor, its tests passing: held on
/// `anchor_untouched`, then answered `release` with `fields`.
fn answered(extra: serde_json::Value) -> Log {
    let log = step(&sealed(), "code_outside_anchor", None);
    let log = run(&log, "passed");
    let mut fields = json!({"criterion_id": "c1", "hold": "anchor_untouched", "choice": "release", "typed": ""});
    if let (Some(f), Some(e)) = (fields.as_object_mut(), extra.as_object()) {
        f.extend(e.clone());
    }
    raw(&log, "hold_answered", fields)
}

fn approved(t: &str, hold: &str, choice: &str) -> Approved {
    Approved { t: t.into(), criterion_id: "c1".into(), hold: hold.into(), choice: choice.into() }
}

#[test]
fn an_answer_naming_a_verified_approval_for_its_card_reads_verified_with_its_t() {
    let log = answered(json!({"approval": "7"}));
    let a = answers(&View::new(&log), "c1", &[approved("7", "anchor_untouched", "release")]);
    assert_eq!(a.len(), 1, "{a:?}");
    assert_eq!((a[0].hold.as_str(), a[0].choice.as_str()), ("anchor_untouched", "release"));
    assert_eq!(a[0].approval.as_deref(), Some("7"));
    assert_eq!(a[0].verified.as_deref(), Some("7"));
}

#[test]
fn an_answer_with_no_approval_reads_not_verified() {
    let log = answered(json!({}));
    let a = answers(&View::new(&log), "c1", &[approved("7", "anchor_untouched", "release")]);
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].approval, None);
    assert_eq!(a[0].verified, None);
}

#[test]
fn an_answer_naming_an_approval_that_did_not_verify_reads_not_verified() {
    let log = answered(json!({"approval": "7"}));
    for verified in [vec![], vec![approved("8", "anchor_untouched", "release")]] {
        let a = answers(&View::new(&log), "c1", &verified);
        assert_eq!(a[0].approval.as_deref(), Some("7"));
        assert_eq!(a[0].verified, None, "{verified:?}");
    }
}

#[test]
fn a_verified_approval_for_another_card_does_not_verify_this_answer() {
    let log = answered(json!({"approval": "7"}));
    for other in [
        approved("7", "anchor_untouched", "re_plan"),
        approved("7", "two_failures", "release"),
        Approved { t: "7".into(), criterion_id: "c2".into(), hold: "anchor_untouched".into(), choice: "release".into() },
    ] {
        let a = answers(&View::new(&log), "c1", std::slice::from_ref(&other));
        assert_eq!(a[0].verified, None, "{other:?}");
    }
}

#[test]
fn another_criterions_answers_are_not_this_ones() {
    let log = answered(json!({"approval": "7"}));
    assert!(answers(&View::new(&log), "c2", &[approved("7", "anchor_untouched", "release")]).is_empty());
}
