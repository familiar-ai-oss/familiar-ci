//! Every move a criterion can make over the sample world, ported from the
//! reference model (not in this repository). A move appends one fact, or a
//! few that land together, such as a rebase or a re-anchor with its check.

use familiar_ai_facts::{Fact, Log};
use serde_json::{json, Map, Value};

use crate::criterion::{
    greenfield, mine, own_tests, place, results, sealed_at, test_files, tests_first_commit,
    tname, touches_tests_of, touching_tests, Place, Test,
};
use crate::criterion::{criterion_rev, epoch, touches, DID_NOT_RUN};
use crate::lifecycle::{announced, first_act, holds, paused, released, replanned, runnable_criteria, start_check, state, turn};
use crate::plan;
use crate::reader::{last_of, text, View};
use crate::sample::{
    apply_move, fields_of, other_agent, row_of, Move, MoveFact, AGENTS, CODEBASE_ID, INTENT_SHA,
    LINES_CHANGED, LINES_LANDED, LINES_OUTSIDE, LINES_PLANNING, MAPPED_SHIFT, PROJECT_ROOT, TEST_COMMAND, TEST_STACK,
};

/// A second `test_command`, for the walk that changes the command.
pub const OTHER_COMMAND: &str = "pytest -q";
/// The sample's reason for `test_change_announced`.
pub const ANNOUNCE_REASON: &str = "the test asserts the wrong outcome";

/// The sample's reason a listed place is not affected [draft].
pub const PLACE_REASON: &str = "it only passes the value on";

/// The sample's claim that the fix lives outside this repository.
pub const FIX_ELSEWHERE: (&str, &str) = ("dependency", "the library the anchor calls");

/// The holder's two possible claims: the fix is here, or it is elsewhere.
fn claim_moves(c: &str) -> Vec<Move> {
    [("this_repository", ""), FIX_ELSEWHERE]
        .iter()
        .map(|(category, place)| mv1("fix_location_category", json!({ "criterion_id": c, "category": category, "where": place })))
        .collect()
}

fn mv(kind: &str, fields: Value, more: Vec<MoveFact>, label: &str) -> Move {
    Move {
        kind: kind.to_string(),
        fields: fields_of(fields),
        more: Some(more),
        label: Some(label.to_string()),
    }
}

fn mv1(kind: &str, fields: Value) -> Move {
    mv(kind, fields, Vec::new(), kind)
}

/// A fresh commit id; ids are names, not content hashes.
fn next_id(v: &View, i: usize) -> String {
    format!("k{}", v.kind("commit_seen").count() + 1 + i)
}

/// The tip's id, or main's sha at submit before any commit.
fn sha_now(v: &View) -> String {
    v.last("commit_seen")
        .and_then(|t| text(t, "commit"))
        .unwrap_or(INTENT_SHA)
        .to_string()
}

/// A new anchor exit with its planning check, one matching and one not.
fn reanchor_moves(v: &View, c: &str) -> Vec<Move> {
    let Some(r) = row_of(c) else {
        return Vec::new();
    };
    let sha = sha_now(v);
    let exit = json!({ "criterion_id": c, "places": [r.place.to_json()] });
    // the owner (2026-10-11): a light criterion's re-anchor carries the binary's documentation check of the sample's place (true) [reading]
    let doc: Vec<MoveFact> = if plan::path_of(v, c) == Some("light") {
        let a = Fact { t: 0, at: 0.0, kind: "place_chosen", tenant: String::new(), fields: fields_of(exit.clone()) };
        plan::doc_spots_of(&a)
            .unwrap_or_default()
            .into_iter()
            .map(|x| {
                let mut f = fields_of(json!({ "criterion_id": c }));
                f.extend(x.to_fields());
                f.extend(fields_of(json!({ "sha": sha, "matched": true })));
                MoveFact { kind: "anchor_is_documentation".into(), fields: f }
            })
            .collect()
    } else {
        Vec::new()
    };
    [(LINES_PLANNING, true), (LINES_OUTSIDE, false)]
        .iter()
        .map(|(h, m)| {
            let mut check = fields_of(json!({ "criterion_id": c }));
            if let Value::Object(p) = r.place.to_json() {
                check.extend(p);
            }
            check.extend(fields_of(json!({ "sha": sha, "lines_hash": h, "matched": m, "checked_at": "planning" })));
            let mut more = vec![MoveFact { kind: "anchor_sha_matched".into(), fields: check }];
            more.extend(doc.iter().cloned());
            mv("place_chosen", exit.clone(), more, "re_anchor")
        })
        .collect()
}

/// The binary's `place_chosen` right after the person's keep_going on `anchor_untouched`
/// (F12, the owner 2026-10-13: "append an anchor to the plan ... with a stamp that the user
/// basically overrode it"): one place per file the change landed in since the seal, carrying
/// the answer's hold and choice, each with its planning check at the tests-first commit. The
/// sample's lines are `LINES_LANDED`; the binary reads them from the hunks. `None` while no
/// such answer stands, or nothing landed.
pub fn kept_place_move(v: &View, c: &str) -> Option<Move> {
    plan::keep_going_answer(v, c)?;
    if released(v, c) {
        return None;
    }
    let files = crate::criterion::landed_files(v, c);
    if files.is_empty() {
        return None;
    }
    let sha = tests_first_commit(v, c)?.ids.last()?.clone();
    let places: Vec<Value> = files
        .iter()
        .map(|f| json!({ "file_path": f, "symbol": "", "line_start": LINES_LANDED.0, "line_end": LINES_LANDED.1 }))
        .collect();
    let checks = places
        .iter()
        .map(|p| {
            let mut check = fields_of(json!({ "criterion_id": c }));
            if let Value::Object(p) = p {
                check.extend(p.clone());
            }
            check.extend(fields_of(json!({ "sha": sha, "lines_hash": LINES_PLANNING, "matched": true, "checked_at": "planning" })));
            MoveFact { kind: "anchor_sha_matched".into(), fields: check }
        })
        .collect();
    Some(mv("place_chosen", json!({ "criterion_id": c, "places": places, "hold": "anchor_untouched", "choice": "keep_going" }), checks, "landed_place"))
}

/// `c`'s anchor is the place the binary kept on the person's keep_going (F12).
fn kept_place(v: &View, c: &str) -> bool {
    plan::anchor_of(v, c).is_some_and(|a| a.kind == "place_chosen" && text(a, "hold") == Some("anchor_untouched") && text(a, "choice") == Some("keep_going"))
}

/// The binary's documentation check of a light part's kept place, right after it lands
/// (door/anchor.rs `binary_checks` → `documentation::write_one`, every spot of the anchor):
/// for the first spot not yet checked, the check either way, since the binary may read
/// either. Empty for a full part, any other anchor, or once every spot is checked.
pub fn kept_doc_moves(v: &View, c: &str) -> Vec<Move> {
    if plan::path_of(v, c) != Some("light") {
        return Vec::new();
    }
    if !kept_place(v, c) {
        return Vec::new();
    }
    let Some(a) = plan::anchor_of(v, c) else {
        return Vec::new();
    };
    let Some(spot) = plan::doc_spots_of(a).unwrap_or_default().into_iter().find(|x| plan::doc_of(v, c, x).is_none()) else {
        return Vec::new();
    };
    let sha = sha_now(v);
    [true, false]
        .iter()
        .map(|m| {
            let mut f = fields_of(json!({ "criterion_id": c }));
            f.extend(spot.to_fields());
            f.extend(fields_of(json!({ "sha": sha, "matched": m })));
            mv("anchor_is_documentation", Value::Object(f), Vec::new(), "kept_doc_check")
        })
        .collect()
}

