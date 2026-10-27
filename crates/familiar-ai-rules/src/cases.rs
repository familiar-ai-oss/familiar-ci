//! The cases card (the owner's plan of 2026-10-25) [draft names]. Once a full-path part's contract
//! is whole, the case-finder is owed: the agent releases the subagent, and the binary records its
//! answer as `FOUND` (found, none or failed). Then the person's card `CARD` shows the planned cases
//! and the found ones. `ADD_ALL` makes every found case one the promise must hold; `KEEP` keeps
//! the plan as it is; `AGAIN` owes one more finder (`FINDERS_MAX` per part); words typed are
//! numbers that pick found cases, or else the person's own case (`p1`, `p2`, …). After the last
//! look the card offers `KEEP` or `TYPED`, the person's own case (the owner's ruling, 2026-10-25:
//! "Keep, or add my own"); `TYPED` with nothing typed is no answer.
//!
//! Everything is read from the part's intent on, so a promise claimed again to add the cases owes
//! no new finder. The test waits on all of it: the plan's line 3 holds only when no finder is owed,
//! no card is open and every case asked for is in the promise.
//!
//! The lens panel replaces the case-finder (the owner's plan of 2026-10-26, lens-panel.md, approved
//! with its names; the holds and the card's choices kept as they are). A panel is owed in place of
//! the finder: first the five `LENSES`, each until its `LENS_FOUND` since the panel began (a
//! `RELEASED` lens with no answer is still owed), then the `SYNTHESIS` until a `FOUND` after the
//! five; a `FOUND` before them closes nothing. `AGAIN` re-runs the whole panel, at most
//! `FINDERS_MAX` panels per part. The synthesis may ask up to three questions; the card is answered
//! when its choice and every question has an answer (`QUESTION_ANSWERED`, the person's pick: one of
//! the question's options, or words typed).

use familiar_ai_facts::{Fact, FoundQuestion};
use serde_json::Value;

use crate::reader::{t_of, text, View};

/// The agent's hold while the case-finder is owed [draft name].
pub const OWED: &str = "cases_owed";
/// The person's card on the found cases [draft name].
pub const CARD: &str = "cases_unconfirmed";
/// The binary's record of the synthesis's answer (before the lens panel, the case-finder's).
pub const FOUND: &str = "cases_found";
/// The lens panel's five lenses, by id, in the order they are released (the owner's plan of
/// 2026-10-26, approved).
pub const LENSES: [&str; 5] = ["ambiguity", "completeness", "happy_path", "edge_cases", "order_and_coupling"];
/// The panel's merge, after the five lenses.
pub const SYNTHESIS: &str = "synthesis";
/// The binary's record of a lens's answer.
pub const LENS_FOUND: &str = "lens_found";
/// The binary's record of which Agent call got which lens (or the synthesis).
pub const RELEASED: &str = "lens_released";
/// The person's pick on one of the synthesis's questions.
pub const QUESTION_ANSWERED: &str = "question_answered";
/// The card's choices [draft names].
pub const ADD_ALL: &str = "add_all";
pub const KEEP: &str = "keep_planned";
pub const AGAIN: &str = "look_again";
/// The choice the picker records for words typed.
pub const TYPED: &str = "typed";
/// The most panels one part gets (the first and one "look again"); before the lens panel, the
/// most case-finders.
pub const FINDERS_MAX: usize = 2;
/// The prefix of the person's own cases' ids.
pub const PERSONS: &str = "p";

/// The part's intent's time, from which its findings and answers are read.
fn since(v: &View, c: &str) -> Option<i64> {
    crate::ask::intent_of(v, c).map(t_of)
}

fn after(f: &Fact, s: Option<i64>) -> bool {
    s.is_none_or(|s| t_of(f) > s)
}

/// One lens panel on a part: when it began (the part's intent, or the person's `AGAIN`), each
/// lens's first answer since then, in `LENSES` order, and its synthesis.
struct Panel<'a> {
    start: Option<i64>,
    lenses: Vec<&'a Fact>,
    synthesis: Option<&'a Fact>,
}

