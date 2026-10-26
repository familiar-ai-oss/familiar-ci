//! c_dep: the dependency flow (plan §7.1, §7.2; lane S9a). The re-earn rule reads the facts,
//! never a commit message:
//!
//! - t1: a `test_log_translation` labelled `crashed` for c at its tests-first commit T;
//! - t2: the person's `hold_answered {c, crashed, a_dependency}`, after t1 (the approval);
//! - t3: the binary's `dependency_drafted {approval: t2}` naming the new criterion c_dep, after t2;
//! - t4: the first laptop stamp of c_dep after t3 at which c_dep is green on the log up to it, at G;
//! - B: the first `commit_seen` after G's that touches no tests, no code and no anchor.
//!
//! The re-earn commit is B when B's `commit_seen` came before t4, else G: fixed at t4 and never
//! re-evaluated (D22). c's tests-first moves there when c_dep is green (1), c's tests there are
//! its proof (2), and no commit between T and it touched c's anchor (3). The rule is keyed on the
//! approval, once each: a crash at the re-earn commit is a new card, answered by a new approval.

use familiar_ai_facts::Fact;
use serde_json::{Map, Value};

use crate::criterion::{mine, proof, tests_first_commit, tests_hash_at, Place, TestsFirst};
use crate::outcome::{is, Family};
use crate::plan;
use crate::reader::{flag, is_of, text, texts, View};

/// The hold whose card offers `a_dependency` (plan §7, Decided: `crashed` is an outcome).
pub const CRASHED: &str = "crashed";
/// The person's choice that asks for a dependency.
pub const A_DEPENDENCY: &str = "a_dependency";

/// One approval's re-earn: the approval's `t`, the dependency it drafted, and the re-earn
/// commit (an id, its content key, where that content first landed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReEarn {
    pub approval: u64,
    pub dependency: String,
    pub commit: String,
    pub key: String,
    pub at: i64,
}

impl ReEarn {
    pub fn tests_first(&self, v: &View) -> TestsFirst {
        TestsFirst { key: self.key.clone(), at: self.at, ids: v.idx().ids.get(&self.key).cloned().unwrap_or_default() }
    }
}

/// Whether `a` is the person's `a_dependency` on a card that offers it: `crashed`,
/// `command_broke`, `two_failures` or a `two_<outcome>` alike (S9a's two, the owner, 2026-10-16).
pub fn is_approval(a: &Fact) -> bool {
    a.kind == "hold_answered"
        && text(a, "choice") == Some(A_DEPENDENCY)
        && text(a, "hold").and_then(crate::criterion::answers_of).is_some_and(|cs| cs.contains(&A_DEPENDENCY))
}

/// The person's `a_dependency` answers on c's cards since its epoch that have not ended, oldest
/// first.
pub fn approvals<'a>(v: &View<'a>, c: &str) -> Vec<&'a Fact> {
    mine(v, c, "hold_answered").into_iter().filter(|a| is_approval(a) && !ended(v, a)).collect()
}

/// The misses that end an approval (plan §7.3, as the judge's `TWICE`).
pub const TWICE: usize = 2;

/// The binary's `dependency_unanswered` for the approval at `t`, oldest first.
pub fn misses<'a>(v: &View<'a>, t: u64) -> Vec<&'a Fact> {
    v.kind("dependency_unanswered").filter(|f| f.fields.get("approval").and_then(Value::as_u64) == Some(t)).collect()
}

/// An approval the binary missed twice ended (plan §7.3): it answers nothing, so its card opens
/// again, and no dependency is drafted or re-earned for it.
pub fn ended(v: &View, a: &Fact) -> bool {
    is_approval(a) && misses(v, a.t).len() >= TWICE
}

/// Whether the dependency drafted for the approval at `t` has its lines: its criterion is planned.
pub fn landed(v: &View, t: u64) -> bool {
    draft_of(v, t).and_then(|d| d.criterion_id()).is_some_and(|d| v.of("criterion", d).any(|f| f.t > t))
}

/// The prefix of a `dependency_unanswered.why` the binary writes when the drafted lines left the
/// plan incomplete: the environment_error card's reading (plan §7.3, step 6).
pub const PLAN_INCOMPLETE: &str = "the plan with the dependency is incomplete";