/// Every answer a person may give to an open hold; the sample types no text. The reserved part
/// `setup` comes after the parts (plan A-4). A choice that confirms the can't-tell card's run
/// (H-4) is offered only at its stage, bound to that run. The cases card's answers are
/// `cases_moves`', as its finding allows [draft].
fn answer_moves(v: &View) -> Vec<Move> {
    let mut out = Vec::new();
    for c in plan::criteria(v).into_iter().chain(std::iter::once(crate::reading::SETUP_PART.to_string())) {
        for h in holds(v, &c).iter() {
            // a clarifying question on a signal [draft]: the person picks a reading on the ask's one card
            if h.why == crate::ask::HOLD && crate::ask::carries(v, &c) {
                for ch in h.clears.iter().filter(|m| m.get("kind").and_then(Value::as_str) == Some("hold_answered")).filter_map(|m| m.get("choice")) {
                    out.push(mv1("hold_answered", json!({ "criterion_id": c, "hold": h.why, "choice": ch, "typed": "" })));
                }
            }
            if h.why == crate::cases::CARD {
                continue;
            }
            if let Some(choices) = crate::criterion::answers_of(&h.why) {
                for choice in choices {
                    let mut fields = json!({ "criterion_id": c, "hold": h.why, "choice": choice, "typed": "" });
                    if crate::criterion::CONFIRMS.contains(choice) {
                        match crate::criterion::confirmable(v, &c, choice) {
                            Some(run_id) => fields["run_id"] = json!(run_id),
                            None => continue,
                        }
                    }
                    out.push(mv1("hold_answered", fields));
                }
            }
        }
    }
    out
}

/// The answers to the reserved part's open holds, offered where the parts' are not.
fn answers_on(v: &View, c: &str) -> Vec<Move> {
    let mut out = Vec::new();
    for h in holds(v, c).iter() {
        for choice in crate::criterion::answers_of(&h.why).unwrap_or_default() {
            out.push(mv1("hold_answered", json!({ "criterion_id": c, "hold": h.why, "choice": choice, "typed": "" })));
        }
    }
    out
}

/// Whether the log holds the adapter's world (lane a5): a choice was made. Before one, no move of
/// the adapter is offered, so every log without one reads as before.
pub fn adapter_world(v: &View) -> bool {
    v.kind("reader_chosen").next().is_some()
}

/// The binary's dry run for the first stack one is owed for (plan A-3), as its choice lands: with
/// no gap, and with the sample's one gap. The run's own facts are the binary's (lane A4).
pub fn dry_run_moves(v: &View) -> Vec<Move> {
    if !adapter_world(v) {
        return Vec::new();
    }
    let Some(o) = crate::reading::dry_run_owed(v, crate::sample::FAMILIAR_VERSION).into_iter().next() else {
        return Vec::new();
    };
    let n = v.kind("reader_chosen").count();
    [Vec::new(), vec![crate::sample::GAP]]
        .iter()
        .map(|gaps| {
            let f = json!({
                "run_id": format!("d-{n}"), "root": o.root, "runner": crate::sample::RUNNER, "reader": crate::sample::READER,
                "command": o.command, "commit": sha_now(v), "familiar": crate::sample::FAMILIAR_VERSION,
                "watched": crate::sample::WATCHED, "gaps": gaps, "why": o.why,
            });
            mv("reader_chosen", f, Vec::new(), "dry_run")
        })
        .collect()
}

/// The re-check of `c`'s tests-first commit with the setup of commit `x` put back (plan A-5): the
/// run reserved with `setup_from`, ended and translated, an honest red or a pass, and no result:
/// a re-check is never a result or an attempt. Labelled `recheck`, or `pipeline_recheck` in CI.
fn recheck_moves(v: &View, c: &str, x: &str, place: Place) -> Vec<Move> {
    let Some(tf) = tests_first_commit(v, c).and_then(|t| t.ids.last().cloned()) else {
        return Vec::new();
    };
    let ci = place == Place::Pipeline;
    let n = v.kind("run_reserved").filter(|f| f.criterion_id() == Some(c) && text(f, "setup_from").is_some()).count();
    let rid = format!("s-{c}-{x}-{n}");
    ["tests_failed", "passed"]
        .iter()
        .map(|label| {
            let word = if *label == "passed" { "passed" } else { "failed" };
            let t = json!({ "run_id": rid, "criterion_id": c, "commit": tf, "by": "regex", "label": label, "evidence": "", "failing_tests": [], "classified_on": "full" });
            let more = vec![
                MoveFact { kind: "run_ended".into(), fields: fields_of(json!({ "run_id": rid, "outcome": word, "end": "exited" })) },
                MoveFact { kind: "test_log_translation".into(), fields: fields_of(t) },
            ];
            let (by, resources, wait) = if ci { ("pipeline", json!([]), "0") } else { ("binary", json!(["tests"]), "60") };
            let reserved = json!({ "run_id": rid, "criterion_id": c, "criterion_rev": criterion_rev(v, c), "commit": tf, "command": TEST_COMMAND, "resources": resources, "wait": wait, "by": by, "setup_from": x });
            mv("run_reserved", reserved, more, if ci { "pipeline_recheck" } else { "recheck" })
        })
        .collect()
}

fn result_moves(c: &str, t: &Test, id: &str, place: Place, rev: bool) -> Vec<Move> {
    ["passed", "failed"]
        .iter()
        .map(|o| {
            mv1("pass_please_pass", json!({ "criterion_id": c, "file_path": t.file_path, "commit": id, "outcome": o, "ran_in": place.as_str(), "with_anchor_reverted": rev }))
        })
        .collect()
}

/// Stamps `c` with its own fingerprint as observed at that commit; with none
/// observed there is no stamp.
fn stamp_move(v: &View, c: &str, id: &str, place: Place) -> Vec<Move> {
    let Some(o) = crate::criterion::observed(v, c, id) else {
        return Vec::new();
    };
    vec![mv1("fingerprint_stamped", json!({ "criterion_id": c, "commit": id, "proof_not_tampered": o.fields.get("tests_hash"), "ran_in": place.as_str() }))]
}

