//! CI classifies too (plan §6.5; lane S9d-ci). Every verify run gets an id: the action reserves
//! it (`run_reserved {by: pipeline}`), ends it, translates it through the same tables, and only
//! then writes each test's result naming the run. The pipeline's exemption from the grandfather
//! cut ends: a pipeline result without `run_id` after the record's first translation is Unknown,
//! as the laptop's is.
//!
//! CI's re-earn (plan §7.1 "On CI"): the approval only from a witness (verify appends those as
//! `hold_answered` after the commit it was answered at). The re-earn commit is the part's first
//! mirrored laptop stamp after that commit, and it moves there only when CI's own run of the part
//! at its tests-first commit crashed, the dependency
//! (another part stamped there) is green there by CI's own runs (condition 1), the part's tests
//! there are its proof (2), and no commit from T to it touched the part's anchor (3).

use familiar_ai_facts::{append_raw, Log};
use familiar_ai_rules::outcome::{result_family, Family};
use familiar_ai_rules::sample::{apply_move, criterion_fixture, Move};
use familiar_ai_rules::{action, moves, View};
use serde_json::{json, Value};

const AT: f64 = 0.0;
const TENANT: &str = "_default_";

fn raw(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { unreachable!() };
    append_raw(log, kind, m, AT, TENANT).unwrap_or_else(|e| panic!("append {kind}: {e}"))
}

fn text<'a>(m: &'a Move, k: &str) -> &'a str {
    m.fields.get(k).and_then(Value::as_str).unwrap_or("")
}

fn test_of(c: &str) -> String {
    format!("tests/test_{c}.sh")
}

fn place_of(c: &str) -> &'static str {
    if c == "c1" { "src/app.sh" } else { "src/lib.sh" }
}

fn hash_of(c: &str) -> String {
    format!("H{c}")
}

/// The plan as verify reads it: c1 (the part) and c3 (its dependency), each with its own
/// command, and the laptop's stamps: c3 red at k2 and green at k3, c1 re-earned red at k3 and
/// green at k4.
fn plan() -> Log {
    let mut log = raw(&Vec::new(), "intent_submitted", json!({ "text": "greet with hello", "sha": "k0" }));
    for c in ["c1", "c3"] {
        log = raw(&log, "criterion", json!({ "criterion_id": c, "text": format!("{c} works"), "path": "full" }));
        log = raw(&log, "place_chosen", json!({ "criterion_id": c, "places": [{ "file_path": place_of(c), "symbol": "f", "line_start": 1, "line_end": 3 }] }));
        log = raw(&log, "test_planned", json!({ "criterion_id": c, "file_path": test_of(c), "will": "create", "test_command": format!("sh {}", test_of(c)) }));
        log = raw(&log, "test_written", json!({ "criterion_id": c, "file_path": test_of(c), "test_name": "" }));
    }
    for (c, id) in [("c3", "k2"), ("c3", "k3"), ("c1", "k3"), ("c1", "k4")] {
        log = raw(&log, "fingerprint_stamped", json!({ "criterion_id": c, "commit": id, "proof_not_tampered": hash_of(c), "ran_in": "laptop" }));
    }
    log
}

/// A commit, observed for every part whose test exists at it.
fn commit(log: &Log, id: &str, files: &[&str], tests: bool, code: bool, overlap: &[&str]) -> Log {
    let mut log = raw(log, "commit_seen", json!({ "commit": id, "files": files, "touches_tests": tests, "touches_code": code, "anchor_overlap": overlap }));
    for c in ["c1", "c3"] {
        let seen = log.iter().any(|f| f.kind == "tests_observed" && f.criterion_id() == Some(c));
        if seen || files.contains(&test_of(c).as_str()) {
            log = raw(&log, "tests_observed", json!({ "criterion_id": c, "criterion_rev": "R", "commit": id, "tests_hash": hash_of(c) }));
        }
    }
    log
}

/// The witnessed approval, appended right after the commit it was answered at.
fn approval(log: &Log, at: &str) -> Log {
    raw(log, "hold_answered", json!({ "criterion_id": "c1", "hold": "crashed", "choice": "a_dependency", "typed": "", "run_id": "", "commit": at, "approval": "7" }))
}

