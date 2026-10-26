//! CI's recheck of a pull request head, as derived state.
//! Results and stamps are read only from facts written with `ran_in: pipeline`; the laptop's
//! stamps are read only for the proof the plan claims.

use familiar_ai_facts::{append_raw, Fact, Log};
use serde_json::{json, Map, Value};

use crate::criterion::{Test, PLACES};
use crate::lifecycle::holds;
use crate::moves::criterion_admissible;
use crate::plan::EXIT_KINDS;
use crate::reader::{content_key, flag, is_of, t_of, text, texts, View, INF};
use crate::sample::Move;

/// The states of the check on a pull request head, of which exactly one holds.
pub const CHECK_ORDER: [&str; 3] = ["not_run", "passed", "failed"];

/// A commit's content key by id, or `?<id>` when the id is unknown.
pub fn content(v: &View, id: &str) -> String {
    v.kind("commit_seen")
        .find(|f| text(f, "commit") == Some(id))
        .map(|f| content_key(v, f))
        .unwrap_or_else(|| format!("?{id}"))
}

/// Whether two ids name the same commit or the same content, since a rebase renames commits.
pub fn same(v: &View, x: &str, y: &str) -> bool {
    x == y || content(v, x) == content(v, y)
}

/// The pull request's head: the latest `commit_seen`.
pub fn head<'a>(v: &View<'a>) -> Option<&'a str> {
    v.last("commit_seen").and_then(|c| text(c, "commit"))
}

/// Whether a stack or a `test_planned` fact supplies a non-empty `test_command`.
fn has_command(f: &Fact) -> bool {
    (f.kind == "test_stack_detected" || f.kind == "test_planned") && crate::plan::has_command(f)
}

/// Whether a fact is a stack with a command; a root's command is the stack's alone, so a
/// criterion never reads a sibling's `test_planned` command.
fn stack_command(f: &Fact) -> bool {
    f.kind == "test_stack_detected" && crate::plan::has_command(f)
}

/// The latest stack claim for the criterion's root; new work with no integration point takes any root.
fn claim<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    let a = v
        .facts
        .iter()
        .rev()
        .find(|f| EXIT_KINDS.contains(&f.kind) && is_of(f, c))?;
    let fps: Option<Vec<String>> = crate::plan::exit_files(a);
    v.facts.iter().rev().find(|f| {
        stack_command(f)
            && fps
                .as_ref()
                .is_none_or(|fps| fps.iter().all(|fp| fp.starts_with(crate::plan::root_of(f))))
    })
}

/// The run started on the head.
/// A run uses the `test_command` in force when it starts, so a command announced later voids it.
pub fn run<'a>(v: &View<'a>) -> Option<&'a Fact> {
    let h = head(v);
    let r = v
        .facts
        .iter()
        .rev()
        .find(|f| f.kind == "pipeline_run_seen" && text(f, "head_commit") == h)?;
    if v.facts.iter().any(|f| has_command(f) && f.t > r.t) {
        return None;
    }
    Some(r)
}

/// The command the action runs the criterion's tests with: its own, else derived from the
/// claimed stack, else the stack's. It must match the command the laptop runs.
pub fn command(v: &View, c: &str) -> Option<String> {
    crate::criterion::own_command(v, c)
        .map(str::to_string)
        .or_else(|| {
            let st = claim(v, c)?;
            let mut files: Vec<String> = Vec::new();
            for f in test_files(v, c) {
                if !files.contains(&f) {
                    files.push(f);
                }
            }
            crate::criterion::derive_from(st, &files)
        })
        .or_else(|| claim(v, c).and_then(|f| text(f, "test_command")).map(str::to_string))
}

/// The current run's position, which every pipeline read must come after; `INF` when no run.
fn run_t(v: &View) -> i64 {
    run(v).map(t_of).unwrap_or(INF)
}

/// The criterion ids, in plan order.
pub fn crits(v: &View) -> Vec<String> {
    crate::plan::criteria(v)
}

/// The planned test files, distinct.
pub fn test_files(v: &View, c: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for f in v.of("test_planned", c) {
        if let Some(fp) = text(f, "file_path") {
            if !out.iter().any(|x| x == fp) {
                out.push(fp.to_string());
            }
        }
    }
    out
}

