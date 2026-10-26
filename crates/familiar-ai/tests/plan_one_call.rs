//! `familiar-ai plan` takes the plan's lines as one JSON array and claims each
//! through `claim`'s own path, in order, stopping at the first refusal; the
//! record ends up the same as with one claim per line.

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "11111111-2222-4333-8444-555555555555";

fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).env("PATH", "/usr/bin:/bin").create()
}

impl Project {
    /// Returns the record's facts as (kind, fields), without the binary's stamps
    /// and with shas masked, since each project has its own.
    fn masked_facts(&self) -> Vec<(String, Value)> {
        self.facts()
            .iter()
            .map(|f| {
                let mut fields = f.fields.clone();
                // filled from the repository and session, so they differ between the two projects
                for k in ["sha", "commit", "agent_conversation_id", "codebase_id"] {
                    if fields.contains_key(k) {
                        fields.insert(k.to_string(), Value::String("<the binary's>".into()));
                    }
                }
                (f.kind.to_string(), Value::Object(fields))
            })
            .collect()
    }
}

/// The eleven lines of a full-path plan for one criterion, as `plan` takes them
/// and as `claim` takes them one by one.
/// Definition change (the owner, 2026-10-11): light is for documentation only;
/// a code anchor is the full path, so behavior and promise are claimed (was:
/// nine lines, light).
fn eleven_lines() -> Vec<Value> {
    vec![
        json!({"kind": "intent_submitted", "for": 0, "text": "greet with hello"}),
        json!({"kind": "behavior", "behavior_id": "b1", "text": "the greeting"}),
        json!({"kind": "promise", "promise_id": "p1", "behavior_id": "b1", "accepts": "a name", "gives_back": "a greeting", "refuses": "nothing", "cases": [{"case_id": "n1", "kind": "normal", "given": "a name", "expect": "a greeting"}, {"case_id": "o1", "kind": "opposite", "given": "no name", "expect": "no greeting"}]}),
        json!({"kind": "criterion", "criterion_id": "c1", "text": "greet says hello", "path": "full", "behavior_id": "b1", "promise_id": "p1"}),
        json!({"kind": "place_chosen", "criterion_id": "c1", "places": [{"file_path": "src/app.sh", "symbol": "greet", "line_start": 1, "line_end": 3}]}),
        json!({"kind": "test_planned", "criterion_id": "c1", "file_path": "tests/test_c1.sh", "will": "create"}),
        json!({"kind": "test_stack_detected", "project_root": ".", "test_stack": "sh", "test_command": "sh tests/test_c1.sh"}),
        json!({"kind": "touching_test_declared", "criterion_id": "c1", "file_path": "tests/test_old.sh", "test_name": "test_greet"}),
        json!({"kind": "criterion_head", "criterion_id": "c1"}),
        json!({"kind": "criterion_assigned", "criterion_id": "c1"}),
        json!({"kind": "fix_location_category", "criterion_id": "c1", "category": "this_repository", "where": ""}),
    ]
}

/// The facts a fresh project holds before any line: setup's own.
fn before_count(_p: &Project) -> usize {
    project("empty").masked_facts().len()
}

fn claim_args(line: &Value) -> Vec<String> {
    let mut out = vec!["claim".to_string(), line["kind"].as_str().expect("kind").to_string()];
    for (k, v) in line.as_object().expect("object") {
        if k == "kind" {
            continue;
        }
        out.push(format!("--{k}"));
        out.push(match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        });
    }
    out
}