/// k1 c1's test alone (T), the approval answered there, k2 c3's test alone, k3 c3's fix
/// (G), `between` (commits landed before G), k4 c1's fix, and the run on the head.
fn history(between: &[(&str, &[&str])]) -> Log {
    let mut log = commit(&plan(), "k1", &["tests/test_c1.sh"], true, false, &[]);
    log = approval(&log, "k1");
    log = commit(&log, "k2", &["tests/test_c3.sh"], true, false, &[]);
    for (id, overlap) in between {
        // its own content: a commit like k4 (c1's place alone) would be k4 by content
        log = commit(&log, id, &[place_of("c1"), "src/app_notes.sh"], false, true, overlap);
    }
    log = commit(&log, "k3", &["src/lib.sh"], false, true, &["c3"]);
    log = commit(&log, "k4", &["src/app.sh"], false, true, &["c1"]);
    raw(&log, "pipeline_run_seen", json!({ "head_commit": "k4" }))
}

fn word(label: &str) -> &'static str {
    if label == "passed" { "passed" } else { "failed" }
}

/// The run reserved under `rid`: its part, its commit, and whether it is the teeth run.
fn run_of(log: &Log, rid: &str) -> (String, String, bool) {
    let v = View::new(log);
    let r = log.iter().find(|f| f.kind == "run_reserved" && f.fields.get("run_id").and_then(Value::as_str) == Some(rid)).expect("reserved");
    let c = r.criterion_id().unwrap_or("").to_string();
    let k = r.fields.get("commit").and_then(Value::as_str).unwrap_or("").to_string();
    let teeth = action::run_id(&v, &c, &k, true) == rid;
    (c, k, teeth)
}

/// The action's moves made until none is left, each run translated as `label` says; the
/// order of the reservations, as (part, commit, teeth).
fn drive(log: &Log, label: impl Fn(&str, &str, bool) -> &'static str) -> (Log, Vec<(String, String, bool)>) {
    let mut log = log.clone();
    let mut order = Vec::new();
    for _ in 0..400 {
        let ms = action::action_admissible(&View::new(&log));
        let Some(first) = ms.first() else { return (log, order) };
        let pick = match first.kind.as_str() {
            "run_ended" | "test_log_translation" => {
                let (c, k, teeth) = run_of(&log, text(first, "run_id"));
                let l = label(&c, &k, teeth);
                let (key, want) = if first.kind == "run_ended" { ("outcome", word(l)) } else { ("label", l) };
                ms.iter().find(|m| text(m, key) == want).unwrap_or_else(|| panic!("no {} {key} {want} in {ms:?}", first.kind)).clone()
            }
            _ => first.clone(),
        };
        if pick.kind == "run_reserved" {
            assert_eq!(text(&pick, "by"), "pipeline", "{pick:?}");
            let teeth = action::run_id(&View::new(&log), text(&pick, "criterion_id"), text(&pick, "commit"), true) == text(&pick, "run_id");
            order.push((text(&pick, "criterion_id").to_string(), text(&pick, "commit").to_string(), teeth));
        }
        log = action::action_append(&log, &pick.kind, &pick.fields, AT, TENANT).unwrap_or_else(|e| panic!("{e}: {pick:?}"));
    }
    panic!("the action never settled")
}

/// The happy history's runs: c1 crashes at k1, red at k3, green at k4 and red with its lines put
/// back; c3 red at k2 and green from k3 on.
fn happy(c: &str, k: &str, teeth: bool) -> &'static str {
    match (c, k, teeth) {
        (_, _, true) => "tests_failed",
        ("c1", "k1", _) => "crashed",
        ("c1", "k3", _) | ("c3", "k2", _) => "tests_failed",
        _ => "passed",
    }
}

