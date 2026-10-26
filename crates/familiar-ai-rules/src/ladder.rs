//! The investigator ladder: a fresh subagent one rung up takes over a criterion held on
//! `two_failures` or `two_tries_not_red`, and hands it back once done.
//! A conversation's rung is read from its latest `model_seen`, never stored.
//! See docs/decisions.md#two-failures-send-an-investigator.

use familiar_ai_facts::Fact;
use serde_json::{json, Map, Value};

use crate::criterion::Clear;
use crate::lifecycle::{holds, released, state};
use crate::plan;
use crate::reader::{is_of, text, View};
use crate::sample::{fields_of, Move, MoveFact};

/// The ladder's rungs, by family word, in order.
pub const LADDER: [&str; 4] = ["haiku", "sonnet", "opus", "fable"];

/// The first ladder word the model's lower-cased name contains, or `None` off the ladder.
pub fn family_of(model: Option<&str>) -> Option<&'static str> {
    let s = model?.to_lowercase();
    LADDER.iter().copied().find(|w| s.contains(w))
}

/// The session of a conversation id; a subagent's id is the session id, a slash, and the agent id.
pub fn session_of(conv: Option<&str>) -> Option<&str> {
    let c = conv?;
    if c.is_empty() {
        return None;
    }
    c.split('/').next()
}

/// Whether `to` is a subagent in `from`'s session other than `from` itself.
pub fn subagent_of(to: Option<&str>, from: Option<&str>) -> bool {
    let Some(t) = to else {
        return false;
    };
    t.contains('/') && Some(t) != from && session_of(to) == session_of(from)
}

/// The model of the conversation's latest `model_seen`.
pub fn model_of<'a>(v: &View<'a>, conv: Option<&str>) -> Option<&'a str> {
    let conv = conv?;
    v.facts
        .iter()
        .rev()
        .find(|f| f.kind == "model_seen" && text(f, "agent_conversation_id") == Some(conv))
        .and_then(|f| text(f, "model"))
}

/// The conversation's rung, or `None` when no model was seen or it is off the ladder;
/// `model_of` tells the two apart.
pub fn rung_of(v: &View, conv: Option<&str>) -> Option<&'static str> {
    family_of(model_of(v, conv))
}

/// The holds an investigator is sent for.
/// D5 (the owner, 2026-10-14): only a valid red climbs; `two_tries_not_red` is the person's card.
pub const SENT_FOR: [&str; 1] = ["two_failures"];

/// Whether the transfer `f` hands a criterion held for an investigator to a fresh subagent of
/// the holder's session, one rung up. It is judged on the log before `f`, which already holds the
/// investigator's `model_seen`.
pub fn investigates(v: &View, f: &Fact) -> bool {
    investigated_for(v, f).is_some()
}

/// The hold the transfer `f` was sent for, or `None` when it is no investigation.
pub fn investigated_for(v: &View, f: &Fact) -> Option<&'static str> {
    let t = f.t as usize;
    if let Some(x) = v.investigates.borrow().get(&t) {
        return *x;
    }
    let x = investigated_uncached(v, f);
    v.investigates.borrow_mut().insert(t, x);
    x
}

/// Whether the binary saw the investigator of transfer `f` end without handing back.
pub fn unfinished(v: &View, f: &Fact) -> bool {
    let to = text(f, "to_conversation");
    v.kind("investigator_unfinished")
        .any(|x| x.t > f.t && x.criterion_id() == f.criterion_id() && text(x, "conversation") == to)
}

fn investigated_uncached(v: &View, f: &Fact) -> Option<&'static str> {
    if f.kind != "criterion_transfer" {
        return None;
    }
    let l = v.prefix(f.t as usize);
    let (Some(c), to, from) = (f.criterion_id(), text(f, "to_conversation"), text(f, "from_conversation")) else {
        return None;
    };
    let named = |x: &Fact| match x.kind {
        "criterion_assigned" => text(x, "agent_conversation_id") == to,
        "criterion_transfer" => text(x, "to_conversation") == to || text(x, "from_conversation") == to,
        _ => false,
    };
    let fresh = !l.facts.iter().any(named);
    let ok = fresh
        && plan::assigned_to(&l, c) == from
        && subagent_of(to, from)
        && investigator_rung(&l, c).is_some_and(|r| rung_of(&l, to) == Some(r));
    if !ok {
        return None;
    }
    let hs = holds(&l, c);
    SENT_FOR.iter().copied().find(|w| hs.iter().any(|h| h.why == *w))
}