/// The tests actually written, distinct by file.
pub fn tests(v: &View, c: &str) -> Vec<Test> {
    crate::criterion::own_tests(v, c)
}

/// `c`'s own tests fingerprint as observed at the latest landing of that content, falling back
/// to a legacy log's unverified per-commit hash.
pub fn tests_hash_at<'a>(v: &View<'a>, c: &str, commit: &str) -> Option<&'a str> {
    let at: Vec<&Fact> = v.kind("commit_seen").collect();
    let me = at.iter().rev().find(|f| text(f, "commit").is_some_and(|id| same(v, id, commit)))?;
    if let Some(o) = text(me, "commit").and_then(|id| crate::criterion::observed(v, c, id)) {
        return text(o, "tests_hash");
    }
    let t0 = me.t;
    let files = test_files(v, c);
    let ok = at
        .iter()
        .rev()
        .find(|f| f.t <= t0 && texts(f, "files").iter().any(|p| files.iter().any(|x| x == p)))?;
    text(ok, "tests_hash")
}

/// The laptop's stamps of `c` since its last anchor exit and before `before`.
/// The pipeline reads the way back from these stamps and the announcements, never from a laptop
/// result, because verify puts the plan's claims before the commits.
fn plan_stamps<'a>(v: &View<'a>, c: &str, before: i64) -> Vec<&'a Fact> {
    let e = v
        .facts
        .iter()
        .rev()
        .find(|f| EXIT_KINDS.contains(&f.kind) && is_of(f, c))
        .map_or(-1, t_of);
    v.facts
        .iter()
        .filter(|f| {
            f.kind == "fingerprint_stamped" && text(f, "ran_in") == Some("laptop") && is_of(f, c) && t_of(f) > e && t_of(f) < before
        })
        .collect()
}

/// The first tests-only commit of `c`'s tests whose tests hash is `h`.
fn first_tests_only<'a>(v: &View<'a>, c: &str, h: &str) -> Option<&'a str> {
    let files = test_files(v, c);
    let f = v.kind("commit_seen").find(|f| {
        flag(f, "touches_tests")
            && !flag(f, "touches_code")
            && texts(f, "files").iter().any(|p| files.iter().any(|x| x == p))
            && text(f, "commit").is_some_and(|id| tests_hash_at(v, c, id) == Some(h))
    })?;
    text(f, "commit")
}

fn seal_hash_before<'a>(v: &View<'a>, c: &str, before: i64) -> Option<&'a str> {
    let ss = plan_stamps(v, c, before);
    let p = text(ss.last()?, "proof_not_tampered");
    let mut i = ss.len() - 1;
    while i > 0 && text(ss[i - 1], "proof_not_tampered") == p {
        i -= 1;
    }
    if i == 0 {
        return p;
    }
    let p0 = text(ss[i - 1], "proof_not_tampered");
    let Some(a) = v
        .facts
        .iter()
        .find(|f| f.kind == "test_change_announced" && is_of(f, c) && f.t > ss[i - 1].t && f.t < ss[i].t)
    else {
        return p; // No announcement between them: an unannounced change.
    };
    // Green is read from stamps alone, since a rebase renames commits: the laptop stamped the
    // proof then in force a second time, a run past its seal.
    let green = ss.iter().filter(|s| s.t < a.t && text(s, "proof_not_tampered") == p0).count() >= 2;
    if !green {
        return p; // Announced before green: a new seal must go red.
    }
    seal_hash_before(v, c, t_of(a)) // The seal as it stood before the announcement.
}

/// The proof the laptop claims now: its latest stamp's `proof_not_tampered`.
pub fn proof<'a>(v: &View<'a>, c: &str) -> Option<&'a str> {
    plan_stamps(v, c, INF).last().and_then(|s| text(s, "proof_not_tampered"))
}

/// The proof the red seal was stamped under, which is the latest proof when there is no way back.
pub fn seal_hash<'a>(v: &View<'a>, c: &str) -> Option<&'a str> {
    if second_seal(v, c).is_some() {
        return proof(v, c);
    }
    seal_hash_before(v, c, INF)
}