#[test]
fn every_pipeline_run_is_reserved_ended_and_translated_before_its_results_name_it() {
    let log = commit(&plan(), "k1", &["tests/test_c1.sh"], true, false, &[]);
    let log = raw(&log, "pipeline_run_seen", json!({ "head_commit": "k1" }));
    let v = View::new(&log);
    let ms = action::action_admissible(&v);
    assert_eq!(ms.len(), 1, "{ms:?}");
    assert_eq!(ms[0].kind, "run_reserved");
    let rid = action::run_id(&v, "c1", "k1", false);
    assert_eq!(text(&ms[0], "run_id"), rid);
    assert_eq!(text(&ms[0], "by"), "pipeline");
    assert_eq!(text(&ms[0], "command"), "sh tests/test_c1.sh");
    assert_ne!(rid, action::run_id(&v, "c1", "k1", true), "the teeth run is its own");
    let log = apply_move(&log, &ms[0], AT, TENANT).expect("reserved");
    let ms = action::action_admissible(&View::new(&log));
    assert!(ms.iter().all(|m| m.kind == "run_ended" && text(m, "run_id") == rid), "{ms:?}");
    let outcomes: Vec<&str> = ms.iter().map(|m| text(m, "outcome")).collect();
    assert_eq!(outcomes, ["passed", "failed", "did_not_run"]);
    // the binary's end carries its evidence: the offered end with the run's own fields
    let mut end = ms[1].fields.clone();
    end.insert("exit_code".into(), json!(1));
    end.insert("stderr_tail".into(), json!(""));
    end.insert("stdout_tail".into(), json!("no hello\n"));
    let log = action::action_append(&log, "run_ended", &end, AT, TENANT).expect("the binary's end is admissible");
    let ms = action::action_admissible(&View::new(&log));
    assert!(ms.iter().all(|m| m.kind == "test_log_translation" && text(m, "run_id") == rid && text(m, "criterion_id") == "c1" && text(m, "commit") == "k1"), "{ms:?}");
    assert!(!ms.iter().any(|m| text(m, "label") == "passed"), "a failed run is not translated as passed");
    // the binary's own reading, a label the walks do not branch on, with its evidence
    let mut t = ms[0].fields.clone();
    t.insert("label".into(), json!("collection_error"));
    t.insert("evidence".into(), json!("ERROR collecting tests/test_c1.sh"));
    t.insert("failing_tests".into(), json!(["tests/test_c1.sh"]));
    let mut wrong = t.clone();
    wrong.insert("label".into(), json!("passed"));
    assert!(action::action_append(&log, "test_log_translation", &wrong, AT, TENANT).is_err(), "a failed run's translation never reads passed");
    let mut other = t.clone();
    other.insert("criterion_id".into(), json!("c3"));
    assert!(action::action_append(&log, "test_log_translation", &other, AT, TENANT).is_err(), "only the reserved run's part");
    let log = action::action_append(&log, "test_log_translation", &t, AT, TENANT).expect("the binary's translation is admissible");
    let ms = action::action_admissible(&View::new(&log));
    assert_eq!(ms.len(), 1, "one result, the run's own: {ms:?}");
    assert_eq!(ms[0].kind, "pass_please_pass");
    assert_eq!(ms[0].fields.get("run_id"), Some(&json!(rid)), "the pipeline's result names its run");
    assert_eq!(text(&ms[0], "outcome"), "failed");
    assert_eq!(text(&ms[0], "ran_in"), "pipeline");
    let log = apply_move(&log, &ms[0], AT, TENANT).expect("result");
    let v = View::new(&log);
    let r = log.last().expect("the result");
    assert_eq!(result_family(&v, "c1", r), Family::TestBroken, "read through its translation, not its word");
    assert!(!action::red(&v, "c1"), "a collection error is no red");
}