/// The action's next step for `c`, in a fixed order: tests at the tests-first
/// commit, stamp, tests at the head, stamp, then the teeth run.
fn pipeline_step(v: &View, c: &str, run: &Fact) -> Vec<Move> {
    let Some(tf) = tests_first_commit(v, c) else {
        return Vec::new();
    };
    let Some(tf_id) = tf.ids.last() else {
        return Vec::new();
    };
    let head = text(run, "head_commit").unwrap_or("");
    let ix = v.idx();
    let head_key = ix.key_of(head);
    let own = own_tests(v, c);
    let has = |key: Option<&str>, t: &Test, rev: bool| {
        results(v, c, Place::Pipeline, rev).iter().any(|r| {
            r.t > run.t && text(r, "commit").and_then(|id| ix.key_of(id)) == key && tname(r) == t.file_path
        })
    };
    let stamped = |key: Option<&str>| {
        mine(v, c, "fingerprint_stamped").iter().any(|s| {
            text(s, "ran_in") == Some("pipeline")
                && s.t > run.t
                && text(s, "commit").and_then(|id| ix.key_of(id)) == key
        })
    };
    let touching = touching_tests(v, c);
    let missing = |key: Option<&str>, ts: &[Test], rev: bool| -> Vec<Test> { ts.iter().filter(|t| !has(key, t, rev)).cloned().collect() };
    let own_tf = missing(Some(&tf.key), &own, false);
    if !own_tf.is_empty() {
        return pipeline_run_moves(v, c, tf_id, false, &own_tf, &[]);
    }
    if !stamped(Some(&tf.key)) {
        return stamp_move(v, c, tf_id, Place::Pipeline);
    }
    let (own_h, touch_h) = (missing(head_key, &own, false), missing(head_key, &touching, false));
    if !own_h.is_empty() || !touch_h.is_empty() {
        return pipeline_run_moves(v, c, head, false, &own_h, &touch_h);
    }
    if !stamped(head_key) {
        return stamp_move(v, c, head, Place::Pipeline);
    }
    // the teeth run; for new work it puts the integration point back
    let own_t = missing(head_key, &own, true);
    if !own_t.is_empty() {
        return pipeline_run_moves(v, c, head, true, &own_t, &[]);
    }
    // A-5 (lane a5): after the teeth, CI re-checks the seal under each later setup, once per run
    for x in crate::reading::setup_commits(v, c) {
        let xk = text(x, "commit").and_then(|id| ix.key_of(id));
        let done = v.of("run_reserved", c).any(|r| {
            r.t > run.t
                && crate::reading::recheck_place(r) == Some(Place::Pipeline)
                && text(r, "setup_from").and_then(|id| ix.key_of(id)) == xk
        });
        if !done {
            let id = xk.and_then(|k| ix.ids.get(k)).and_then(|ids| ids.last()).cloned().unwrap_or_default();
            return recheck_moves(v, c, &id, Place::Pipeline);
        }
    }
    Vec::new()
}

/// The pipeline's run of `c` at `id` for the tests not yet run there. While the record has no
/// translation its results are legacy, one file at a time as before; once it translates, a
/// result without a run is Unknown (plan §6.5, S9d), so the run is reserved by the pipeline,
/// ended, translated and its results name it, in one batch per label: the own tests take the
/// run's word, the touching tests pass.
fn pipeline_run_moves(v: &View, c: &str, id: &str, rev: bool, own: &[Test], touching: &[Test]) -> Vec<Move> {
    if !v.kind("test_log_translation").any(|_| true) {
        let t = own.first().or(touching.first());
        return t.map(|t| result_moves(c, t, id, Place::Pipeline, rev)).unwrap_or_default();
    }
    let n = v.kind("run_reserved").filter(|f| f.criterion_id() == Some(c) && text(f, "commit") == Some(id)).count();
    let rid = format!("p-{c}-{id}-{n}");
    ["passed", "tests_failed"]
        .iter()
        .map(|label| {
            let word = if *label == "passed" { "passed" } else { "failed" };
            let t = json!({ "run_id": rid, "criterion_id": c, "commit": id, "by": "regex", "label": label, "evidence": "", "failing_tests": [], "classified_on": "full" });
            let mut more = vec![
                MoveFact { kind: "run_ended".into(), fields: fields_of(json!({ "run_id": rid, "outcome": word, "end": "exited" })) },
                MoveFact { kind: "test_log_translation".into(), fields: fields_of(t) },
            ];
            for (t, o) in own.iter().map(|t| (t, word)).chain(touching.iter().map(|t| (t, "passed"))) {
                more.push(MoveFact {
                    kind: "pass_please_pass".into(),
                    fields: fields_of(json!({ "criterion_id": c, "file_path": t.file_path, "commit": id, "outcome": o, "ran_in": "pipeline", "with_anchor_reverted": rev, "run_id": rid })),
                });
            }
            let reserved = json!({ "run_id": rid, "criterion_id": c, "criterion_rev": criterion_rev(v, c), "commit": id, "command": TEST_COMMAND, "resources": [], "wait": "0", "by": "pipeline" });
            mv("run_reserved", reserved, more, "pipeline_ran")
        })
        .collect()
}

/// The binary's next fact: the place where the change landed right after the
/// person's keep_going, else the pipeline's step on its head if a run is seen,
/// else each runnable criterion's tests and stamp at the tip, in plan order.
/// Both run to the end before the agent's next act.
pub fn binary_moves(v: &View) -> Vec<Move> {
    let d = dependency_moves(v);
    if !d.is_empty() {
        return d;
    }
    for c in plan::criteria(v) {
        if let Some(m) = kept_place_move(v, &c) {
            return vec![m];
        }
        let d = kept_doc_moves(v, &c);
        if !d.is_empty() {
            return d;
        }
    }
    // the adapter (lane a5): an owed dry run is reserved first, then each owed re-check of a seal
    let d = dry_run_moves(v);
    if !d.is_empty() {
        return d;
    }
    for c in plan::criteria(v) {
        if released(v, &c) {
            continue;
        }
        if let Some(x) = crate::reading::setup_rechecks_owed(v, &c).into_iter().next() {
            return recheck_moves(v, &c, &x, Place::Laptop);
        }
    }
    if let Some(run) = v.last("pipeline_run_seen") {
        for c in plan::criteria(v) {
            if !released(v, &c) {
                let s = pipeline_step(v, &c, run);
                if !s.is_empty() {
                    return s;
                }
            }
        }
    }
    for a in runnable_criteria(v) {
        let s = laptop_step(v, &a);
        if !s.is_empty() {
            return s;
        }
    }
    Vec::new()
}

