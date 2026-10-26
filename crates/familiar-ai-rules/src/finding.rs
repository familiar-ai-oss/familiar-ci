//! The investigator's finding on a test that never went red [draft words] (the owner,
//! 2026-10-16, S7b). The binary writes `investigator_finding` after it re-read every quote; a
//! diagnosis that gave none is `investigator_unfinished`. The finding clears no hold, raises
//! none, narrows no card and relabels no run: only the rescope gate reads it, after the person's
//! `re_plan` on a `requirement_missed` finding.

use familiar_ai_facts::Fact;
use serde_json::{json, Value};

use crate::lifecycle::holds;
use crate::plan;
use crate::reader::{last_of, t_of, text, View};
use crate::sample::{fields_of, Move, MoveFact};

/// The hold a finding is owed for.
pub const HOLD: &str = "two_tries_not_red";
/// The diagnoses that may give no finding before the card goes to the person bare.
pub const MISSES: usize = 2;
/// The category whose `re_plan` asks for a new reading of the part.
pub const RESCOPES: &str = "requirement_missed";

/// The `t` of the person's last answer to `HOLD` on `c`, or -1: a rise is owed its own finding.
pub fn since(v: &View, c: &str) -> i64 {
    last_of(v.of("hold_answered", c).filter(|f| text(f, "hold") == Some(HOLD)).collect()).map(t_of).unwrap_or(-1)
}

/// The finding of `c` since the person's last answer to `HOLD`.
pub fn finding<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    let s = since(v, c);
    last_of(v.of("investigator_finding", c).filter(|f| t_of(f) > s).collect())
}

/// The diagnoses of `c` that gave no finding since the person's last answer to `HOLD`.
pub fn misses(v: &View, c: &str) -> usize {
    let s = since(v, c);
    v.of("investigator_unfinished", c).filter(|f| t_of(f) > s).count()
}

/// Whether a diagnosis is owed for `c`: `HOLD` stands with no finding and fewer than `MISSES`
/// misses since the person's last answer to it.
pub fn owed(v: &View, c: &str) -> bool {
    holds(v, c).iter().any(|h| h.why == HOLD) && finding(v, c).is_none() && misses(v, c) < MISSES
}

/// A finding's category word.
pub fn category(f: &Fact) -> Option<&str> {
    text(f, "category")
}

/// The finding the person's answer `a` read: the latest before it, after the answer before it.
pub fn read_by<'a>(v: &View<'a>, c: &str, a: &Fact) -> Option<&'a Fact> {
    let before = v.prefix(a.t as usize);
    finding(&before, c).and_then(|f| v.at(f.t as usize))
}

/// The person's `re_plan` on `HOLD` that read a `requirement_missed` finding, while it is their
/// latest answer to `HOLD`.
pub fn rescoped<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    let a = last_of(v.of("hold_answered", c).filter(|f| text(f, "hold") == Some(HOLD)).collect())?;
    if text(a, "choice") != Some("re_plan") {
        return None;
    }
    (read_by(v, c, a).and_then(category) == Some(RESCOPES)).then_some(a)
}

/// The latest `criterion` of `c` before `t`.
fn criterion_before<'a>(v: &View<'a>, c: &str, t: i64) -> Option<&'a Fact> {
    last_of(v.of("criterion", c).filter(|f| t_of(f) < t).collect())
}

/// Whether line 2 waits for `c`'s new reading: after the rescoping `re_plan`, no criterion
/// claimed since whose text differs from the one the answer read (presence plus difference).
pub fn criterion_owed(v: &View, c: &str) -> bool {
    let Some(a) = rescoped(v, c) else {
        return false;
    };
    let read = criterion_before(v, c, t_of(a)).and_then(|f| text(f, "text"));
    // the latest claim since the answer counts: the old words claimed again hold the gate again
    !last_of(v.of("criterion", c).collect()).is_some_and(|f| t_of(f) > t_of(a) && text(f, "text") != read)
}

fn slots(p: &Fact) -> [Option<&str>; 3] {
    [text(p, "accepts"), text(p, "gives_back"), text(p, "refuses")]
}