#[test]
fn a_pipeline_result_without_run_id_after_the_first_translation_is_unknown() {
    let log = commit(&plan(), "k1", &["tests/test_c1.sh"], true, false, &[]);
    let result = json!({ "criterion_id": "c1", "file_path": test_of("c1"), "commit": "k1", "outcome": "failed", "ran_in": "pipeline", "with_anchor_reverted": false });
    let before = raw(&log, "pass_please_pass", result.clone());
    let r = before.last().expect("result").clone();
    assert_eq!(result_family(&View::new(&before), "c1", &r), Family::ValidRed, "before any translation: legacy, read by its word");
    let t = json!({ "run_id": "r1", "criterion_id": "c1", "commit": "k1", "by": "regex", "label": "tests_failed", "evidence": "", "failing_tests": [], "classified_on": "full" });
    let translated = raw(&before, "test_log_translation", t);
    assert_eq!(result_family(&View::new(&translated), "c1", &r), Family::ValidRed, "the legacy result keeps its reading");
    let after = raw(&translated, "pass_please_pass", result);
    let late = after.last().expect("result");
    assert_eq!(result_family(&View::new(&after), "c1", late), Family::Unknown, "a pipeline result with no run after the first translation is Unknown, like the laptop's");
}

#[test]
fn the_models_pipeline_step_names_a_run_once_the_record_translates() {
    // the laptop's runs through the sample's run batches, so the record translates
    let mut log = criterion_fixture(&["c2"], true, true, AT, TENANT).expect("fixture");
    let ran = |log: &Log, label: &str| -> Log {
        let v = View::new(log);
        let m = moves::criterion_admissible(&v)
            .into_iter()
            .find(|m| m.label.as_deref() == Some("ran") && m.more.as_ref().is_some_and(|more| more.iter().any(|f| f.kind == "test_log_translation" && f.fields.get("label") == Some(&json!(label)))))
            .unwrap_or_else(|| panic!("no ran {label}"));
        apply_move(log, &m, AT, TENANT).expect("ran")
    };
    let step = |log: &Log, label: &str| -> Log {
        let v = View::new(log);
        let m = moves::criterion_admissible(&v).into_iter().find(|m| m.label.as_deref() == Some(label)).unwrap_or_else(|| panic!("no {label}"));
        apply_move(log, &m, AT, TENANT).expect(label)
    };
    for l in ["tests_only", "test_written"] {
        log = step(&log, l);
    }
    log = ran(&log, "tests_failed");
    log = step(&log, "fingerprint_stamped");
    log = step(&log, "code");
    log = ran(&log, "passed");
    log = step(&log, "fingerprint_stamped");
    log = step(&log, "pipeline_run_seen");
    let v = View::new(&log);
    let pipeline: Vec<Move> = moves::criterion_admissible(&v)
        .into_iter()
        .filter(|m| m.kind == "run_reserved" || (m.kind == "pass_please_pass" && text(m, "ran_in") == "pipeline"))
        .collect();
    assert!(!pipeline.is_empty(), "the pipeline's step is offered");
    for m in &pipeline {
        assert_eq!(m.kind, "run_reserved", "no pipeline result without its run: {m:?}");
        assert_eq!(text(m, "by"), "pipeline");
        let more = m.more.as_deref().unwrap_or(&[]);
        assert!(more.iter().any(|f| f.kind == "test_log_translation"), "{m:?}");
        for f in more.iter().filter(|f| f.kind == "pass_please_pass") {
            assert_eq!(f.fields.get("run_id"), m.fields.get("run_id"), "{m:?}");
        }
    }
}

#[test]
fn ci_re_earns_the_red_at_the_mirrored_stamp_after_its_own_crash_and_the_dependency_green() {
    let (log, order) = drive(&history(&[]), happy);
    let v = View::new(&log);
    assert_eq!(action::tests_first(&v, "c1"), Some("k3"), "the re-earn commit: c1's first mirrored stamp after the approval's commit");
    assert!(action::proven(&v, "c1"), "{order:?}");
    assert!(action::proven(&v, "c3"));
    let at = |x: (&str, &str, bool)| order.iter().position(|o| (o.0.as_str(), o.1.as_str(), o.2) == x).unwrap_or_else(|| panic!("{x:?} never ran: {order:?}"));
    assert_eq!(at(("c1", "k1", false)), 0, "CI's own run at T comes first: the crash is read from it");
    assert!(at(("c3", "k3", false)) < at(("c1", "k3", false)), "the dependency runs at the stamp's commit before the part's red there: {order:?}");
}