/// The binary's draft after the person's `a_dependency` (plan §7.3; lane S9a): for the first
/// approval with no draft, `dependency_drafted` and the dependency's lines in one batch, from
/// the sample's dependency row: its criterion (path and ids copied from the part that needs it),
/// its place with the planning check at the tip, its planned test, the insert just before the
/// part (§7.2), its holder (the part's) and its fix location. None once the row's id is taken.
pub fn dependency_moves(v: &View) -> Vec<Move> {
    let r = &crate::sample::DEPENDENCY;
    for c in plan::criteria(v) {
        for a in crate::dependency::approvals(v, &c) {
            if crate::dependency::draft_of(v, a.t).is_some() || plan::criteria(v).iter().any(|x| x == r.criterion_id) {
                continue;
            }
            let d = r.criterion_id;
            let Some(src) = last_of(v.of("criterion", &c).collect()) else { continue };
            let test = r.tests.first().map(|t| t.file_path).unwrap_or("");
            let mut crit = fields_of(json!({ "criterion_id": d, "text": r.text }));
            for k in ["path", "behavior_id", "promise_id"] {
                if let Some(x) = src.fields.get(k) {
                    crit.insert(k.into(), x.clone());
                }
            }
            let mut check = fields_of(json!({ "criterion_id": d }));
            check.extend(fields_of(r.place.to_json()));
            check.extend(fields_of(json!({ "sha": sha_now(v), "lines_hash": LINES_PLANNING, "matched": true, "checked_at": "planning" })));
            let holder = plan::assigned_to(v, &c).unwrap_or(r.agent_conversation_id);
            let mut more = vec![
                MoveFact { kind: "criterion".into(), fields: crit },
                MoveFact { kind: "place_chosen".into(), fields: fields_of(json!({ "criterion_id": d, "places": [r.place.to_json()] })) },
                MoveFact { kind: "anchor_sha_matched".into(), fields: check },
                MoveFact { kind: "test_planned".into(), fields: fields_of(json!({ "criterion_id": d, "file_path": test, "will": "create" })) },
            ];
            more.extend(crate::dependency::restring(v, d, &c).into_iter().map(|(kind, fields)| MoveFact { kind: kind.into(), fields }));
            more.push(MoveFact { kind: "criterion_assigned".into(), fields: fields_of(json!({ "criterion_id": d, "agent_conversation_id": holder })) });
            more.push(MoveFact { kind: "fix_location_category".into(), fields: fields_of(json!({ "criterion_id": d, "category": "this_repository", "where": "" })) });
            let draft = json!({ "approval": a.t, "criterion_id": d, "text": r.text, "test_file": test });
            return vec![mv("dependency_drafted", draft, more, "dependency_drafted")];
        }
    }
    Vec::new()
}

/// One runnable criterion's next binary fact at the tip.
fn laptop_step(v: &View, a: &str) -> Vec<Move> {
    let Some(tip) = v.last("commit_seen") else {
        return Vec::new();
    };
    if crate::reader::t_of(tip) <= epoch(v, a) {
        return Vec::new();
    }
    if touches_tests_of(v, a, tip) {
        for fp in test_files(v, a) {
            if !v.of("test_written", a).any(|w| text(w, "file_path") == Some(&fp)) {
                let t = row_of(a).and_then(|r| r.tests.iter().find(|x| x.file_path == fp));
                if let Some(t) = t {
                    return vec![mv1("test_written", json!({ "criterion_id": a, "file_path": fp, "test_name": t.test_name }))];
                }
            }
        }
    }
    let own = own_tests(v, a);
    if own.is_empty() {
        return Vec::new();
    }
    let tip_id = text(tip, "commit").unwrap_or("");
    let laptop = results(v, a, Place::Laptop, false);
    let none_yet = !laptop.iter().any(|r| text(r, "commit") == Some(tip_id));
    for t in own.iter().chain(touching_tests(v, a).iter()) {
        if !laptop.iter().any(|r| text(r, "commit") == Some(tip_id) && tname(r) == t.file_path) {
            let mut out = result_moves(a, t, tip_id, Place::Laptop, false);
            if none_yet {
                out.extend(run_moves(v, a, tip_id));
            }
            return out;
        }
    }
    let stamped = mine(v, a, "fingerprint_stamped")
        .iter()
        .any(|s| text(s, "ran_in") == Some("laptop") && text(s, "commit") == Some(tip_id));
    if !stamped {
        return stamp_move(v, a, tip_id, Place::Laptop);
    }
    Vec::new()
}

/// The labels a sample run is translated as (plan §6.1): a pass, a valid red by assertion and
/// by the place's missing name, a run too slow, and one the tables cannot read.
pub const RUN_LABELS: [&str; 5] = ["passed", "tests_failed", "missing_target_symbol", "timed_out", "unclassified"];

/// The binary's run of `a`'s command at `id` as S3–S6 write it: reserved, ended, translated,
/// and a result naming the run for each own test (with the run's word); then the touching tests'
/// run of their own under the touching command (the owner's plan of 2026-10-21, step 0) [draft],
/// as pytest's table reads a pass beside a skip (unclassified, exit 0, counted), its result for
/// each touching test (passed). With no touching command the touching results name the own run,
/// as the binary's do. One move per label in `RUN_LABELS`.
fn run_moves(v: &View, a: &str, id: &str) -> Vec<Move> {
    let Some(row) = row_of(a) else {
        return Vec::new();
    };
    // the id names the work, not the log's length, so two logs alike offer alike moves
    // the laptop's own runs: a pipeline run reserved here is CI's (S9d)
    let n = v.kind("run_reserved").filter(|f| f.criterion_id() == Some(a) && text(f, "commit") == Some(id) && text(f, "by") != Some("pipeline")).count();
    let rid = format!("r-{a}-{id}-{n}");
    let test = row.tests.first().map(|t| t.file_path).unwrap_or("");
    RUN_LABELS
        .iter()
        .map(|label| {
            let word = if *label == "passed" { "passed" } else { "failed" };
            // read as the binary reads a run now (lane H6, D7): every translation carries counts
            let mut t = json!({ "run_id": rid, "criterion_id": a, "commit": id, "by": "regex", "label": label, "evidence": "", "failing_tests": [], "classified_on": "full", "counts": {} });
            match *label {
                "tests_failed" => {
                    t["location"] = json!({ "file_path": test, "line": 1, "kind": "assertion" });
                    t["counts"] = json!({ "failed": 1, "passed": 1 });
                }
                "passed" => t["counts"] = json!({ "passed": 2 }),
                "missing_target_symbol" => {
                    t["missing_name"] = json!(row.place.symbol);
                    t["location"] = json!({ "file_path": test, "line": 1 });
                }
                _ => {}
            }
            let end = if *label == "timed_out" { "timed_out" } else { "exited" };
            let mut more = vec![
                MoveFact { kind: "run_ended".into(), fields: fields_of(json!({ "run_id": rid, "outcome": word, "end": end })) },
                MoveFact { kind: "test_log_translation".into(), fields: fields_of(t) },
            ];
            let result = |t: &Test, o: &str, run: &str| MoveFact {
                kind: "pass_please_pass".into(),
                fields: fields_of(json!({ "criterion_id": a, "file_path": t.file_path, "commit": id, "outcome": o, "ran_in": "laptop", "with_anchor_reverted": false, "run_id": run })),
            };
            let own = own_tests(v, a);
            for t in &own {
                more.push(result(t, word, &rid));
            }
            let touching: Vec<Test> = touching_tests(v, a).into_iter().filter(|t| !own.iter().any(|o| o.file_path == t.file_path)).collect();
            match crate::criterion::touching_command(v, a).filter(|_| !touching.is_empty()) {
                Some(command) => {
                    let trid = format!("{rid}-t");
                    more.push(MoveFact { kind: "run_reserved".into(), fields: fields_of(json!({ "run_id": trid, "criterion_id": a, "criterion_rev": row.criterion_rev, "commit": id, "command": command, "resources": ["tests"], "wait": "60", "by": "binary" })) });
                    more.push(MoveFact { kind: "run_ended".into(), fields: fields_of(json!({ "run_id": trid, "outcome": "passed", "end": "exited" })) });
                    let tt = json!({ "run_id": trid, "criterion_id": a, "commit": id, "by": "regex", "label": "unclassified", "evidence": "", "failing_tests": [], "classified_on": "full", "counts": { "passed": 1, "skipped": 1 } });
                    more.push(MoveFact { kind: "test_log_translation".into(), fields: fields_of(tt) });
                    for t in &touching {
                        more.push(result(t, "passed", &trid));
                    }
                }
                None => {
                    for t in &touching {
                        more.push(result(t, "passed", &rid));
                    }
                }
            }
            let reserved = json!({ "run_id": rid, "criterion_id": a, "criterion_rev": row.criterion_rev, "commit": id, "command": TEST_COMMAND, "resources": ["tests"], "wait": "60", "by": "binary" });
            mv("run_reserved", reserved, more, "ran")
        })
        .collect()
}

