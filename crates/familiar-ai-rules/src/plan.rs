//! The plan's eleven lines and its state, derived from the facts and ported
//! from the reference model (not in this repository). Approval is derived
//! from `planning_confirm_complete`; no fact records a person approving.

use familiar_ai_facts::Fact;
use serde_json::Value;

use crate::reader::{int, is_of, last_of, text, texts, t_of, View};

/// The three ways to anchor a criterion (plan line 4).
pub const EXIT_KINDS: [&str; 3] = ["place_chosen", "new_work_spot_chosen", "new_work_no_spot"];
const STRING_KINDS: [&str; 2] = ["criterion_head", "criterion_strung_on_head"];

/// The plan states; exactly one holds at a time.
pub const PLAN_ORDER: [&str; 2] = ["confirmed", "planning"];
/// The plan's eleven lines, by name, in order.
pub const PLAN_LINES: [&str; 11] = [
    "line_1", "line_2", "line_3", "line_4", "line_5", "line_6", "line_7", "line_8", "line_9",
    "line_10", "line_11",
];

/// One place of a `place_chosen`, with the four fields a planning check names.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaceRef<'a> {
    pub file_path: &'a str,
    pub symbol: &'a str,
    pub line_start: Option<i64>,
    pub line_end: Option<i64>,
}

/// The places of a `place_chosen` fact.
pub fn places_of(a: &Fact) -> Vec<PlaceRef<'_>> {
    a.fields
        .get("places")
        .and_then(Value::as_array)
        .map(|ps| {
            ps.iter()
                .map(|p| PlaceRef {
                    file_path: p.get("file_path").and_then(Value::as_str).unwrap_or(""),
                    symbol: p.get("symbol").and_then(Value::as_str).unwrap_or(""),
                    line_start: p.get("line_start").and_then(Value::as_i64),
                    line_end: p.get("line_end").and_then(Value::as_i64),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The distinct criterion ids of `criterion` facts, in first-seen order.
pub fn criteria(v: &View) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for f in v.kind("criterion") {
        if let Some(c) = f.criterion_id() {
            if !out.iter().any(|x| x == c) {
                out.push(c.to_string());
            }
        }
    }
    out
}

/// The criterion's path, full or light, from its latest `criterion` fact.
pub fn path_of<'a>(v: &View<'a>, c: &str) -> Option<&'a str> {
    last_of(v.of("criterion", c).collect()).and_then(|f| text(f, "path"))
}

/// The criterion's last anchor exit, unless a `re_plan` answer to
/// `anchor_moved` came after it, which sends planning back to choosing the code.
pub fn anchor_of<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    let a = last_exit(v, c)?;
    if unchosen(v, c).is_some() {
        return None;
    }
    Some(a)
}

pub(crate) fn last_exit<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    v.facts
        .iter()
        .rev()
        .find(|f| EXIT_KINDS.contains(&f.kind) && is_of(f, c))
}

/// Whether an anchor exit is the place where the change landed, which the binary
/// appends right after the person's keep_going on `anchor_untouched`: a
/// `place_chosen` carrying that answer's hold and choice, the stamp that the
/// person overrode the plan (the owner, 2026-10-13, F12).
pub fn kept(a: &Fact) -> bool {
    a.kind == "place_chosen" && text(a, "hold") == Some("anchor_untouched") && text(a, "choice") == Some("keep_going")
}

/// The criterion's last anchor exit that is not a kept place: where its proof
/// is judged from, since a kept place keeps the proof.
pub fn epoch_exit<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    v.facts
        .iter()
        .rev()
        .find(|f| EXIT_KINDS.contains(&f.kind) && is_of(f, c) && !kept(f))
}

/// The person's keep_going on `anchor_untouched` after the criterion's last
/// anchor exit: while it stands the place where the change landed is owed,
/// and only the binary appends it.
pub fn keep_going_answer<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    let a = last_exit(v, c)?;
    v.of("hold_answered", c)
        .filter(|f| {
            text(f, "hold") == Some("anchor_untouched") && text(f, "choice") == Some("keep_going") && t_of(f) > t_of(a)
        })
        .last()
}