/// The open investigator transfers of `c`, oldest first: the climb after its last link whose
/// investigator ended unfinished.
pub fn chain_of<'a>(v: &View<'a>, c: &str) -> Vec<&'a Fact> {
    if let Some(ix) = v.memo.borrow().chain.get(c) {
        return ix.iter().filter_map(|&i| v.at(i)).collect();
    }
    let climb = climb_of(v, c);
    let cut = climb.iter().rposition(|f| unfinished(v, f)).map_or(0, |i| i + 1);
    let out = climb[cut..].to_vec();
    v.memo
        .borrow_mut()
        .chain
        .insert(c.to_string(), out.iter().map(|f| f.t as usize).collect());
    out
}

/// The run of investigator transfers back from the latest holder, closed ones included, empty
/// once `c` is released. The hand-back goes to its first `from_conversation`.
pub fn climb_of<'a>(v: &View<'a>, c: &str) -> Vec<&'a Fact> {
    if let Some(ix) = v.memo.borrow().climb.get(c) {
        return ix.iter().filter_map(|&i| v.at(i)).collect();
    }
    let out = climb_uncached(v, c);
    v.memo
        .borrow_mut()
        .climb
        .insert(c.to_string(), out.iter().map(|f| f.t as usize).collect());
    out
}

fn climb_uncached<'a>(v: &View<'a>, c: &str) -> Vec<&'a Fact> {
    if released(v, c) {
        return Vec::new();
    }
    let hs: Vec<&Fact> = v
        .facts
        .iter()
        .filter(|f| is_of(f, c) && (f.kind == "criterion_assigned" || f.kind == "criterion_transfer"))
        .collect();
    let mut out: Vec<&Fact> = Vec::new();
    for f in hs.into_iter().rev() {
        if f.kind != "criterion_transfer" || !investigates(v, f) {
            break;
        }
        out.push(f);
    }
    out.reverse();
    out
}

/// The open investigator transfer of `c`.
pub fn investigator_of<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    chain_of(v, c).last().copied()
}

/// The rung an investigator of `c` must have now: one above the holder's, and fable once more
/// after fable. An unseen model counts as fable so the ladder never under-escalates; there is
/// none without a holder, for a model off the ladder, or while another investigator is open in
/// the holder's session.
pub fn investigator_rung(v: &View, c: &str) -> Option<&'static str> {
    if let Some(x) = v.memo.borrow().irung.get(c) {
        return *x;
    }
    let x = rung_uncached(v, c);
    v.memo.borrow_mut().irung.insert(c.to_string(), x);
    x
}

fn rung_uncached(v: &View, c: &str) -> Option<&'static str> {
    let h = plan::assigned_to(v, c).filter(|h| !h.is_empty())?;
    let busy = plan::criteria(v).iter().any(|x| {
        x != c
            && investigator_of(v, x)
                .is_some_and(|f| session_of(text(f, "to_conversation")) == session_of(Some(h)))
    });
    if busy {
        return None;
    }
    let Some(r) = rung_of(v, Some(h)) else {
        return if model_of(v, Some(h)).is_none() { Some("fable") } else { None };
    };
    if r != "fable" {
        let i = LADDER.iter().position(|w| *w == r)?;
        return LADDER.get(i + 1).copied();
    }
    let ch = climb_of(v, c);
    let cv: Vec<Option<&str>> = match ch.first() {
        Some(first) => std::iter::once(text(first, "from_conversation"))
            .chain(ch.iter().map(|f| text(f, "to_conversation")))
            .collect(),
        None => vec![Some(h)],
    };
    let fables = cv.into_iter().filter(|x| rung_of(v, *x) == Some("fable")).count();
    (fables < 2).then_some("fable")
}

/// The clear an investigator offers: a transfer from the holder to a fresh subagent at `rung`.
/// `rung` is a key of the clear only; the transfer's `model_seen` carries it.
pub fn investigate_clears(v: &View, c: &str) -> Vec<Clear> {
    let Some(r) = investigator_rung(v, c) else {
        return Vec::new();
    };
    let from = plan::assigned_to(v, c).map(Value::from).unwrap_or(Value::Null);
    vec![fields_of(json!({ "kind": "criterion_transfer", "criterion_id": c, "from_conversation": from, "rung": r }))]
}