/// The panels on `c` since its intent, oldest first: each begins at the intent or at the person's
/// `AGAIN`, the first answer on the card after the previous panel's synthesis; its synthesis is
/// the first `FOUND` after all five lenses answered. At most `FINDERS_MAX`.
fn panels<'a>(v: &View<'a>, c: &str) -> Vec<Panel<'a>> {
    let found: Vec<&Fact> = v.of(LENS_FOUND, c).collect();
    let syntheses: Vec<&Fact> = v.of(FOUND, c).collect();
    let answers = answers(v, c);
    let mut start = since(v, c);
    let mut out = Vec::new();
    loop {
        let lenses: Vec<&Fact> = LENSES.iter().filter_map(|l| found.iter().find(|f| after(f, start) && text(f, "lens") == Some(l)).copied()).collect();
        let done = (lenses.len() == LENSES.len()).then(|| lenses.iter().map(|f| t_of(f)).max()).flatten();
        let synthesis = done.and_then(|d| syntheses.iter().find(|f| t_of(f) > d).copied());
        out.push(Panel { start, lenses, synthesis });
        let Some(s) = synthesis else { break };
        if out.len() >= FINDERS_MAX {
            break;
        }
        match answers.iter().find(|a| t_of(a) > t_of(s)) {
            Some(a) if text(a, "choice") == Some(AGAIN) => start = Some(t_of(a)),
            _ => break,
        }
    }
    out
}

/// The panel on `c` still owed, when one is: the card applies and the latest panel has no synthesis.
fn owed_panel<'a>(v: &View<'a>, c: &str) -> Option<Panel<'a>> {
    if !applies(v, c) {
        return None;
    }
    panels(v, c).pop().filter(|p| p.synthesis.is_none())
}

/// The syntheses' findings on `c` since its intent, oldest first, one per panel (before the lens
/// panel, the case-finder's).
pub fn findings<'a>(v: &View<'a>, c: &str) -> Vec<&'a Fact> {
    panels(v, c).into_iter().filter_map(|p| p.synthesis).collect()
}

/// The lenses the owed panel on `c` still waits for, in `LENSES` order; none when no panel is owed.
pub fn lenses_owed(v: &View, c: &str) -> Vec<&'static str> {
    let Some(p) = owed_panel(v, c) else {
        return Vec::new();
    };
    LENSES.iter().copied().filter(|l| !p.lenses.iter().any(|f| text(f, "lens") == Some(l))).collect()
}

/// What the owed panel on `c` waits for next: the first lens with no answer, else `SYNTHESIS`;
/// `None` when no panel is owed.
pub fn owed_next(v: &View, c: &str) -> Option<&'static str> {
    owed_panel(v, c)?;
    Some(lenses_owed(v, c).first().copied().unwrap_or(SYNTHESIS))
}

/// What the next Agent call on `c` is released to: the first owed lens not released since the
/// panel began, else, once the five answered, `SYNTHESIS` when it was not released since; `None`
/// when no panel is owed or every owed one is out, waiting for its answer.
pub fn to_release(v: &View, c: &str) -> Option<&'static str> {
    let p = owed_panel(v, c)?;
    let out = |l: &str| v.of(RELEASED, c).any(|r| after(r, p.start) && text(r, "lens") == Some(l));
    let owed = lenses_owed(v, c);
    if owed.is_empty() {
        return (!out(SYNTHESIS)).then_some(SYNTHESIS);
    }
    owed.into_iter().find(|l| !out(l))
}

/// The lenses' answers of the latest panel on `c` (the owed one, or the one its latest synthesis
/// closed), one per lens in `LENSES` order: what the synthesis reads.
pub fn lens_findings<'a>(v: &View<'a>, c: &str) -> Vec<&'a Fact> {
    panels(v, c).pop().map(|p| p.lenses).unwrap_or_default()
}