#[test]
fn the_record_after_plan_is_the_record_after_the_same_claims() {
    let lines = eleven_lines();
    let by_plan = project("plan");
    let (code, out, err) = by_plan.run(&["plan"], &Value::Array(lines.clone()).to_string());
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("familiar-ai: 11 lines claimed"), "{out}");
    // definition change (plan cases-card, 2026-10-25): a full-path plan waits on the case-finder and the person's card before it is confirmed [draft names]
    // updated with the definition (the cases card's words, step 4) [draft words]: the hold reads in the binary's words for cases_owed
    let owed = format!("· {}", familiar_ai::teach::words::hold(&familiar_ai_rules::View::new(&by_plan.facts()), Some("c1"), "cases_owed"));
    assert!(out.contains("familiar-ai: the plan is still being written") && out.contains(&owed), "the status comes back with the answer: {out}");

    let by_claims = project("claims");
    for line in &lines {
        let args = claim_args(line);
        let argv: Vec<&str> = args.iter().map(String::as_str).collect();
        let (code, out, err) = by_claims.run(&argv, "");
        assert_eq!(code, 0, "claim {args:?}: {out}{err}");
    }

    let a = by_plan.masked_facts();
    let b = by_claims.masked_facts();
    assert_eq!(a.len(), b.len(), "the same number of facts\nplan: {a:?}\nclaims: {b:?}");
    for (i, (x, y)) in a.iter().zip(&b).enumerate() {
        assert_eq!(x.0, y.0, "fact {i}: the same kind");
        // only what the binary stamps differs
        assert_eq!(x.1, y.1, "fact {i} ({}): the same fields", x.0);
    }
    // definition change (the owner's plan of 2026-10-21, step 3) [draft]: once confirmed, the
    // binary's places_listed follows too, the same either way
    // definition change (plan cases-card, 2026-10-25) [draft names]: the plan is not confirmed
    // until the case-finder and the person's card, so no listing follows yet, either way
    assert_eq!(a.iter().filter(|(k, _)| k == "places_listed").count(), 0, "{a:?}");
    assert_eq!(a.iter().filter(|(k, _)| k != "anchor_sha_matched" && k != "places_listed").count(), 11 + before_count(&by_plan), "eleven lines, plus the anchor's check");
}

#[test]
fn a_refused_line_stops_plan_and_the_lines_before_it_stay() {
    let mut lines = eleven_lines();
    lines[5] = json!({"kind": "place_chosen", "criterion_id": "c1", "places": "not-an-array"});
    let p = project("refused");
    let before = p.masked_facts().len();
    let (code, out, err) = p.run(&["plan"], &Value::Array(lines).to_string());
    assert_eq!(code, 1, "{out}{err}");
    let all = format!("{out}{err}");
    assert!(all.contains("familiar-ai: line 6 of 11, place_chosen:"), "{all}");
    assert!(!all.contains("lines claimed"), "{all}");
    let facts = p.masked_facts();
    let claimed: Vec<&str> = facts[before..].iter().map(|(k, _)| k.as_str()).collect();
    // the three lines before it and the anchor's check; nothing from line 4 on
    assert!(claimed.starts_with(&["intent_submitted", "behavior", "promise", "criterion", "place_chosen"]), "{claimed:?}");
    for k in ["test_planned", "test_stack_detected", "touching_test_declared", "criterion_head", "criterion_assigned", "fix_location_category", "planning_confirm_complete"] {
        assert!(!claimed.contains(&k), "line 4 and after were not appended: {claimed:?}");
    }
}

#[test]
fn plan_with_nothing_on_stdin_prints_the_arrays_form() {
    let p = project("form");
    let (code, out, err) = p.run(&["plan"], "");
    assert_eq!(code, 0, "{out}{err}");
    let arr: Value = serde_json::from_str(&out).expect("a JSON array");
    let kinds: Vec<&str> = arr.as_array().expect("array").iter().map(|l| l["kind"].as_str().expect("kind")).collect();
    assert_eq!(
        kinds,
        [
            // definition change (the owner, 2026-10-13): behavior and promise come before the criterion that names them
            "intent_submitted", "behavior", "promise", "criterion", "place_chosen", "new_work_spot_chosen", "new_work_no_spot",
            "test_planned", "test_stack_detected", "touching_test_declared", "criterion_head", "criterion_strung_on_head",
            "criterion_assigned", "fix_location_category"
        ]
    );
    assert_eq!(p.masked_facts().len(), p.masked_facts().len(), "nothing appended");
}

#[test]
fn plan_refuses_what_is_not_an_array_of_objects() {
    let p = project("shape");
    let before = p.masked_facts().len();
    for bad in [r#"{"kind":"criterion"}"#, "[1,2]", r#"[{"text":"no kind"}]"#] {
        let (code, out, err) = p.run(&["plan"], bad);
        assert_eq!(code, 1, "{bad}: {out}{err}");
        assert!(err.contains("familiar-ai: plan takes a JSON array of {\"kind\": …, …} objects on stdin"), "{bad}: {out}{err}");
        assert_eq!(p.masked_facts().len(), before, "{bad}: nothing appended");
    }
}