/// The sample's id for a fresh subagent, `<session>/i<n+1>`; the binary reads the real one from
/// SubagentStart.
pub fn fresh_subagent(v: &View, holder: &str) -> String {
    let s = session_of(Some(holder));
    let mut seen: Vec<&str> = Vec::new();
    for f in v.facts {
        for k in ["agent_conversation_id", "from_conversation", "to_conversation"] {
            let x = text(f, k);
            if subagent_of(x, s) {
                if let Some(x) = x {
                    if !seen.contains(&x) {
                        seen.push(x);
                    }
                }
            }
        }
    }
    format!("{}/i{}", s.unwrap_or("null"), seen.len() + 1)
}

fn mv(kind: &str, fields: Value, more: Vec<MoveFact>, label: &str) -> Move {
    Move {
        kind: kind.to_string(),
        fields: fields_of(fields),
        more: Some(more),
        label: Some(label.to_string()),
    }
}

/// The investigator moves for `c`: a spawn at the next rung or at haiku, which clears nothing,
/// and with an investigator open, the transfer back once it is done or `investigator_unfinished`.
pub fn investigator_moves(v: &View, c: &str) -> Vec<Move> {
    let mut out = Vec::new();
    let h = plan::assigned_to(v, c).filter(|h| !h.is_empty());
    let inv = investigator_of(v, c);
    if let Some(h) = h {
        if holds(v, c).iter().any(|x| SENT_FOR.contains(&x.why.as_str())) {
            let id = fresh_subagent(v, h);
            let to = |model: &str, label: &str| {
                let tr = fields_of(json!({ "criterion_id": c, "from_conversation": h, "to_conversation": id }));
                mv("model_seen", json!({ "agent_conversation_id": id, "model": model }), vec![MoveFact { kind: "criterion_transfer".into(), fields: tr }], label)
            };
            if let Some(r) = investigator_rung(v, c) {
                out.push(to(r, "investigator"));
            }
            if inv.is_none() {
                out.push(to("haiku", "not_one_rung_up"));
            }
        }
    }
    if let Some(inv) = inv {
        let red = investigated_for(v, inv) == Some("two_tries_not_red");
        let to = text(inv, "to_conversation");
        if hands_back(v, c, inv) {
            let back = climb_of(v, c).first().and_then(|f| text(f, "from_conversation"));
            out.push(mv("criterion_transfer", json!({ "criterion_id": c, "from_conversation": to, "to_conversation": back }), Vec::new(), "transfer_back"));
        } else {
            let why = if red { "no_seal" } else { "no_green" };
            out.push(mv("investigator_unfinished", json!({ "criterion_id": c, "conversation": to, "why": why }), Vec::new(), "investigator_unfinished"));
        }
    }
    out
}

/// Whether the open investigator `inv` did what it was sent for: a laptop seal for
/// `two_tries_not_red`, green or proven for `two_failures`.
pub fn hands_back(v: &View, c: &str, inv: &Fact) -> bool {
    if investigated_for(v, inv) == Some("two_tries_not_red") {
        crate::criterion::sealed_at(v, c, crate::criterion::Place::Laptop).is_some()
    } else {
        matches!(state(v, c), "green" | "proven")
    }
}

/// Whether a move meets a clear: one of its facts carries every field of the clear, and for a
/// clear with `rung`, a `model_seen` for the transfer's `to_conversation` is at that rung.
pub fn clear_matches(m: &Move, cl: &Map<String, Value>) -> bool {
    let fs = m.facts();
    let kind = cl.get("kind").and_then(Value::as_str);
    let rung = cl.get("rung").and_then(Value::as_str);
    fs.iter().any(|(k, f)| {
        Some(*k) == kind
            && cl.iter().all(|(key, val)| key == "kind" || key == "rung" || f.get(key) == Some(val))
            && rung.is_none_or(|r| {
                fs.iter().any(|(gk, g)| {
                    *gk == "model_seen"
                        && g.get("agent_conversation_id") == f.get("to_conversation")
                        && family_of(g.get("model").and_then(Value::as_str)) == Some(r)
                })
            })
    })
}