/// Whether the binary owes the lines of the draft for the approval `a` (plan §7.3, the backstop):
/// drafted, not landed, not ended, and, after a refused plan, only once the person answered the
/// environment_error card `try_again` (S9c review 4: re_plan and release re-run nothing).
pub fn lines_owed(v: &View, a: &Fact) -> bool {
    let Some(c) = a.criterion_id() else { return false };
    if draft_of(v, a.t).is_none() || landed(v, a.t) || ended(v, a) {
        return false;
    }
    match misses(v, a.t).into_iter().last() {
        None => true,
        Some(m) => v.of("hold_answered", c).any(|f| f.t > m.t && text(f, "hold") == Some("environment_error") && text(f, "choice") == Some("try_again")),
    }
}

/// The approval of `c` whose draft's lines failed the plan, with no answer to the
/// environment_error card since: that card stands.
pub fn plan_refused<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    let a = approvals(v, c).into_iter().last()?;
    let miss = misses(v, a.t).into_iter().last()?;
    let refused = text(miss, "why").is_some_and(|w| w.starts_with(PLAN_INCOMPLETE)) && !landed(v, a.t);
    let answered = v.of("hold_answered", c).any(|f| f.t > miss.t && text(f, "hold") == Some("environment_error"));
    (refused && !answered).then_some(miss)
}

/// The `dependency_drafted` that answers the approval at `t`.
pub fn draft_of<'a>(v: &View<'a>, t: u64) -> Option<&'a Fact> {
    v.kind("dependency_drafted").find(|f| f.fields.get("approval").and_then(Value::as_u64) == Some(t))
}

/// t4: the first laptop stamp of `d` after `after` at which `d` is green on the log up to it.
/// Green is read on the raw seal, so the reading never turns back to the part that waits.
pub fn green_stamp<'a>(v: &View<'a>, d: &str, after: u64) -> Option<&'a Fact> {
    v.of("fingerprint_stamped", d)
        .filter(|s| s.t > after && text(s, "ran_in") == Some(Place::Laptop.as_str()))
        .find(|s| crate::criterion::green_raw(&v.prefix(s.t as usize + 1), d, Place::Laptop))
}

/// The `commit_seen` of `id` latest before `before`.
fn seen_before<'a>(v: &View<'a>, id: &str, before: u64) -> Option<&'a Fact> {
    v.kind("commit_seen").filter(|f| f.t < before && text(f, "commit") == Some(id)).last()
}

/// B's shape: a commit that touches no tests, no code and no anchor.
pub fn empty(f: &Fact) -> bool {
    !flag(f, "touches_tests") && !flag(f, "touches_code") && texts(f, "anchor_overlap").is_empty()
}

/// The re-earn commit of one approval, when the rule's conditions hold.
fn re_earn_of(v: &View, c: &str, a: &Fact) -> Option<ReEarn> {
    let t2 = a.t;
    // T: c's tests-first commit as it stood when the person answered
    let base = tests_first_commit(&v.prefix(t2 as usize), c)?;
    let ix = v.idx();
    // t1 < t2: on the crashed card, the run at T crashed; the other cards' holds are their own t1
    let crashed = v.of("test_log_translation", c).any(|t| {
        t.t < t2 && text(t, "label") == Some(CRASHED) && text(t, "commit").and_then(|id| ix.key_of(id)) == Some(base.key.as_str())
    });
    if text(a, "hold") == Some(CRASHED) && !crashed {
        return None;
    }
    // t3 > t2, naming a criterion first planned by the batch
    let d = draft_of(v, t2).filter(|d| d.t > t2)?;
    let dep = d.criterion_id()?;
    if dep == c || v.of("criterion", dep).any(|f| f.t < t2) {
        return None;
    }
    // (1) t4: c_dep green at G
    let g = green_stamp(v, dep, d.t)?;
    let t4 = g.t;
    let g_seen = seen_before(v, text(g, "commit")?, t4)?;
    // B before t4, else G
    let b = v.kind("commit_seen").find(|f| f.t > g_seen.t && empty(f));
    let r = match b {
        Some(b) if b.t < t4 => b,
        _ => g_seen,
    };
    let id = text(r, "commit")?;
    let key = ix.key_of(id)?.to_string();
    // (2) c's tests there are its proof
    if tests_hash_at(v, c, id).is_none() || tests_hash_at(v, c, id) != proof(v, c) {
        return None;
    }
    // (3) no commit between T and it touched c's anchor
    let touched = v
        .kind("commit_seen")
        .any(|f| (f.t as i64) > base.at && f.t <= r.t && texts(f, "anchor_overlap").contains(&c));
    if touched {
        return None;
    }
    let at = ix.first_at(Some(&key));
    Some(ReEarn { approval: t2, dependency: dep.to_string(), commit: id.to_string(), key, at })
}