/// One piece of work a criterion owes: its tests run at a commit of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Work {
    pub criterion_rev: String,
    pub commit: String,
}

/// Whether each of `c`'s own and touching tests (or, with none written yet, any
/// one) has a laptop run at `id` that ran and was not a teeth run: one file's
/// result does not serve the others. Until run facts carry authority, any such
/// result is accepted.
fn accepted_at(v: &View, c: &str, id: &str) -> bool {
    let ran: Vec<&str> = v
        .of("pass_please_pass", c)
        .filter(|r| {
            text(r, "commit") == Some(id)
                && text(r, "ran_in") == Some("laptop")
                && !crate::reader::flag(r, "with_anchor_reverted")
                && text(r, "outcome") != Some(DID_NOT_RUN)
        })
        .filter_map(|r| text(r, "file_path"))
        .collect();
    let tests: Vec<Test> = own_tests(v, c).into_iter().chain(touching_tests(v, c)).collect();
    if tests.is_empty() {
        return !ran.is_empty();
    }
    tests.iter().all(|t| ran.contains(&t.file_path.as_str()))
}

/// The holds whose `try_again` runs the commit again (plan §6.7, behaviour 2).
const RERUN_HOLDS: [&str; 3] = ["environment_error", "run_unclassified", "retry_spent"];

/// Each commit that the person's `try_again` on `environment_error`, `run_unclassified` or
/// `retry_spent` runs again for `c`, with the latest such answer for it (plan §6.7, behaviour 2):
/// the commit of `c`'s latest laptop result before the answer. What ran there before the answer is
/// the run it answered; the re-run's results come after it.
pub fn reruns<'a>(v: &View<'a>, c: &str) -> Vec<(String, &'a Fact)> {
    let laptop: Vec<&Fact> = mine(v, c, "pass_please_pass")
        .into_iter()
        .filter(|r| text(r, "ran_in") == Some("laptop") && !crate::reader::flag(r, "with_anchor_reverted"))
        .collect();
    let mut out: Vec<(String, &Fact)> = Vec::new();
    let answers = mine(v, c, "hold_answered")
        .into_iter()
        .filter(|a| RERUN_HOLDS.contains(&text(a, "hold").unwrap_or("")) && text(a, "choice") == Some("try_again"));
    for a in answers {
        let Some(id) = laptop.iter().rev().find(|r| r.t < a.t).and_then(|r| text(r, "commit")) else { continue };
        match out.iter_mut().find(|(x, _)| x == id) {
            Some(x) => x.1 = a,
            None => out.push((id.to_string(), a)),
        }
    }
    out
}

/// Whether the re-run the answer `a` owes at `id` happened: each of `c`'s own and touching tests
/// (with none written yet, any one) has a laptop result there after the answer, whatever it ended
/// with. Once per answer.
fn rerun_done(v: &View, c: &str, id: &str, a: &Fact) -> bool {
    let after: Vec<&str> = v
        .of("pass_please_pass", c)
        .filter(|r| r.t > a.t && text(r, "commit") == Some(id))
        .filter(|r| text(r, "ran_in") == Some("laptop") && !crate::reader::flag(r, "with_anchor_reverted"))
        .filter_map(|r| text(r, "file_path"))
        .collect();
    let tests: Vec<Test> = own_tests(v, c).into_iter().chain(touching_tests(v, c)).collect();
    if tests.is_empty() {
        return !after.is_empty();
    }
    tests.iter().all(|t| after.iter().any(|f| *f == t.file_path))
}

/// The commits touching `c`'s tests or anchor since its last accepted result, and each commit the
/// person's `try_again` runs again until its re-run happened ([`reruns`]), and the re-earn commit
/// until each test ran there (plan §7.1), oldest first. Any other commit outside every anchor is
/// owed by no criterion.
pub fn owed(v: &View, c: &str) -> Vec<Work> {
    let e = epoch(v, c);
    let commits: Vec<&Fact> = v.kind("commit_seen").filter(|f| crate::reader::t_of(f) > e).collect();
    let last = commits
        .iter()
        .rposition(|f| text(f, "commit").is_some_and(|id| accepted_at(v, c, id)))
        .map_or(0, |i| i + 1);
    let again: Vec<String> = reruns(v, c).into_iter().filter(|(id, a)| !rerun_done(v, c, id, a)).map(|(id, _)| id).collect();
    // the re-earn commit (plan §7.1): neither B nor G touches c's tests or anchor
    let re_earn: Option<String> = crate::dependency::re_earn(v, c).map(|r| r.commit).filter(|id| !accepted_at(v, c, id));
    let rev = criterion_rev(v, c);
    let mut out: Vec<Work> = Vec::new();
    for (i, f) in commits.iter().enumerate() {
        let Some(id) = text(f, "commit") else { continue };
        let due = (i >= last && touches(v, c, f)) || again.iter().any(|x| x == id) || re_earn.as_deref() == Some(id);
        if due && !out.iter().any(|w| w.commit == id) {
            out.push(Work { criterion_rev: rev.clone(), commit: id.to_string() });
        }
    }
    out
}