/// The way back: the first commit after the red seal's tests-first commit whose tests hash is
/// the proof claimed now; none once a second seal spends it.
pub fn mark<'a>(v: &View<'a>, c: &str) -> Option<&'a str> {
    if second_seal(v, c).is_some() {
        return None;
    }
    way_back(v, c)
}

/// The second seal: after the way back's green, the latest laptop stamp of the proof claimed now
/// at a tests-only commit of `c`'s tests landed after the way back's commit. The plan carries the
/// stamp, never the laptop's result, so CI reads the re-stamp as the seal and proves the part from
/// it, red there; the way back is spent (the owner, 2026-10-13: "k6 is a second red seal, that
/// requires green"). See docs/decisions.md#the-way-back
pub fn second_seal<'a>(v: &View<'a>, c: &str) -> Option<&'a str> {
    let m = way_back(v, c)?;
    let p = proof(v, c)?;
    let files = test_files(v, c);
    let cs: Vec<&Fact> = v.kind("commit_seen").collect();
    let im = cs.iter().position(|f| text(f, "commit") == Some(m))?;
    let tests_only = |f: &Fact| {
        flag(f, "touches_tests") && !flag(f, "touches_code") && texts(f, "files").iter().any(|x| files.iter().any(|y| y == x))
    };
    let seen = |id: &str| cs.iter().any(|f| text(f, "commit") == Some(id));
    // CI's own commit after the way back's for the stamp at `id`: the commit itself, or, when a
    // rebase renamed it (its id is nowhere in CI's history), the first tests-only commit there
    // whose tests hash is the stamp's proof. Found by content, it may bind to an earlier commit
    // of the same content (the accepted limit).
    let later = |id: &str| -> Option<&'a str> {
        let at = |f: &&&Fact| tests_only(f) && text(f, "commit").is_some_and(|x| x != m && tests_hash_at(v, c, x) == Some(p));
        if seen(id) {
            cs[im + 1..].iter().filter(|f| text(f, "commit") == Some(id)).find(at).and_then(|f| text(f, "commit"))
        } else {
            cs[im + 1..].iter().find(at).and_then(|f| text(f, "commit"))
        }
    };
    let ids: Vec<&'a str> = plan_stamps(v, c, INF)
        .into_iter()
        .filter(|s| text(s, "proof_not_tampered") == Some(p))
        .filter_map(|s| text(s, "commit"))
        .collect();
    // The first stamp of the proof claimed now is the way back's own; renamed, it is not a re-stamp.
    let skip = usize::from(ids.first().is_some_and(|id| !seen(id)));
    ids[skip..].iter().rev().filter(|id| **id != m).find_map(|id| later(id))
}

/// The way back as the stamps and announcements show it, before any second seal.
fn way_back<'a>(v: &View<'a>, c: &str) -> Option<&'a str> {
    let (p, sh) = (proof(v, c)?, seal_hash_before(v, c, INF)?);
    if p == sh {
        return None;
    }
    let t = first_tests_only(v, c, sh)?;
    let cs: Vec<&Fact> = v.kind("commit_seen").collect();
    let i = cs.iter().position(|f| text(f, "commit") == Some(t))?;
    cs[i + 1..]
        .iter()
        .filter_map(|f| text(f, "commit"))
        .find(|id| tests_hash_at(v, c, id) == Some(p))
}

/// The first tests-only commit whose tests hash is the head's, or after the way back the red
/// seal's, or the second seal once one spends the way back.
pub fn tests_first<'a>(v: &View<'a>, c: &str) -> Option<&'a str> {
    if let Some(s) = second_seal(v, c) {
        return Some(s);
    }
    if mark(v, c).is_some() {
        return first_tests_only(v, c, seal_hash(v, c)?);
    }
    plain_tests_first(v, c).map(|t| re_earned(v, c, t).0)
}