/// Whether line 3 waits for `c`'s new promise, on the full path: after the rescoping `re_plan`,
/// no promise claimed since for the criterion's promise whose slots differ from the one the
/// answer read.
pub fn promise_owed(v: &View, c: &str) -> bool {
    if plan::path_of(v, c) != Some("full") {
        return false;
    }
    let Some(a) = rescoped(v, c) else {
        return false;
    };
    let at = t_of(a);
    let pid_read = criterion_before(v, c, at).and_then(|f| text(f, "promise_id"));
    let read = last_of(v.kind("promise").filter(|p| t_of(p) < at && text(p, "promise_id") == pid_read).collect()).map(slots);
    let pid = last_of(v.of("criterion", c).collect()).and_then(|f| text(f, "promise_id"));
    !last_of(v.kind("promise").filter(|p| text(p, "promise_id") == pid).collect()).is_some_and(|p| t_of(p) > at && Some(slots(p)) != read)
}

fn mv(kind: &str, fields: Value, more: Vec<MoveFact>, label: &str) -> Move {
    Move { kind: kind.to_string(), fields: fields_of(fields), more: Some(more), label: Some(label.to_string()) }
}

/// The sample's findings, one per category, in the order the investigator checks them.
pub fn sample_finding(c: &str, category: &str) -> Value {
    let mut f = json!({
        "criterion_id": c, "category": category, "model": "claude-opus-4-6",
        "runs": [], "reason": "the sample's reason", "quotes": [],
    });
    match category {
        "wrong_test" => f["guidance"] = json!("the sample's guidance"),
        "requirement_missed" => {
            f["missed"] = json!("the sample's missed requirement");
            f["scope"] = json!("the sample's scope");
        }
        _ => {}
    }
    f
}

/// The binary's moves for `c` while a diagnosis is owed: a finding of each category (label
/// `finding`), or a diagnosis that gave none (`finding_missed`).
pub fn finding_moves(v: &View, c: &str) -> Vec<Move> {
    if !owed(v, c) {
        return Vec::new();
    }
    let mut out: Vec<Move> = familiar_ai_facts::FINDING_CATEGORIES.iter().map(|k| mv("investigator_finding", sample_finding(c, k), Vec::new(), "finding")).collect();
    let conv = format!("{}/d{}", crate::ladder::session_of(plan::assigned_to(v, c)).unwrap_or("null"), misses(v, c) + 1);
    out.push(mv("investigator_unfinished", json!({ "criterion_id": c, "conversation": conv, "why": "unverified" }), Vec::new(), "finding_missed"));
    out
}

/// The agent's new reading of `c` after the rescoping `re_plan` (label `rescope`): the criterion
/// claimed again with what was missed, and on the full path its promise with it.
pub fn rescope_moves(v: &View, c: &str) -> Vec<Move> {
    if !criterion_owed(v, c) && !promise_owed(v, c) {
        return Vec::new();
    }
    let Some(crit) = last_of(v.of("criterion", c).collect()) else {
        return Vec::new();
    };
    let missed = rescoped(v, c).and_then(|a| read_by(v, c, a)).and_then(|f| text(f, "missed")).unwrap_or("what was missed");
    // the new words take what was missed in, or, read again after a later rescope, out: the
    // sample's two readings, so a walk stays finite
    let toggled = |now: &str, with: String| match now.strip_suffix(with.as_str()) {
        Some(before) => before.to_string(),
        None => format!("{now}{with}"),
    };
    let mut promise = Vec::new();
    if promise_owed(v, c) {
        let pid = text(crit, "promise_id");
        if let Some(p) = last_of(v.kind("promise").filter(|p| text(p, "promise_id") == pid).collect()) {
            let mut pf = p.fields.clone();
            pf.insert("refuses".into(), Value::from(toggled(text(p, "refuses").unwrap_or(""), format!(", and {missed}"))));
            promise.push(MoveFact { kind: "promise".into(), fields: pf });
        }
    }
    if !criterion_owed(v, c) {
        return promise.into_iter().map(|p| mv(&p.kind, Value::Object(p.fields), Vec::new(), "rescope")).collect();
    }
    let mut fields = crit.fields.clone();
    fields.insert("text".into(), Value::from(toggled(text(crit, "text").unwrap_or(""), format!(", with {missed}"))));
    vec![mv("criterion", Value::Object(fields), promise, "rescope")]
}