/// The agent's possible commits for the active criterion, right and wrong.
pub fn commit_moves(v: &View, a: &str) -> Vec<Move> {
    let Some(r) = row_of(a) else {
        return Vec::new();
    };
    let Some(tfile) = r.tests.first().map(|t| t.file_path) else {
        return Vec::new();
    };
    let af = place(v, a)
        .and_then(|p| p.get("file_path"))
        .and_then(Value::as_str)
        .unwrap_or(r.place.file_path)
        .to_string();
    let cur = crate::criterion::current_tests(v, a).map(str::to_string);
    let id = next_id(v, 0);
    let gf = greenfield(v, a);
    // The binary observes every criterion at the commit, in plan order: `a`'s files as
    // this commit leaves them, every other's as they stand, and none where no file of it
    // exists. `commit_seen` itself carries no hash.
    let crits = crate::plan::criteria(v);
    let obs = |h: Option<&str>| -> Vec<MoveFact> {
        crits
            .iter()
            .filter_map(|x| {
                let hx = if x.as_str() == a { h.map(str::to_string) } else { crate::criterion::current_tests(v, x).map(str::to_string) }?;
                let mut fields = Map::new();
                fields.insert("criterion_id".into(), Value::from(x.as_str()));
                if let Some(rev) = row_of(x).map(|r| r.criterion_rev) {
                    fields.insert("criterion_rev".into(), Value::from(rev));
                }
                fields.insert("commit".into(), Value::from(id.as_str()));
                fields.insert("tests_hash".into(), Value::from(hx));
                Some(MoveFact { kind: "tests_observed".into(), fields })
            })
            .collect()
    };
    let cm = |files: Vec<&str>, tt: bool, tc: bool, ov: Vec<&str>, h: Option<&str>, label: &str| {
        let mut f = json!({ "commit": id, "files": files, "touches_tests": tt, "touches_code": tc, "anchor_overlap": ov });
        carry_existing(v, &mut f);
        mv("commit_seen", f, obs(h), label)
    };
    let mut out: Vec<Move> = match &cur {
        None => vec![cm(vec![tfile], true, false, vec![], Some(r.hash_sealed), "tests_only")],
        Some(cur) => [r.hash_sealed, r.hash_edited]
            .iter()
            .filter(|h| *h != cur)
            .map(|h| cm(vec![tfile], true, false, vec![], Some(h), "changed_test"))
            .collect(),
    };
    let on_anchor: Vec<&str> = if gf { vec![] } else { vec![a] };
    out.push(cm(vec![&af], false, true, on_anchor.clone(), cur.as_deref(), "code"));
    if !gf {
        // the owner (2026-10-11): a's code also changes the lines of a criterion not yet started (anchor_overlap names both)
        if let Some(d) = crits.iter().find(|x| x.as_str() != a && place(v, x).is_some() && start_check(v, x).is_none()) {
            let df = place(v, d).and_then(|p| p.get("file_path")).and_then(Value::as_str).unwrap_or_default();
            let mut files = vec![af.as_str()];
            if df != af {
                files.push(df);
            }
            out.push(cm(files, false, true, vec![a, d.as_str()], cur.as_deref(), "code_on_sibling_anchor"));
        }
    }
    if !gf {
        out.push(cm(vec![&af], false, true, vec![], cur.as_deref(), "code_outside_anchor")); // the agent is told, not refused
    }
    if cur.is_none() {
        out.push(cm(vec![tfile, &af], true, true, on_anchor, Some(r.hash_sealed), "tests_and_code_mixed"));
    }
    out
}

/// A commit leaves a standing change to existing tests standing (the owner, 2026-10-20) [draft]:
/// the binary reads it against the base, so every later commit carries the tip's.
fn carry_existing(v: &View, f: &mut Value) {
    if let Some(x) = v.last("commit_seen").and_then(|t| t.fields.get(crate::existing::FIELD)) {
        f[crate::existing::FIELD] = x.clone();
    }
}

/// The adapter (lane a5): a commit of a file the choice watches, neither tests nor code, made
/// whatever part is at work (a part green or not); every part observed as it stands.
fn setup_change_move(v: &View) -> Option<Move> {
    if !adapter_world(v) {
        return None;
    }
    let id = next_id(v, 0);
    let obs: Vec<MoveFact> = crate::plan::criteria(v)
        .iter()
        .filter_map(|x| {
            let h = crate::criterion::current_tests(v, x)?;
            let mut fields = Map::new();
            fields.insert("criterion_id".into(), Value::from(x.as_str()));
            if let Some(rev) = row_of(x).map(|r| r.criterion_rev) {
                fields.insert("criterion_rev".into(), Value::from(rev));
            }
            fields.insert("commit".into(), Value::from(id.as_str()));
            fields.insert("tests_hash".into(), Value::from(h));
            Some(MoveFact { kind: "tests_observed".into(), fields })
        })
        .collect();
    let mut f = json!({ "commit": id, "files": [crate::sample::SETUP_FILE], "touches_tests": false, "touches_code": false, "anchor_overlap": [] });
    carry_existing(v, &mut f);
    Some(mv("commit_seen", f, obs, "setup_change"))
}

fn stack_move(cmd: &str) -> Move {
    mv1("test_stack_detected", json!({ "codebase_id": CODEBASE_ID, "project_root": PROJECT_ROOT, "test_stack": TEST_STACK, "test_command": cmd }))
}