/// The first tests-only commit whose tests hash is the head's, before any re-earn.
fn plain_tests_first<'a>(v: &View<'a>, c: &str) -> Option<&'a str> {
    let h = head(v)?;
    let th = tests_hash_at(v, c, h)?;
    let files = test_files(v, c);
    let f = v.kind("commit_seen").find(|f| {
        flag(f, "touches_tests")
            && !flag(f, "touches_code")
            && texts(f, "files").iter().any(|p| files.iter().any(|x| x == p))
            && text(f, "commit").is_some_and(|id| tests_hash_at(v, c, id) == Some(th))
    })?;
    text(f, "commit")
}

/// c_dep on CI (plan §7.1 "On CI"; lane S9d-ci). For each approval of `a_dependency` on c's
/// crashed card since c's epoch (verify appends only those a witness vouched for, as
/// `hold_answered`), c's tests-first moves to the commit of c's first mirrored laptop stamp after
/// the approval's commit when CI's own run of c at the tests-first commit then in force crashed,
/// c's tests there are its proof (2), no commit from that tests-first commit to it touched c's
/// anchor (3), and another part the laptop stamped there, the dependency, is green there by CI's
/// own runs (1). CI does not recompute "B before t4". Also the run CI must make before it can
/// read the next approval, as (part, commit): c's own at the tests-first commit, or a
/// dependency's at the stamp's commit.
fn re_earned<'a>(v: &View<'a>, c: &str, base: &'a str) -> (&'a str, Option<(String, &'a str)>) {
    let cs: Vec<&Fact> = v.kind("commit_seen").collect();
    let pos = |id: &str| cs.iter().position(|f| text(f, "commit").is_some_and(|x| same(v, x, id)));
    let stamps = plan_stamps(v, c, INF);
    let mut cur = base;
    // every approval S9c reads (a_dependency on crashed, two_<outcome>, command_broke), since the epoch
    for a in crate::dependency::approvals(v, c) {
        // the crash at T, from CI's own run there, on the crashed card (the other cards' holds are their own t1)
        if !ran(v, c, cur) {
            return (cur, Some((c.to_string(), cur)));
        }
        if text(a, "hold") == Some(crate::dependency::CRASHED) && !crashed_at(v, c, cur) {
            continue;
        }
        let Some(at) = text(a, "commit").and_then(pos).or_else(|| cs.iter().rposition(|f| f.t < a.t)) else { continue };
        let Some((pr, r)) = stamps
            .iter()
            .filter_map(|s| text(s, "commit"))
            .filter_map(|id| pos(id).map(|p| (p, id)))
            .filter(|(p, _)| *p > at)
            .min_by_key(|(p, _)| *p)
        else {
            continue;
        };
        // (2) c's tests there are its proof
        if tests_hash_at(v, c, r).is_none() || tests_hash_at(v, c, r) != proof(v, c) {
            continue;
        }
        // (3) no commit from the tests-first commit to it touched c's anchor
        let Some(pb) = pos(cur).filter(|pb| *pb < pr) else { continue };
        if cs[pb + 1..=pr].iter().any(|f| texts(f, "anchor_overlap").contains(&c)) {
            continue;
        }
        // (1) the dependency green there, by CI's own runs
        let mut green = false;
        for d in stamped_at(v, c, r) {
            if tests(v, &d).is_empty() {
                continue;
            }
            if !ran(v, &d, r) {
                return (cur, Some((d, r)));
            }
            green |= tests(v, &d).iter().all(|x| result(v, &d, &x.file_path, r, false) == Some("passed"));
        }
        if green {
            cur = r;
        }
    }
    (cur, None)
}

/// The other parts the laptop stamped at `commit`, in stamp order.
fn stamped_at(v: &View, c: &str, commit: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in v.kind("fingerprint_stamped") {
        let Some(d) = s.criterion_id() else { continue };
        if d != c && text(s, "ran_in") == Some("laptop") && text(s, "commit").is_some_and(|id| same(v, id, commit)) && !out.iter().any(|x| x == d) {
            out.push(d.to_string());
        }
    }
    out
}

/// Whether this run's pipeline has a result for each of `c`'s own tests at `commit`.
fn ran(v: &View, c: &str, commit: &str) -> bool {
    tests(v, c).iter().all(|x| has_result(v, c, &x.file_path, commit, false))
}