/// The holds whose `re_plan` means choosing the place again.
pub const REPLAN_HOLDS: [&str; 11] = [
    "anchor_moved",
    "anchor_untouched",
    "two_tries_not_red",
    "two_test_broken",
    "two_crashed",
    "two_timed_out",
    "environment_error",
    "run_unclassified",
    "retry_spent",
    "miscoped",
    "setup_changed",
];

/// Whether a fact is the person's `re_plan` answer to a hold of `REPLAN_HOLDS`.
pub fn is_re_plan(f: &Fact) -> bool {
    f.kind == "hold_answered"
        && text(f, "hold").is_some_and(|h| REPLAN_HOLDS.contains(&h))
        && text(f, "choice") == Some("re_plan")
}

/// The latest `re_plan` answer to a hold of `REPLAN_HOLDS` after the
/// criterion's last anchor exit; while it stands no place is chosen.
pub fn unchosen<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    let a = last_exit(v, c)?;
    v.of("hold_answered", c).filter(|f| is_re_plan(f) && t_of(f) > t_of(a)).last()
}

/// The binary's planning check of a place, after the anchor exit.
pub fn check_of<'a>(v: &View<'a>, c: &str, place: &PlaceRef) -> Option<&'a Fact> {
    let a = anchor_of(v, c)?;
    v.facts.iter().rev().find(|f| {
        f.kind == "anchor_sha_matched"
            && is_of(f, c)
            && text(f, "checked_at") == Some("planning")
            && t_of(f) > t_of(a)
            && text(f, "file_path") == Some(place.file_path)
            && text(f, "symbol") == Some(place.symbol)
            && int(f, "line_start") == place.line_start
            && int(f, "line_end") == place.line_end
    })
}

/// One spot of an anchor exit, as `anchor_is_documentation` names it (the
/// owner, 2026-10-11): a place's file, symbol and lines; a new-work anchor's
/// file alone.
#[derive(Debug, Clone, PartialEq)]
pub struct DocSpot {
    pub file_path: String,
    pub symbol: Option<String>,
    pub line_start: Option<i64>,
    pub line_end: Option<i64>,
}

impl DocSpot {
    /// The spot's fields, an absent one left out.
    pub fn to_fields(&self) -> serde_json::Map<String, Value> {
        let mut m = serde_json::Map::new();
        m.insert("file_path".into(), Value::from(self.file_path.as_str()));
        if let Some(s) = &self.symbol {
            m.insert("symbol".into(), Value::from(s.as_str()));
        }
        if let Some(n) = self.line_start {
            m.insert("line_start".into(), Value::from(n));
        }
        if let Some(n) = self.line_end {
            m.insert("line_end".into(), Value::from(n));
        }
        m
    }
}

/// `docSpotsOf`: what `anchor_is_documentation` names of an anchor exit, one
/// spot per place; a new-work anchor's file alone; `None`: no file.
pub fn doc_spots_of(a: &Fact) -> Option<Vec<DocSpot>> {
    if a.kind == "place_chosen" {
        let ps = a.fields.get("places").and_then(Value::as_array)?;
        return Some(
            ps.iter()
                .map(|p| DocSpot {
                    file_path: p.get("file_path").and_then(Value::as_str).unwrap_or("").to_string(),
                    symbol: p.get("symbol").and_then(Value::as_str).map(str::to_string),
                    line_start: p.get("line_start").and_then(Value::as_i64),
                    line_end: p.get("line_end").and_then(Value::as_i64),
                })
                .collect(),
        );
    }
    exit_files(a).map(|fs| fs.into_iter().map(|file_path| DocSpot { file_path, symbol: None, line_start: None, line_end: None }).collect())
}

/// `doc_of`: the binary's documentation check of one spot of the anchor,
/// after the exit (as `check_of`).
pub fn doc_of<'a>(v: &View<'a>, c: &str, spot: &DocSpot) -> Option<&'a Fact> {
    let a = anchor_of(v, c)?;
    v.facts.iter().rev().find(|f| {
        f.kind == "anchor_is_documentation"
            && is_of(f, c)
            && t_of(f) > t_of(a)
            && text(f, "file_path") == Some(spot.file_path.as_str())
            && text(f, "symbol") == spot.symbol.as_deref()
            && int(f, "line_start") == spot.line_start
            && int(f, "line_end") == spot.line_end
    })
}