/// The re-earn that governs c's tests-first commit: the latest approval whose conditions hold.
pub fn re_earn(v: &View, c: &str) -> Option<ReEarn> {
    approvals(v, c).into_iter().rev().find_map(|a| re_earn_of(v, c, a))
}

/// `miscoped` (plan §6.4, D20): c's run at the re-earn commit is green, so c passed once c_dep
/// was fixed. S7 raises the hold; this is its reading.
pub fn miscoped(v: &View, c: &str) -> bool {
    let Some(r) = re_earn(v, c) else {
        return false;
    };
    let own = crate::criterion::own_tests(v, c);
    !own.is_empty()
        && own.iter().all(|t| {
            is(v, c, crate::criterion::result_at(v, c, Place::Laptop, false, &t.file_path, Some(&r.key)), Family::Green)
        })
}

/// The dependency `c` was, when `c` is miscoped: the re-earn's drafted criterion.
pub fn miscoped_by(v: &View, c: &str) -> Option<String> {
    if !miscoped(v, c) {
        return None;
    }
    re_earn(v, c).map(|r| r.dependency)
}

/// The crash the crashed card reads: the latest run of c translated `crashed` at its tests-first
/// commit with no answer to the card after it. Answered, it is gone; a crash at the re-earn
/// commit is a new one (the loop guard: one re-earn per approval).
pub fn crash_unanswered<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    let tf = tests_first_commit(v, c)?;
    let ix = v.idx();
    let crash = v
        .of("test_log_translation", c)
        .filter(|t| text(t, "label") == Some(CRASHED) && text(t, "commit").and_then(|id| ix.key_of(id)) == Some(tf.key.as_str()))
        .last()?;
    let answered = mine(v, c, "hold_answered").iter().any(|a| a.t > crash.t && text(a, "hold") == Some(CRASHED) && !ended(v, a));
    (!answered).then_some(crash)
}

/// The latest stringing fact of `c`.
fn string_fact<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    v.facts.iter().rev().find(|f| is_of(f, c) && matches!(f.kind, "criterion_head" | "criterion_strung_on_head"))
}

/// The members after `x` on its string, in order.
fn after(v: &View, x: &str) -> Vec<String> {
    let cs = plan::criteria(v);
    let mut out: Vec<String> = Vec::new();
    let mut cur = x.to_string();
    while let Some(n) = cs.iter().find(|y| plan::on_string(v, y) && plan::string_before(v, y).as_deref() == Some(cur.as_str())) {
        if out.contains(n) || n == x {
            break;
        }
        out.push(n.clone());
        cur = n.clone();
    }
    out
}

fn fields(c: &str, head: Option<&str>) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("criterion_id".into(), Value::from(c));
    if let Some(h) = head {
        m.insert("head".into(), Value::from(h));
    }
    m
}

/// The insert (plan §7.2): `d` strung just before `x`, never replacing. Before a head, `d` heads
/// the string and `x` and its members are strung on it; before a member, `d`, `x` and the members
/// after `x` are strung again on the same head, so the latest stringing facts order them.
pub fn restring(v: &View, d: &str, x: &str) -> Vec<(&'static str, Map<String, Value>)> {
    let Some(l) = string_fact(v, x).filter(|_| plan::on_string(v, x)) else {
        return Vec::new();
    };
    let rest = after(v, x);
    let (mut out, head) = if l.kind == "criterion_head" {
        (vec![("criterion_head", fields(d, None))], d.to_string())
    } else {
        let h = text(l, "head").unwrap_or("").to_string();
        (vec![("criterion_strung_on_head", fields(d, Some(&h)))], h)
    };
    out.push(("criterion_strung_on_head", fields(x, Some(&head))));
    for m in rest {
        out.push(("criterion_strung_on_head", fields(&m, Some(&head))));
    }
    out
}

/// The insert check: a restring never leaves a criterion off the string. Every criterion on the
/// string before is on it after, and so is every criterion the facts after `before` string.
pub fn keeps_string(before: &View, after: &View) -> bool {
    let n = before.facts.len() as u64;
    let kept = plan::criteria(before).iter().filter(|c| plan::on_string(before, c)).all(|c| plan::on_string(after, c));
    let strung = after
        .facts
        .iter()
        .filter(|f| f.t >= n && matches!(f.kind, "criterion_head" | "criterion_strung_on_head"))
        .filter_map(|f| f.criterion_id())
        .all(|c| plan::on_string(after, c));
    kept && strung
}