/// Whether this run's pipeline run of `c` at `commit` crashed, read from its translation.
fn crashed_at(v: &View, c: &str, commit: &str) -> bool {
    let t0 = run_t(v);
    v.of("pass_please_pass", c).any(|r| {
        t_of(r) > t0
            && text(r, "ran_in") == Some("pipeline")
            && !flag(r, "with_anchor_reverted")
            && text(r, "commit").is_some_and(|id| same(v, id, commit))
            && text(r, "run_id").and_then(|id| crate::outcome::translation_of(v, id)).is_some_and(|t| text(t, "label") == Some(crate::dependency::CRASHED))
    })
}

/// Whether the latest anchor exit is `new_work_no_spot`.
pub fn greenfield(v: &View, c: &str) -> bool {
    v.facts
        .iter()
        .rev()
        .find(|f| EXIT_KINDS.contains(&f.kind) && is_of(f, c))
        .is_some_and(|f| f.kind == "new_work_no_spot")
}

/// The outcome of a test file at a commit, from this run's pipeline results only.
pub fn result<'a>(v: &View<'a>, c: &str, file_path: &str, commit: &str, teeth: bool) -> Option<&'a str> {
    let t0 = run_t(v);
    let r = v.facts.iter().rev().find(|f| {
        f.kind == "pass_please_pass"
            && t_of(f) > t0
            && text(f, "ran_in") == Some("pipeline")
            && is_of(f, c)
            && text(f, "file_path") == Some(file_path)
            && text(f, "commit").is_some_and(|id| same(v, id, commit))
            && flag(f, "with_anchor_reverted") == teeth
    })?;
    // a run that did not run is neither, as the lifecycle reads it (criterion::results)
    if text(r, "outcome") == Some(crate::criterion::DID_NOT_RUN) {
        return Some("other");
    }
    // the family of the result (plan §6.5): a pass is Green, a red a valid one
    let c = r.criterion_id().unwrap_or("");
    match crate::outcome::result_family(v, c, r) {
        crate::outcome::Family::Green => Some("passed"),
        crate::outcome::Family::ValidRed => Some("failed"),
        _ => Some("other"),
    }
}

/// This run's pipeline stamp at a commit.
pub fn stamp<'a>(v: &View<'a>, c: &str, commit: &str) -> Option<&'a Fact> {
    let t0 = run_t(v);
    v.facts.iter().rev().find(|f| {
        f.kind == "fingerprint_stamped"
            && t_of(f) > t0
            && text(f, "ran_in") == Some("pipeline")
            && is_of(f, c)
            && text(f, "commit").is_some_and(|id| same(v, id, commit))
    })
}

/// Whether every test failed at the tests-first commit.
pub fn red(v: &View, c: &str) -> bool {
    let Some(t) = tests_first(v, c) else {
        return false;
    };
    let ts = tests(v, c);
    !ts.is_empty() && ts.iter().all(|x| result(v, c, &x.file_path, t, false) == Some("failed"))
}

/// Whether the stamp at the tests-first commit proves the tests that ran are the sealed ones.
pub fn stamped(v: &View, c: &str) -> bool {
    let Some(t) = tests_first(v, c) else {
        return false;
    };
    stamp(v, c, t).is_some_and(|s| text(s, "proof_not_tampered") == tests_hash_at(v, c, t))
}

/// Whether every test passed at the head under the same proof.
pub fn green_at_head(v: &View, c: &str) -> bool {
    let (Some(h), Some(t)) = (head(v), tests_first(v, c)) else {
        return false;
    };
    let ts = tests(v, c);
    let (Some(sh), Some(st)) = (stamp(v, c, h), stamp(v, c, t)) else {
        return false;
    };
    !ts.is_empty()
        && ts.iter().all(|x| result(v, c, &x.file_path, h, false) == Some("passed"))
        && text(sh, "proof_not_tampered") == if mark(v, c).is_some() { proof(v, c) } else { text(st, "proof_not_tampered") }
}