fn matched(f: &Fact) -> bool {
    f.fields.get("matched") == Some(&Value::Bool(true))
}

/// `documented`: every spot of the current anchor checked documentation; no
/// anchor: line 4's to say; new work with no file: never documentation.
pub fn documented(v: &View, c: &str) -> bool {
    let Some(a) = anchor_of(v, c) else {
        return true;
    };
    let Some(sp) = doc_spots_of(a) else {
        return false;
    };
    sp.iter().all(|x| doc_of(v, c, x).is_some_and(matched))
}

/// `not_documentation`: a light criterion the binary said no to (or with no
/// file to read): the cure is the full path.
pub fn not_documentation(v: &View, c: &str) -> bool {
    let Some(a) = anchor_of(v, c) else {
        return false;
    };
    if path_of(v, c) != Some("light") {
        return false;
    }
    let Some(sp) = doc_spots_of(a) else {
        return true;
    };
    sp.iter().any(|x| doc_of(v, c, x).is_some_and(|d| !matched(d)))
}

/// The latest stringing fact of a criterion (`criterion_head` or strung).
fn string_fact<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    v.facts
        .iter()
        .rev()
        .find(|f| is_of(f, c) && STRING_KINDS.contains(&f.kind))
}

/// Whether the criterion is a head or strung on a current head; strung on a
/// non-head leaves it off the string.
pub fn on_string(v: &View, c: &str) -> bool {
    let Some(l) = string_fact(v, c) else {
        return false;
    };
    if l.kind == "criterion_head" {
        return true;
    }
    let Some(head) = text(l, "head") else {
        return false;
    };
    if head == c {
        return false;
    }
    string_fact(v, head).is_some_and(|h| h.kind == "criterion_head")
}

/// The criterion just before `c` on its head's string, ordered by the head
/// and then each stringing fact's `t`.
pub fn string_before(v: &View, c: &str) -> Option<String> {
    let l = string_fact(v, c)?;
    if l.kind == "criterion_head" || !on_string(v, c) {
        return None;
    }
    let head = text(l, "head")?;
    let mut sib: Vec<&Fact> = v
        .kind("criterion_strung_on_head")
        .filter(|f| text(f, "head") == Some(head))
        .filter(|f| {
            f.criterion_id()
                .and_then(|x| string_fact(v, x))
                .is_some_and(|h| h.t == f.t)
        })
        .collect();
    sib.sort_by_key(|f| f.t);
    let mut chain: Vec<&str> = vec![head];
    chain.extend(sib.iter().filter_map(|f| f.criterion_id()));
    let i = chain.iter().position(|x| *x == c)?;
    if i > 0 {
        Some(chain[i - 1].to_string())
    } else {
        None
    }
}

/// The conversation that holds the criterion, after any transfer.
pub fn assigned_to<'a>(v: &View<'a>, c: &str) -> Option<&'a str> {
    let l = v
        .facts
        .iter()
        .rev()
        .find(|f| is_of(f, c) && (f.kind == "criterion_assigned" || f.kind == "criterion_transfer"))?;
    if l.kind == "criterion_assigned" {
        text(l, "agent_conversation_id")
    } else {
        text(l, "to_conversation")
    }
}

/// A stack counts only with a non-empty `test_command`; `test_stack`
/// is free text and selects nothing.
pub fn has_command(f: &Fact) -> bool {
    text(f, "test_command").is_some_and(|s| !s.is_empty())
}

/// The files an anchor exit names, empty when it names none.
pub fn file_paths_of(a: &Fact) -> Vec<String> {
    exit_files(a).unwrap_or_default()
}

/// The files an anchor exit's code lives in; `None` when it names none, which
/// means any root.
pub fn exit_files(a: &Fact) -> Option<Vec<String>> {
    match a.kind {
        "place_chosen" => Some(places_of(a).iter().map(|p| p.file_path.to_string()).collect()),
        "new_work_spot_chosen" => Some(text(a, "file_path").map(str::to_string).into_iter().collect()),
        "new_work_no_spot" => text(a, "integration_point").map(|ip| vec![integration_file(ip).to_string()]),
        _ => None,
    }
}