#[test]
fn no_crash_at_t_on_ci_moves_nothing() {
    let (log, _) = drive(&history(&[]), |c, k, teeth| if (c, k, teeth) == ("c1", "k1", false) { "tests_failed" } else { happy(c, k, teeth) });
    assert_eq!(action::tests_first(&View::new(&log), "c1"), Some("k1"), "a valid red at T is the seal; no re-earn");
}

#[test]
fn the_dependency_not_green_by_cis_own_runs_moves_nothing() {
    let (log, _) = drive(&history(&[]), |c, k, teeth| if (c, k, teeth) == ("c3", "k3", false) { "tests_failed" } else { happy(c, k, teeth) });
    let v = View::new(&log);
    assert_eq!(action::tests_first(&v, "c1"), Some("k1"), "condition 1, from CI's own run of the dependency at the stamp's commit");
    assert!(!action::proven(&v, "c1"), "a crash at T is no red");
}

#[test]
fn an_anchor_touched_between_t_and_the_stamp_moves_nothing() {
    let (log, _) = drive(&history(&[("kx", &["c1"])]), happy);
    let v = View::new(&log);
    assert_eq!(action::tests_first(&v, "c1"), Some("k1"), "condition 3");
    assert!(!action::proven(&v, "c1"));
}

/// A pipeline run that did not run (ended `did_not_run`) is no red and no green, whatever its
/// translation reads: the lifecycle never reads such a result, so the action does not either
/// (lane s9d-ci, the action walks' R3).
#[test]
fn a_pipeline_run_that_did_not_run_is_no_red_whatever_its_translation() {
    let log = commit(&plan(), "k1", &["tests/test_c1.sh"], true, false, &[]);
    let log = raw(&log, "pipeline_run_seen", json!({ "head_commit": "k1" }));
    let ms = action::action_admissible(&View::new(&log));
    let log = apply_move(&log, &ms[0], AT, TENANT).expect("reserved");
    let ms = action::action_admissible(&View::new(&log));
    let end = ms.iter().find(|m| text(m, "outcome") == "did_not_run").expect("did_not_run offered").clone();
    let log = apply_move(&log, &end, AT, TENANT).expect("ended");
    let ms = action::action_admissible(&View::new(&log));
    let t = ms.iter().find(|m| m.kind == "test_log_translation" && text(m, "label") == "tests_failed").expect("a translation").clone();
    let log = apply_move(&log, &t, AT, TENANT).expect("translated");
    let ms = action::action_admissible(&View::new(&log));
    let log = apply_move(&log, &ms[0], AT, TENANT).expect("result");
    let v = View::new(&log);
    assert_eq!(action::result(&v, "c1", &test_of("c1"), "k1", false), Some("other"));
    assert!(!action::red(&v, "c1"));
}

/// The sample's laptop run is named by the laptop's own runs at that commit: a pipeline run
/// reserved there is CI's and changes no laptop run's id (lane s9d-ci: the action walks' unsound,
/// where an unseated pipeline run's reservation renamed the laptop's next run).
#[test]
fn the_laptops_run_id_does_not_count_the_pipelines_runs() {
    let mut log = criterion_fixture(&["c2"], true, true, AT, TENANT).expect("fixture");
    for l in ["tests_only", "test_written"] {
        let v = View::new(&log);
        let m = moves::criterion_admissible(&v).into_iter().find(|m| m.label.as_deref() == Some(l)).expect(l);
        log = apply_move(&log, &m, AT, TENANT).expect(l);
    }
    let rid = |log: &Log| -> Vec<String> {
        let v = View::new(log);
        moves::criterion_admissible(&v).iter().filter(|m| m.label.as_deref() == Some("ran")).map(|m| text(m, "run_id").to_string()).collect()
    };
    let before = rid(&log);
    assert!(!before.is_empty(), "the laptop's run is offered");
    let tip = log.iter().rev().find(|f| f.kind == "commit_seen").and_then(|f| f.fields.get("commit")).and_then(Value::as_str).expect("tip").to_string();
    let after = raw(&log, "run_reserved", json!({ "run_id": "pipeline-9-c2-x-plain", "criterion_id": "c2", "criterion_rev": "R", "commit": tip, "command": "x", "resources": [], "wait": "0", "by": "pipeline" }));
    assert_eq!(rid(&after), before);
}