/// Whether a commit after the tests-first one, up to the head, overlaps the anchor.
pub fn overlap(v: &View, c: &str) -> bool {
    let (Some(h), Some(t)) = (head(v), tests_first(v, c)) else {
        return false;
    };
    let cs: Vec<&Fact> = v.kind("commit_seen").collect();
    let t_t = cs.iter().filter(|f| text(f, "commit") == Some(t)).map(|f| t_of(f)).min();
    let t_h = cs.iter().filter(|f| text(f, "commit") == Some(h)).map(|f| t_of(f)).max();
    let (Some(t_t), Some(t_h)) = (t_t, t_h) else {
        return false;
    };
    let gf = greenfield(v, c);
    cs.iter().any(|f| {
        t_of(f) > t_t
            && t_of(f) <= t_h
            && if gf { flag(f, "touches_code") } else { texts(f, "anchor_overlap").contains(&c) }
    })
}

/// Whether every touching test passed at the head.
pub fn touching_ok(v: &View, c: &str) -> bool {
    let Some(h) = head(v) else {
        return false;
    };
    v.of("touching_test_declared", c)
        .all(|t| result(v, c, text(t, "file_path").unwrap_or(""), h, false) == Some("passed"))
}

/// Whether every test failed at the head with the anchor put back.
/// For new work the integration point, or the spot's file, is what is put back.
pub fn teeth(v: &View, c: &str) -> bool {
    let Some(h) = head(v) else {
        return false;
    };
    let ts = tests(v, c);
    !ts.is_empty() && ts.iter().all(|x| result(v, c, &x.file_path, h, true) == Some("failed"))
}

/// Whether a teeth run passed, which is the `no_teeth` hold.
pub fn no_teeth(v: &View, c: &str) -> bool {
    let Some(h) = head(v) else {
        return false;
    };
    tests(v, c).iter().any(|x| result(v, c, &x.file_path, h, true) == Some("passed"))
}

/// Whether the criterion is released, so it does not block the check.
pub fn released(v: &View, c: &str) -> bool {
    holds(v, c).iter().any(|h| h.why == "released")
}

/// Whether the pipeline proved the criterion on this run.
pub fn proven(v: &View, c: &str) -> bool {
    run(v).is_some()
        && command(v, c).is_some()
        && tests_first(v, c).is_some()
        && red(v, c)
        && stamped(v, c)
        && green_at_head(v, c)
        && overlap(v, c)
        && touching_ok(v, c)
        && teeth(v, c)
}

/// Whether the check is in state `s`: passed when every criterion is proven or released, else
/// failed once a run exists.
pub fn state_is(v: &View, s: &str) -> bool {
    match s {
        "not_run" => run(v).is_none(),
        "passed" => run(v).is_some() && crits(v).iter().all(|c| proven(v, c) || released(v, c)),
        "failed" => run(v).is_some() && !state_is(v, "passed"),
        _ => false,
    }
}

/// The first state in `CHECK_ORDER` that holds.
pub fn check_state(v: &View) -> &'static str {
    CHECK_ORDER.iter().copied().find(|s| state_is(v, s)).unwrap_or("?")
}

/// The labels the walks branch a failed pipeline run on; the binary's own reading may be any
/// label of `LABELS`.
pub const RED_LABELS: [&str; 3] = ["tests_failed", "crashed", "unclassified"];

/// Every label a translation may carry (plan §6.1).
pub const LABELS: [&str; 13] = [
    "passed",
    "tests_failed",
    "missing_target_symbol",
    "collection_error",
    "no_tests_collected",
    "setup_error",
    "usage_error",
    "environment_error",
    "crashed",
    "runner_crash",
    "killed",
    "timed_out",
    "unclassified",
];

/// The words a pipeline run ends with.
pub const END_WORDS: [&str; 3] = ["passed", "failed", "did_not_run"];

/// The fields of a pipeline run's facts that the binary writes from what the run did, beyond
/// the move the rules offer: its wait, its evidence and its reading.
const FREE: [(&str, &[&str]); 3] = [
    ("run_reserved", &["wait"]),
    ("run_ended", &["end", "exit_code", "signal", "stdout_tail", "stderr_tail", "truncated"]),
    ("test_log_translation", &["by", "label", "evidence", "failing_tests", "classified_on", "location", "missing_name", "counts", "ran_tests", "skipped_tests", "versions"]),
];