/// The file of an integration point `<file>[:<symbol>]`: the text before the
/// first `:` after the last `/`, since a symbol may itself hold `::` or `.`.
pub fn integration_file(ip: &str) -> &str {
    let from = ip.rfind('/').map_or(0, |i| i + 1);
    match ip[from..].find(':') {
        Some(i) => &ip[..from + i],
        None => ip,
    }
}

fn promise_filled(p: &Fact) -> bool {
    ["accepts", "gives_back", "refuses"]
        .iter()
        .all(|k| text(p, k).is_some_and(|s| !s.is_empty()))
        && contract_wants(p).is_none()
}

/// One case of a promise's contract [draft names], as the rules read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Case {
    pub case_id: String,
    /// `normal`, `opposite` or `edge`; `place` for a listed site that applies.
    pub kind: String,
    pub given: String,
    pub expect: String,
}

/// A promise's cases, in the order claimed; empty when it gives none.
pub fn cases_in(p: &Fact) -> Vec<Case> {
    let s = |c: &Value, k: &str| c.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    p.fields
        .get("cases")
        .and_then(Value::as_array)
        .map(|cs| cs.iter().map(|c| Case { case_id: s(c, "case_id"), kind: s(c, "kind"), given: s(c, "given"), expect: s(c, "expect") }).collect())
        .unwrap_or_default()
}

/// The latest promise `c` names, when it names one that was claimed.
pub fn promise_of<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    let crit = last_of(v.of("criterion", c).collect())?;
    let pid = text(crit, "promise_id")?;
    last_of(v.kind("promise").filter(|f| text(f, "promise_id") == Some(pid)).collect())
}

/// The cases of `c`'s contract: the promise's, then a case of kind `place` for each listed site
/// the agent classified as applying (`places::place_cases`) [draft names]; empty when there are none.
pub fn cases_of(v: &View, c: &str) -> Vec<Case> {
    let mut out = promise_of(v, c).map(cases_in).unwrap_or_default();
    out.extend(crate::places::place_cases(v, c));
    out
}

/// The contract's version the claim door writes on every promise it takes (`contract_version`
/// [draft name]): a promise that carries it, or gives `cases`, is held to its contract.
pub const CONTRACT_VERSION: u64 = 1;

/// What a promise's contract still wants, in the agent's words; `None` when it is whole: at
/// least one normal case and one opposite case, or `opposite_none` with its reason; each case
/// with its own id, what is given and what is expected. Edge cases are optional. A promise
/// written before the contract (no `contract_version`, no `cases`: an old record, a committed
/// plan) reads as it always did; every promise the claim door takes now carries the version.
pub fn contract_wants(p: &Fact) -> Option<String> {
    if !p.fields.contains_key("cases") && !p.fields.contains_key("contract_version") {
        return None;
    }
    let cases = cases_in(p);
    let mut ids: Vec<&str> = Vec::new();
    for k in &cases {
        if k.case_id.trim().is_empty() || k.given.trim().is_empty() || k.expect.trim().is_empty() {
            return Some(format!("each case with its case_id, what is given and what is expected ({} is not)", if k.case_id.trim().is_empty() { "a case" } else { k.case_id.as_str() }));
        }
        if ids.contains(&k.case_id.as_str()) {
            return Some(format!("each case_id once ({} is twice)", k.case_id));
        }
        ids.push(&k.case_id);
    }
    if !cases.iter().any(|k| k.kind == "normal") {
        return Some("a normal case (the call that shows the behavior asked for)".to_string());
    }
    let reason = text(p, "opposite_none").is_some_and(|r| !r.trim().is_empty());
    if !reason && !cases.iter().any(|k| k.kind == "opposite") {
        return Some("an opposite case (the same call with the trigger absent), or --opposite_none with the reason there is none".to_string());
    }
    None
}

/// What the contract of the promise `c` names still wants; `None` when it is whole or gives no
/// cases, or `c` names no claimed promise.
pub fn contract_short(v: &View, c: &str) -> Option<String> {
    promise_of(v, c).and_then(contract_wants)
}