/// Every move admissible in the criterion's world at this log.
pub fn criterion_admissible(v: &View) -> Vec<Move> {
    let mut out: Vec<Move> = Vec::new();
    let crits = plan::criteria(v);
    if !crate::criterion::confirmed(v) {
        // before confirmation only the planning holds' clears are admissible
        for c in &crits {
            let hs = crate::lifecycle::whys(v, c);
            // after re_plan on anchor_moved, choosing a new place comes next
            if hs.iter().any(|w| w == "anchor_not_found") || plan::unchosen(v, c).is_some() {
                out.extend(reanchor_moves(v, c));
            }
            // S7b: after a re_plan on a requirement_missed finding, the part's new reading
            out.extend(crate::finding::rescope_moves(v, c));
            // a light part's kept place: the binary's documentation check, and after a no its cure (claimed again --path full)
            out.extend(kept_doc_moves(v, c));
            if kept_place(v, c) && hs.iter().any(|w| w == "planning_incomplete") && kept_doc_moves(v, c).is_empty() {
                out.extend(crate::sample::plan_admissible(v).into_iter().filter(|m| {
                    m.kind == "behavior" || m.kind == "promise" || (m.kind == "criterion" && m.fields.get("criterion_id").and_then(Value::as_str) == Some(c.as_str()))
                }));
            }
            if hs.iter().any(|w| w == "no_test_stack") && !out.iter().any(|m| m.kind == "test_stack_detected") {
                out.push(stack_move(TEST_COMMAND));
            }
        }
        // the cases card [draft]: the binary's finding, the person's answers, the cases asked for claimed
        for c in &crits {
            out.extend(cases_moves(v, c));
        }
        // the one question [draft]: the person answers how to make it happen before the plan is confirmed
        for c in crits.iter().filter(|c| crate::trace::repro_unknown(v, c)) {
            for ch in [crate::trace::TYPE_IT, crate::trace::GO_AHEAD] {
                out.push(mv1("hold_answered", json!({ "criterion_id": c, "hold": crate::trace::REPRO, "choice": ch, "typed": "" })));
            }
        }
        if crits.iter().any(|c| kept_place(v, c)) {
            out.extend(answer_moves(v)); // a person may answer at any time, the kept place's planning lines owed or not
        } else {
            // the gap card (plan A-4) is the project's, answered before the plan is confirmed too
            out.extend(answers_on(v, crate::reading::SETUP_PART));
        }
        return out;
    }
    let b = binary_moves(v);
    if !b.is_empty() {
        out.extend(b);
        out.extend(answer_moves(v)); // a person may answer at any time
        return out;
    }
    let run = runnable_criteria(v);
    for a in &run {
        if state(v, a) == "testing" && crate::lifecycle::start_owed(v, a) {
            // F7: a start check for each place still lacking one, each read against its own planning check
            let places = plan::anchor_of(v, a).and_then(|x| x.fields.get("places")).and_then(Value::as_array);
            let owed = crate::lifecycle::place_checks(v, a).into_iter().zip(places.into_iter().flatten()).filter(|(pc, _)| pc.start.is_none());
            let mut all: Vec<Move> = Vec::new();
            for (pc, p) in owed {
                // the start check before the criterion starts; an outside edit (uncommitted: tip_matched false) shows as a moved hash
                let sha = sha_now(v);
                let pl = pc.planning;
                let same = pl.and_then(|pl| text(pl, "lines_hash")).unwrap_or(LINES_PLANNING);
                // the owner (2026-10-11): a commit after planning touched its lines: the binary maps the range
                let own = pl.is_some_and(|pl| {
                    v.kind("commit_seen").any(|f| crate::reader::t_of(f) > crate::reader::t_of(pl) && crate::reader::texts(f, "anchor_overlap").contains(&a.as_str()))
                });
                let mut q = p.clone();
                if let Value::Object(q) = &mut q {
                    for k in ["line_start", "line_end"] {
                        if let Some(n) = q.get(k).and_then(Value::as_i64) {
                            q.insert(k.into(), Value::from(n + MAPPED_SHIFT));
                        }
                    }
                }
                let mut checks = vec![(p, same, true, true, "start_check"), (p, LINES_OUTSIDE, false, false, "outside_edit_seen")];
                if own {
                    // after another criterion's commit on its lines, the lines at their new place with the working
                    // tree matching the tip, and the WRONG one, an uncommitted edit there too
                    checks.push((&q, LINES_CHANGED, true, true, "own_work_moved"));
                    checks.push((&q, LINES_CHANGED, true, false, "own_work_uncommitted"));
                }
                all.extend(checks.into_iter().map(|(at, h, m, tm, label)| {
                    let mut f = fields_of(json!({ "criterion_id": a }));
                    if let Value::Object(at) = at.clone() {
                        f.extend(at);
                    }
                    f.extend(fields_of(json!({ "sha": sha, "lines_hash": h, "matched": m, "checked_at": "start", "tip_matched": tm })));
                    mv("anchor_sha_matched", Value::Object(f), Vec::new(), label)
                }));
            }
            if !all.is_empty() {
                // the gap card is the project's: a part's start check does not wait on it, nor it on one
                all.extend(answers_on(v, crate::reading::SETUP_PART));
                return all;
            }
        }
    }
    out.extend(answer_moves(v));
    for c in &crits {
        out.extend(crate::ladder::investigator_moves(v, c));
        // S7b: the binary's diagnosis of a test that never went red, while owed
        out.extend(crate::finding::finding_moves(v, c));
    }
    for c in &crits {
        // the holder's first act is its claim; after re_plan it may claim again
        if first_act(v, c) || (!released(v, c) && replanned(v, c)) {
            out.extend(claim_moves(c));
        }
    }
    for c in &crits {
        if !first_act(v, c) && holds(v, c).iter().any(|h| h.why == "anchor_untouched" || h.why == "anchor_not_found") {
            out.extend(reanchor_moves(v, c));
        }
    }
    for a in &run {
        let first = first_act(v, a);
        // no commit before the holder's claim, nor while a listed place is unclassified, nor while
        // the ask's reading is unclear [draft]
        let held = crate::places::holds(v, a) || crate::ask::holds(v, a);
        if !paused(v, a) && !first {
            out.extend(commit_moves(v, a).into_iter().filter(|_| !held));
        }
        if sealed_at(v, a, Place::Laptop).is_some() && !announced(v, a) && !first {
            out.push(mv1("test_change_announced", json!({ "criterion_id": a, "reason": ANNOUNCE_REASON })));
        }
    }
    for c in &crits {
        // changing a sealed test of a green criterion is announced first (the way back)
        if !run.contains(c)
            && turn(v, c)
            && !released(v, c)
            && matches!(state(v, c), "green" | "proven")
            && sealed_at(v, c, Place::Laptop).is_some()
            && !announced(v, c)
            && !first_act(v, c)
        {
            out.push(mv("test_change_announced", json!({ "criterion_id": c, "reason": ANNOUNCE_REASON }), Vec::new(), "announce_after_green"));
        }
    }
    // an existing test's change stands unannounced (the owner, 2026-10-20) [draft]: the agent
    // announces each file with a reason
    if !first_act_all(v) {
        for e in crate::existing::unannounced(v) {
            out.push(mv1(crate::existing::ANNOUNCED, json!({ "file_path": e.get("file_path").cloned().unwrap_or(Value::Null), "reason": ANNOUNCE_REASON })));
        }
    }
    // places before the fix (the owner's plan of 2026-10-21, step 3) [draft]: the agent
    // classifies each listed site left, as applying or not affected with its reason
    for c in &crits {
        if crate::places::holds(v, c) {
            for s in crate::places::unclassified(v, c) {
                for (verdict, reason) in [(crate::places::APPLIES, ""), (crate::places::NOT_AFFECTED, PLACE_REASON)] {
                    let f = json!({ "criterion_id": c, "file": s.file, "line": s.line, "verdict": verdict, "reason": reason });
                    out.push(mv(crate::places::CLASSIFIED, f, Vec::new(), &format!("place_{verdict}")));
                }
            }
        }
    }
    // a clarifying question on a signal (the owner's plan of 2026-10-21, step 2) [draft]: the
    // agent says how the ask reads, one way and why, or the readings it sees
    if crits.iter().any(|c| crate::ask::owed(v, c)) {
        out.push(mv(crate::ask::ONE, json!({ "why": crate::ask::SAMPLE_WHY }), Vec::new(), "one_reading"));
        out.push(mv(crate::ask::ALTERNATIVES, json!({ "readings": crate::ask::SAMPLE_READINGS }), Vec::new(), "reading_alternatives"));
    }
    // the setup commit is a commit like any: none while every part at work is paused
    // nor while a part's listed place is unclassified or its ask's reading unclear [draft]: the
    // binary refuses every commit then
    let live: Vec<&String> = run.iter().filter(|a| !first_act(v, a)).collect();
    let held = |a: &String| crate::places::holds(v, a) || crate::ask::holds(v, a);
    if (live.is_empty() || live.iter().any(|a| !paused(v, a))) && !run.iter().any(held) {
        out.extend(setup_change_move(v));
    }
    let cur = last_of(
        v.kind("test_stack_detected")
            .filter(|f| text(f, "project_root") == Some(PROJECT_ROOT) && plan::has_command(f))
            .collect(),
    );
    // the command gate (plan §6.3): no new command while the person's fix_what_broke_it stands
    let locked = crate::lifecycle::command_locked_at(v, PROJECT_ROOT).is_some();
    if let Some(cur) = cur.filter(|_| !locked) {
        let next = if text(cur, "test_command") == Some(OTHER_COMMAND) { TEST_COMMAND } else { OTHER_COMMAND };
        out.push(stack_move(next)); // the agent announces a changed test_command
    }
    for c in &crits {
        if !released(v, c) && state(v, c) != "proven" && crate::ladder::investigator_of(v, c).is_none() {
            let from = plan::assigned_to(v, c);
            out.push(mv1("criterion_transfer", json!({ "criterion_id": c, "from_conversation": from, "to_conversation": other_agent(from) }))); // not while an investigator is open
        }
    }
    let cs: Vec<&Fact> = v.kind("commit_seen").collect();
    if !cs.is_empty() {
        // the binary observes each rewritten commit afresh: the same files, the same
        // fingerprints
        let mut copies: Vec<MoveFact> = Vec::new();
        for (i, f) in cs.iter().enumerate() {
            let nid = next_id(v, i);
            let mut fields = Map::new();
            fields.insert("commit".into(), Value::from(nid.as_str()));
            for k in ["files", "touches_tests", "touches_code", "anchor_overlap", crate::existing::FIELD] {
                if let Some(val) = f.fields.get(k) {
                    fields.insert(k.into(), val.clone());
                }
            }
            copies.push(MoveFact { kind: "commit_seen".into(), fields });
            let old = text(f, "commit").unwrap_or("");
            for x in &crits {
                let Some(o) = crate::criterion::observed(v, x, old) else { continue };
                let mut fields = Map::new();
                fields.insert("criterion_id".into(), Value::from(x.as_str()));
                if let Some(rev) = o.fields.get("criterion_rev") {
                    fields.insert("criterion_rev".into(), rev.clone());
                }
                fields.insert("commit".into(), Value::from(nid.as_str()));
                fields.insert("tests_hash".into(), o.fields.get("tests_hash").cloned().unwrap_or(Value::Null));
                copies.push(MoveFact { kind: "tests_observed".into(), fields });
            }
        }
        let mut it = copies.into_iter();
        if let Some(first) = it.next() {
            out.push(mv("commit_seen", Value::Object(first.fields), it.collect(), "rebase")); // a rebase: every commit again, new ids, same content
        }
    }
    if let Some(tip) = cs.last() {
        let tip_id = text(tip, "commit").unwrap_or("");
        let all_done = crits.iter().all(|c| released(v, c) || matches!(state(v, c), "green" | "proven"));
        let some_green = crits.iter().any(|c| state(v, c) == "green");
        let ran = v.kind("pipeline_run_seen").any(|f| text(f, "head_commit") == Some(tip_id));
        if all_done && some_green && !ran {
            out.push(mv1("pipeline_run_seen", json!({ "head_commit": tip_id })));
        }
    }
    let _ = AGENTS; // the transfer's second conversation comes from `other_agent`
    out
}