/// The questions of the latest synthesis on `c`, in its order.
pub fn questions(v: &View, c: &str) -> Vec<FoundQuestion> {
    findings(v, c).last().and_then(|f| f.fields.get("questions")).and_then(|q| serde_json::from_value(q.clone()).ok()).unwrap_or_default()
}

/// The person's pick on the latest synthesis's question `q` on `c`, the latest since that
/// synthesis: one of the question's options, or `TYPED` with words typed; any other is no answer.
pub fn question_answer<'a>(v: &View<'a>, c: &str, q: &str) -> Option<&'a Fact> {
    let s = *findings(v, c).last()?;
    let question = questions(v, c).into_iter().find(|x| x.question_id == q)?;
    v.of(QUESTION_ANSWERED, c)
        .filter(|a| t_of(a) > t_of(s) && text(a, "question_id") == Some(q))
        .filter(|a| match text(a, "choice") {
            Some(TYPED) => text(a, "typed").is_some_and(|w| !w.trim().is_empty()),
            Some(ch) => question.options.iter().any(|o| o == ch),
            None => false,
        })
        .last()
}

/// The ids of the latest synthesis's questions on `c` with no answer yet, in its order.
pub fn unanswered_questions(v: &View, c: &str) -> Vec<String> {
    questions(v, c).into_iter().map(|q| q.question_id).filter(|q| question_answer(v, c, q).is_none()).collect()
}

/// Whether the person answered the card's choice on `c` since its latest synthesis.
pub fn choice_answered(v: &View, c: &str) -> bool {
    findings(v, c).last().is_some_and(|f| answers(v, c).iter().any(|a| t_of(a) > t_of(f)))
}

/// Whether `a` is `TYPED` with nothing typed [draft]: no answer, so the card stays open rather than
/// close with nothing asked for.
fn nothing_typed(a: &Fact) -> bool {
    text(a, "choice") == Some(TYPED) && text(a, "typed").is_none_or(|w| w.trim().is_empty())
}

/// The person's answers on `c`'s card since its intent, oldest first; `TYPED` with nothing typed
/// is none.
fn answers<'a>(v: &View<'a>, c: &str) -> Vec<&'a Fact> {
    let s = since(v, c);
    v.of("hold_answered", c).filter(|a| text(a, "hold") == Some(CARD) && after(a, s) && !nothing_typed(a)).collect()
}

/// The part `c` was drafted from, when the binary drafted `c` as a dependency on the person's
/// approval (door/dependency.rs): the part of the approval its `dependency_drafted` answers.
fn source_part<'a>(v: &View<'a>, c: &str) -> Option<&'a str> {
    let t = v.of("dependency_drafted", c).find_map(|d| d.fields.get("approval").and_then(Value::as_u64))?;
    v.at(usize::try_from(t).ok()?).and_then(Fact::criterion_id)
}

/// Whether the card applies to `c`: a full-path part whose latest promise carries a whole contract,
/// not released, its tests-first commit not taken yet. A dependency the binary drafted stands on the
/// finding and card answer of its source part (the owner, 2026-10-25): it owes no case-finder and
/// opens no card of its own.
fn applies(v: &View, c: &str) -> bool {
    crate::plan::path_of(v, c) == Some("full")
        && source_part(v, c).is_none()
        && crate::plan::promise_of(v, c).is_some_and(|p| p.fields.contains_key("contract_version") && crate::plan::contract_wants(p).is_none())
        && !crate::lifecycle::released(v, c)
        && v.of("test_written", c).next().is_none()
}

/// Whether the lens panel is owed on `c`: none closed since its intent, or the person's first
/// answer after the latest synthesis is `AGAIN` and fewer than `FINDERS_MAX` panels ran, and that
/// panel's synthesis is not recorded yet.
pub fn owed(v: &View, c: &str) -> bool {
    owed_panel(v, c).is_some()
}

/// Whether the person's card is open on `c`: a synthesis stands whose choice or one of whose
/// questions has no answer yet.
pub fn card_open(v: &View, c: &str) -> bool {
    applies(v, c) && !owed(v, c) && !findings(v, c).is_empty() && (!choice_answered(v, c) || !unanswered_questions(v, c).is_empty())
}