/// 1: an `intent_submitted` with a sha.
pub fn line_1(v: &View) -> bool {
    v.last("intent_submitted")
        .and_then(|i| text(i, "sha"))
        .is_some_and(|s| !s.is_empty())
}

/// 2: at least one criterion; a light one only when the binary checked its
/// anchor is documentation (the owner, 2026-10-11: light is for
/// documentation only).
/// The rescope gate (the owner, 2026-10-16, S7b): after the person's `re_plan` on a
/// `requirement_missed` finding, a criterion claimed since whose text differs.
pub fn line_2(v: &View) -> bool {
    let cs = criteria(v);
    !cs.is_empty() && cs.iter().all(|c| line_2_of(v, c))
}

/// Line 2 for one part: it is claimed, light only when documented, and not owed a new reading.
pub fn line_2_of(v: &View, c: &str) -> bool {
    v.of("criterion", c).next().is_some() && (path_of(v, c) != Some("light") || documented(v, c)) && !crate::finding::criterion_owed(v, c)
}

/// 3: full path ⇒ its behavior row and its latest promise's three slots
/// filled, and its contract whole when it gives cases (`contract_wants`); light needs neither. After a rescoping `re_plan` (S7b), a promise claimed since
/// whose slots differ.
pub fn line_3(v: &View) -> bool {
    criteria(v).iter().all(|c| line_3_of(v, c))
}

/// Line 3 for one part.
pub fn line_3_of(v: &View, c: &str) -> bool {
    if path_of(v, c) != Some("full") {
        return true;
    }
    if crate::finding::promise_owed(v, c) {
        return false;
    }
    let Some(crit) = last_of(v.of("criterion", c).collect()) else {
        return true;
    };
    let bid = text(crit, "behavior_id");
    if !v.kind("behavior").any(|f| text(f, "behavior_id") == bid) {
        return false;
    }
    let pid = text(crit, "promise_id");
    last_of(v.kind("promise").filter(|f| text(f, "promise_id") == pid).collect()).is_some_and(promise_filled)
        // the cases card (the owner's plan of 2026-10-25) [draft]: no finder owed, no card open, and every case the
        // person asked for in the promise
        && !crate::cases::holds(v, c)
        && crate::cases::missing(v, c).is_empty()
}

/// 4: an anchor exit per criterion, the last one counts; for a part whose tests run under a
/// recorder, only after a red try and on its list (`trace::line_4_of`) [draft].
pub fn line_4(v: &View) -> bool {
    criteria(v).iter().all(|c| line_4_of(v, c))
}

/// Line 4 for one part: an anchor, and for a traced part what the try asks of it (the owner's plan
/// of 2026-10-23) [draft].
pub fn line_4_of(v: &View, c: &str) -> bool {
    anchor_of(v, c).is_some_and(|a| crate::trace::placed_after_the_try(v, c, a))
}

/// 5: an anchor that names lines ⇒ the latest planning check matched.
pub fn line_5(v: &View) -> bool {
    criteria(v).iter().all(|c| line_5_of(v, c))
}

/// Line 5 for one part.
pub fn line_5_of(v: &View, c: &str) -> bool {
    let Some(a) = anchor_of(v, c) else {
        return true;
    };
    if a.kind != "place_chosen" {
        return true;
    }
    places_of(a).iter().all(|p| {
        if p.line_start.is_none() || p.line_end.is_none() {
            return true;
        }
        check_of(v, c, p).is_some_and(|ch| ch.fields.get("matched") == Some(&Value::Bool(true)))
    })
}

/// 6: at least one `test_planned`.
pub fn line_6(v: &View) -> bool {
    criteria(v).iter().all(|c| line_6_of(v, c))
}

/// Line 6 for one part.
pub fn line_6_of(v: &View, c: &str) -> bool {
    v.of("test_planned", c).next().is_some()
}

/// 7: a `test_stack_detected` with a non-empty `test_command` for the
/// anchor's root; new work with no integration point: any root.
pub fn line_7(v: &View) -> bool {
    criteria(v).iter().all(|c| line_7_of(v, c))
}