/// The sample's found cases [draft words]: (short, given, expect, why), the first one or both.
pub const CASES_FOUND: [(&str, &str, &str, &str); 2] = [
    ("a password with a space in it opens a session", "the right password, with a space in it", "a session", "the check compares the whole string"),
    ("a user name in capitals opens the same session", "the user name in capitals and the right password", "the same session", "user names are looked up without case"),
];
/// The model the sample's case-finder ran on, and the tokens it used.
pub const CASES_MODEL: &str = "sonnet";
pub const CASES_TOKENS: u64 = 4000;
/// The sample's numbers typed on the card: the first found case [draft].
pub const CASES_NUMBERS: &str = "1";
/// The sample's case typed in the person's words [draft].
pub const CASES_TYPED: &str = "a locked account is refused even with the right password";
/// What the agent writes the person's own case expects, when the words say it [draft].
pub const CASES_PERSONS_EXPECT: &str = "as the person said";

/// The cases card's moves on `c` (the owner's plan of 2026-10-25) [draft names]: while the
/// case-finder is owed, the binary's finding (found with one case or two, none, failed); while
/// the person's card is open, each choice it offers but the person's own case, the numbers typed
/// when it found cases and a case in the person's words; and once no hold stands and cases asked
/// for are missing from the promise, the agent's promise claimed again holding them (each found
/// case as the finder gave it, the person's in their words), every field of the latest promise
/// kept.
pub fn cases_moves(v: &View, c: &str) -> Vec<Move> {
    use crate::cases;
    let mut out = Vec::new();
    if cases::owed(v, c) {
        let found = |n: usize| -> Vec<Value> {
            CASES_FOUND[..n]
                .iter()
                .enumerate()
                .map(|(i, (short, given, expect, why))| json!({ "case_id": format!("a{}", i + 1), "short": short, "given": given, "expect": expect, "why": why }))
                .collect()
        };
        for (label, outcome, cs, why) in [
            ("cases_found_one", "found", found(1), ""),
            ("cases_found_two", "found", found(2), ""),
            ("cases_none", "none", Vec::new(), "the planned cases cover it"),
            ("cases_failed", "failed", Vec::new(), "no valid answer"),
        ] {
            let f = json!({ "criterion_id": c, "outcome": outcome, "cases": cs, "why": why, "model": CASES_MODEL, "tokens": CASES_TOKENS });
            out.push(mv(cases::FOUND, f, Vec::new(), label));
        }
    }
    if cases::card_open(v, c) {
        // the person's own case is words typed (below); picked with nothing typed it is no answer
        for ch in cases::offered(v, c).into_iter().filter(|ch| *ch != cases::TYPED) {
            out.push(mv1("hold_answered", json!({ "criterion_id": c, "hold": cases::CARD, "choice": ch, "typed": "" })));
        }
        let any_found = cases::findings(v, c).last().is_some_and(|f| !cases::found_ids(f).is_empty());
        if any_found {
            out.push(mv("hold_answered", json!({ "criterion_id": c, "hold": cases::CARD, "choice": cases::TYPED, "typed": CASES_NUMBERS }), Vec::new(), "cases_typed_numbers"));
        }
        out.push(mv("hold_answered", json!({ "criterion_id": c, "hold": cases::CARD, "choice": cases::TYPED, "typed": CASES_TYPED }), Vec::new(), "cases_typed_words"));
    }
    let missing = cases::missing(v, c);
    if !cases::holds(v, c) && !missing.is_empty() {
        if let Some(p) = plan::promise_of(v, c) {
            let fs = cases::findings(v, c);
            let mut f = p.fields.clone();
            let mut cs: Vec<Value> = f.get("cases").and_then(Value::as_array).cloned().unwrap_or_default();
            for id in &missing {
                let from_finding = fs.iter().rev().find_map(|x| {
                    x.fields.get("cases").and_then(Value::as_array).and_then(|xs| xs.iter().find(|k| k.get("case_id").and_then(Value::as_str) == Some(id.as_str()))).cloned()
                });
                let case = match from_finding {
                    Some(k) => json!({ "case_id": id, "kind": "edge", "given": k.get("given").cloned().unwrap_or(Value::Null), "expect": k.get("expect").cloned().unwrap_or(Value::Null) }),
                    None => {
                        let words = cases::persons_case(v, c, id).unwrap_or_default();
                        json!({ "case_id": id, "kind": "edge", "given": words, "expect": CASES_PERSONS_EXPECT })
                    }
                };
                cs.push(case);
            }
            f.insert("cases".into(), Value::Array(cs));
            out.push(mv("promise", Value::Object(f), Vec::new(), "cases_claimed"));
        }
    }
    out
}

/// Whether every part at work still owes its holder's first act, so nothing of the work is
/// claimed yet.
fn first_act_all(v: &View) -> bool {
    let run = runnable_criteria(v);
    !run.is_empty() && run.iter().all(|a| first_act(v, a))
}

/// Whether two moves have the same kind, fields and landed facts, compared whole.
pub fn same_move(x: &Move, y: &Move) -> bool {
    let more = |m: &Move| m.more.clone().unwrap_or_default();
    x.kind == y.kind && x.fields == y.fields && more(x) == more(y)
}

/// Appends every fact of `m` through `append_raw` if `m` is admissible here.
pub fn criterion_append(log: &Log, m: &Move, at: f64, tenant: &str) -> Result<Log, String> {
    let v = View::new(log);
    if !criterion_admissible(&v).iter().any(|e| same_move(e, m)) {
        return Err(format!("{} is not admissible", m.kind));
    }
    apply_move(log, m, at, tenant).map_err(|e| e.to_string())
}