/// The id of this pipeline run's run of `c`'s command at `commit` (with the anchor put back
/// when `teeth`): one per part, commit and teeth in each run on the head.
pub fn run_id(v: &View, c: &str, commit: &str, teeth: bool) -> String {
    format!("pipeline-{}-{c}-{commit}-{}", run_t(v), if teeth { "teeth" } else { "plain" })
}

/// Whether this run's pipeline has a result for `file_path` of `c` at `commit`.
fn has_result(v: &View, c: &str, fp: &str, commit: &str, teeth: bool) -> bool {
    let t0 = run_t(v);
    v.of("pass_please_pass", c).any(|f| {
        t_of(f) > t0
            && text(f, "ran_in") == Some("pipeline")
            && text(f, "file_path") == Some(fp)
            && text(f, "commit").is_some_and(|id| same(v, id, commit))
            && flag(f, "with_anchor_reverted") == teeth
    })
}

/// The next move of the pipeline's run of `c` at `commit` (plan §6.5): reserved, ended,
/// translated when it ran, then each of `files`' results naming the run. Empty once each file
/// has its result.
fn run_step(v: &View, c: &str, commit: &str, teeth: bool, files: &[String]) -> Vec<Move> {
    if files.iter().all(|fp| has_result(v, c, fp, commit, teeth)) {
        return Vec::new();
    }
    let rid = run_id(v, c, commit, teeth);
    let of = |kind: &str| v.kind(kind).find(|f| text(f, "run_id") == Some(rid.as_str()));
    if of("run_reserved").is_none() {
        let reserved = json!({ "run_id": rid, "criterion_id": c, "criterion_rev": crate::criterion::criterion_rev(v, c), "commit": commit, "command": command(v, c).unwrap_or_default(), "resources": [], "wait": "0", "by": "pipeline" });
        return vec![Move::plain("run_reserved", reserved)];
    }
    let Some(end) = of("run_ended") else {
        return END_WORDS
            .iter()
            .map(|o| Move::plain("run_ended", json!({ "run_id": rid, "outcome": o, "end": if *o == "did_not_run" { "not_started" } else { "exited" } })))
            .collect();
    };
    let word = text(end, "outcome").unwrap_or("failed");
    let result: Vec<Move> = files
        .iter()
        .filter(|fp| !has_result(v, c, fp, commit, teeth))
        .take(1)
        .map(|fp| Move::plain("pass_please_pass", json!({ "criterion_id": c, "file_path": fp, "commit": commit, "outcome": word, "ran_in": "pipeline", "with_anchor_reverted": teeth, "run_id": rid })))
        .collect();
    // A run that ended did_not_run is translated when it ran (a crash, a timeout) and not when it
    // never started; its first result settles which.
    let did_not_run = word == crate::criterion::DID_NOT_RUN;
    let named = v.of("pass_please_pass", c).any(|f| text(f, "run_id") == Some(rid.as_str()));
    if crate::outcome::translation_of(v, &rid).is_none() && !(did_not_run && named) {
        let labels: &[&str] = if word == "passed" { &["passed"] } else { &RED_LABELS };
        let mut out: Vec<Move> = labels
            .iter()
            .map(|l| Move::plain("test_log_translation", json!({ "run_id": rid, "criterion_id": c, "commit": commit, "by": "regex", "label": l, "evidence": "", "failing_tests": [], "classified_on": "full" })))
            .collect();
        if did_not_run {
            out.extend(result);
        }
        return out;
    }
    result
}