/// Line 7 for one part; with no anchor yet it holds (line 4 is owed).
pub fn line_7_of(v: &View, c: &str) -> bool {
    let Some(a) = anchor_of(v, c) else {
        return true;
    };
    let Some(fps) = exit_files(a) else {
        return v.kind("test_stack_detected").any(has_command);
    };
    fps.iter().all(|fp| {
        v.kind("test_stack_detected")
            .any(|f| has_command(f) && text(f, "project_root").is_some_and(|r| fp.starts_with(r)))
    })
}

/// 8: none declared is allowed; the binary's own search for touching tests is
/// not modelled.
pub fn line_8(_v: &View) -> bool {
    true
}

/// 9: every criterion on the string.
pub fn line_9(v: &View) -> bool {
    criteria(v).iter().all(|c| on_string(v, c))
}

/// 10: every criterion has a `criterion_assigned`.
pub fn line_10(v: &View) -> bool {
    criteria(v).iter().all(|c| line_10_of(v, c))
}

/// Line 10 for one part.
pub fn line_10_of(v: &View, c: &str) -> bool {
    v.of("criterion_assigned", c).next().is_some()
}

/// 11: every criterion has a `fix_location_category`, of any category.
pub fn line_11(v: &View) -> bool {
    criteria(v).iter().all(|c| v.of("fix_location_category", c).next().is_some())
}

/// `test_may_be_written` [draft] (the owner's plan of 2026-10-23, change A): lines 1–3 and 6–10
/// hold for the part `c`, so its planned test files may be written before its place is chosen.
/// Line 7 before an anchor asks for a stack with a command at any root, which the test runs under.
/// `planning_confirm_complete` stays all eleven lines.
pub fn test_may_be_written(v: &View, c: &str) -> bool {
    line_1(v)
        && line_2_of(v, c)
        && line_3_of(v, c)
        && line_6_of(v, c)
        && (if anchor_of(v, c).is_some() { line_7_of(v, c) } else { v.kind("test_stack_detected").any(has_command) })
        && line_8(v)
        && on_string(v, c)
        && line_10_of(v, c)
}

/// The plan's lines, by name.
pub fn line(v: &View, name: &str) -> Option<bool> {
    Some(match name {
        "line_1" => line_1(v),
        "line_2" => line_2(v),
        "line_3" => line_3(v),
        "line_4" => line_4(v),
        "line_5" => line_5(v),
        "line_6" => line_6(v),
        "line_7" => line_7(v),
        "line_8" => line_8(v),
        "line_9" => line_9(v),
        "line_10" => line_10(v),
        "line_11" => line_11(v),
        _ => return None,
    })
}

/// Whether all eleven lines hold.
pub fn planning_confirm_complete(v: &View) -> bool {
    PLAN_LINES.iter().all(|n| line(v, n).unwrap_or(false))
}

/// `confirmed` when every line holds, else `planning`.
pub fn plan_state(v: &View) -> &'static str {
    if planning_confirm_complete(v) {
        "confirmed"
    } else {
        "planning"
    }
}

/// The `test_stack_detected` facts that carry a command.
pub(crate) fn roots_with_command<'a>(v: &View<'a>) -> Vec<&'a Fact> {
    v.kind("test_stack_detected").filter(|f| has_command(f)).collect()
}

/// A stack fact's project root, or empty.
pub(crate) fn root_of(f: &Fact) -> &str {
    text(f, "project_root").unwrap_or("")
}

/// The stack in force for the files before `before`: among the `test_stack_detected` with a
/// command whose root covers every file, the longest root; equal roots, the latest. No files
/// (new work with no integration point): any root covers. The empty root never displaces `src/`.
pub(crate) fn stack_covering<'a>(v: &View<'a>, files: Option<&[String]>, before: i64) -> Option<&'a Fact> {
    let mut best: Option<&'a Fact> = None;
    for f in v.facts.iter() {
        if f.kind != "test_stack_detected"
            || !has_command(f)
            || crate::reader::t_of(f) >= before
            || !files.is_none_or(|fs| fs.iter().all(|fp| fp.starts_with(root_of(f))))
        {
            continue;
        }
        if best.is_none_or(|b| root_of(f).len() >= root_of(b).len()) {
            best = Some(f);
        }
    }
    best
}

#[allow(dead_code)]
pub(crate) fn files_field(f: &Fact) -> Vec<&str> {
    texts(f, "files")
}