/// Whether `c` waits on the case-finder or the person's card.
pub fn holds(v: &View, c: &str) -> bool {
    owed(v, c) || card_open(v, c)
}

/// The card's choices on `c`'s latest finding: `ADD_ALL` and `KEEP` when it found cases, else
/// `KEEP` and, while another finder may run, `AGAIN`, then `TYPED`, the person's own case (the
/// owner's ruling, 2026-10-25: "Keep, or add my own"), so the card always offers two.
pub fn offered(v: &View, c: &str) -> Vec<&'static str> {
    let fs = findings(v, c);
    let Some(last) = fs.last() else {
        return Vec::new();
    };
    if text(last, "outcome") == Some("found") && !found_ids(last).is_empty() {
        return vec![ADD_ALL, KEEP];
    }
    if fs.len() < FINDERS_MAX {
        vec![KEEP, AGAIN]
    } else {
        vec![KEEP, TYPED]
    }
}

/// The ids of a finding's cases, in its order.
pub fn found_ids(f: &Fact) -> Vec<String> {
    f.fields
        .get("cases")
        .and_then(Value::as_array)
        .map(|cs| cs.iter().filter_map(|x| x.get("case_id").and_then(Value::as_str).map(str::to_string)).collect())
        .unwrap_or_default()
}

/// The numbers words typed name, when they are only numbers (and "and", commas, spaces).
fn numbers(words: &str) -> Option<Vec<usize>> {
    let tokens: Vec<&str> = words.split(|ch: char| !ch.is_alphanumeric()).filter(|t| !t.is_empty()).collect();
    let nums: Vec<usize> = tokens.iter().filter_map(|t| t.parse().ok()).collect();
    let only = tokens.iter().all(|t| t.parse::<usize>().is_ok() || t.eq_ignore_ascii_case("and"));
    (only && !nums.is_empty()).then_some(nums)
}

/// The cases the person asked for on `c`, in their order: every found case after `ADD_ALL`, the
/// found cases words typed number, or the person's own case for any other words.
pub fn required(v: &View, c: &str) -> Vec<String> {
    let fs = findings(v, c);
    let mut out: Vec<String> = Vec::new();
    let mut own = 0;
    for a in answers(v, c) {
        let Some(f) = fs.iter().rev().find(|f| t_of(f) < t_of(a)) else {
            continue;
        };
        let ids = found_ids(f);
        let picked: Vec<String> = match text(a, "choice") {
            Some(ADD_ALL) => ids,
            Some(TYPED) => {
                let words = text(a, "typed").unwrap_or("").trim();
                match numbers(words) {
                    Some(ns) => ns.iter().filter_map(|n| n.checked_sub(1).and_then(|i| ids.get(i)).cloned()).collect(),
                    None if !words.is_empty() => {
                        own += 1;
                        vec![format!("{PERSONS}{own}")]
                    }
                    None => Vec::new(),
                }
            }
            _ => Vec::new(),
        };
        for id in picked {
            if !out.contains(&id) {
                out.push(id);
            }
        }
    }
    out
}

/// The words of the person's own case `id` on `c`, as they typed them.
pub fn persons_case(v: &View, c: &str, id: &str) -> Option<String> {
    let n: usize = id.strip_prefix(PERSONS)?.parse().ok()?;
    answers(v, c)
        .into_iter()
        .filter(|a| text(a, "choice") == Some(TYPED))
        .filter_map(|a| text(a, "typed").map(str::trim).filter(|w| !w.is_empty() && numbers(w).is_none()).map(str::to_string))
        .nth(n.checked_sub(1)?)
}

/// The cases the person asked for that `c`'s latest promise does not hold yet.
pub fn missing(v: &View, c: &str) -> Vec<String> {
    let have: Vec<String> = crate::plan::promise_of(v, c).map(|p| crate::plan::cases_in(p).into_iter().map(|k| k.case_id).collect()).unwrap_or_default();
    required(v, c).into_iter().filter(|id| !have.contains(id)).collect()
}