/// The moves the action may make next; its steps run in a fixed order and only a test's outcome branches.
pub fn action_admissible(v: &View) -> Vec<Move> {
    let mut out: Vec<Move> = Vec::new();
    let Some(h) = head(v) else {
        return out;
    };
    if run(v).is_none() {
        out.push(Move::plain("pipeline_run_seen", json!({ "head_commit": h }))); // A new push gets a new run.
        return out;
    }
    let stamp_move = |c: &str, commit: &str| -> Vec<Move> {
        vec![Move::plain("fingerprint_stamped", json!({ "criterion_id": c, "commit": commit, "proof_not_tampered": tests_hash_at(v, c, commit), "ran_in": "pipeline" }))]
    };
    let own = |c: &str| -> Vec<String> { tests(v, c).into_iter().map(|x| x.file_path).collect() };
    for c in crits(v) {
        if released(v, &c) {
            continue; // A released criterion is shown, not rechecked.
        }
        if command(v, &c).is_none() {
            continue; // No command for its root, so nothing can run its tests.
        }
        // CI's re-earn reads its own runs: the part's at its tests-first commit, then the dependency's.
        if let Some((x, k)) = plain_tests_first(v, &c).filter(|_| second_seal(v, &c).is_none() && mark(v, &c).is_none()).and_then(|b| re_earned(v, &c, b).1) {
            let s = run_step(v, &x, k, false, &own(&x));
            if !s.is_empty() {
                return s;
            }
        }
        let Some(t) = tests_first(v, &c) else {
            continue;
        };
        let ts = own(&c);
        let touch: Vec<String> = v
            .of("touching_test_declared", &c)
            .filter_map(|f| text(f, "file_path"))
            .map(str::to_string)
            .collect();
        let s = run_step(v, &c, t, false, &ts); // At the tests-first commit the tests must fail.
        if !s.is_empty() {
            return s;
        }
        if !ts.is_empty() && stamp(v, &c, t).is_none() {
            return stamp_move(&c, t);
        }
        if h == t {
            continue;
        }
        let at_head: Vec<String> = ts.iter().chain(touch.iter()).cloned().collect();
        let s = run_step(v, &c, h, false, &at_head); // At the head: its tests and the touching tests.
        if !s.is_empty() {
            return s;
        }
        if !ts.is_empty() && stamp(v, &c, h).is_none() {
            return stamp_move(&c, h);
        }
        let s = run_step(v, &c, h, true, &ts); // The teeth run, new work included.
        if !s.is_empty() {
            return s;
        }
    }
    out
}

/// Whether `fields` is the offered move `e`, the binary's own fields of a pipeline run aside:
/// any label but `passed` for a run that did not pass, any label for one that did.
fn loose_eq(e: &Move, kind: &str, fields: &Map<String, Value>) -> bool {
    if e.kind != kind {
        return false;
    }
    let free: &[&str] = FREE.iter().find(|(k, _)| *k == kind).map_or(&[], |(_, f)| f);
    if kind == "test_log_translation" {
        let got = fields.get("label").and_then(Value::as_str).unwrap_or("");
        let offered = e.fields.get("label").and_then(Value::as_str).unwrap_or("");
        if !LABELS.contains(&got) || (got == "passed" && offered != "passed") {
            return false;
        }
    }
    let kept = |m: &Map<String, Value>| -> Map<String, Value> { m.iter().filter(|(k, _)| !free.contains(&k.as_str())).map(|(k, x)| (k.clone(), x.clone())).collect() };
    kept(&e.fields) == kept(fields)
}

fn move_eq(e: &Move, kind: &str, fields: &Map<String, Value>) -> bool {
    e.kind == kind && &e.fields == fields
}

/// Append a pipeline move, or a single-fact criterion move, and refuse anything else with an error.
pub fn action_append(log: &Log, kind: &str, fields: &Map<String, Value>, at: f64, tenant: &str) -> Result<Log, String> {
    let v = View::new(log);
    let ok = action_admissible(&v).iter().any(|e| move_eq(e, kind, fields) || loose_eq(e, kind, fields))
        || criterion_admissible(&v)
            .iter()
            .any(|e| move_eq(e, kind, fields) && e.more.as_ref().is_none_or(Vec::is_empty));
    if !ok {
        return Err(format!("actionAppend: {kind} is not admissible (check {})", check_state(&v)));
    }
    append_raw(log, kind, fields.clone(), at, tenant).map_err(|e| e.to_string())
}

/// Both places a fact can be written in.
pub fn places() -> [crate::criterion::Place; 2] {
    PLACES
}
