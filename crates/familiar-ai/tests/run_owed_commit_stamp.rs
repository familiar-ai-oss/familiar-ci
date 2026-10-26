//! A tests-first commit that landed without a hook call, and is run late by
//! the catch-up, is stamped at that commit just as a run at the tip is.

mod common;

use common::Project;
use familiar_ai::{FAMILIAR_FOLDER, TENANT};
use familiar_ai_facts::{
    AnchorShaMatched, CheckedAt, Criterion, CriterionAssigned, CriterionHead, FixLocation, FixLocationCategory, IntentSubmitted,
    Path as CriterionPath, Place, PlaceChosen, Record, TestPlanned, TestStackDetected, Will,
};
use serde_json::{json, Value};

const SESSION: &str = "5c67ae1d-1111-4222-8333-944455566688";
const APP: &str = "#!/bin/sh\ngreet() { echo hi; }\ngreet\n";
const FIXED: &str = "#!/bin/sh\ngreet() { echo hello; }\ngreet\n";
const TEST: &str = "grep -q hello src/app.sh\n";

fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).no_app().no_setup().file(".gitignore", ".familiar/record\n").file("src/app.sh", APP).create()
}

impl Project {
    fn record(&self) -> Record {
        Record::open_waiting(&self.record_path(), std::time::Duration::from_secs(5)).expect("record")
    }
}

/// Writes the plan as facts in the record, with the test command that asks git.
fn write_plan(r: &Project) {
    std::fs::create_dir_all(r.path(FAMILIAR_FOLDER)).expect("folder");
    let base = r.head();
    let record = r.record();
    let lines_hash = familiar_ai_git::lines_hash(&r.repo(), &base, "src/app.sh", 1, 3).expect("lines hash");
    let at = 1.0;
    record.append(&IntentSubmitted { text: "greet with hello".into(), sha: base.clone(), for_t: None }, at, TENANT).expect("intent");
    // definition change (the owner, 2026-10-11): light is for documentation only; a code anchor is the full path
    record.append(&familiar_ai_facts::Behavior { behavior_id: "b1".into(), text: "the greeting".into() }, at, TENANT).expect("behavior");
    record.append(&familiar_ai_facts::Promise { promise_id: "p1".into(), behavior_id: "b1".into(), accepts: "a name".into(), gives_back: "a greeting".into(), refuses: "nothing".into() }, at, TENANT).expect("promise");
    record
        .append(
            &Criterion { criterion_id: "c1".into(), text: "greet says hello".into(), path: CriterionPath::Full, behavior_id: Some("b1".into()), promise_id: Some("p1".into()), for_t: None },
            at,
            TENANT,
        )
        .expect("criterion");
    record
        .append(
            &PlaceChosen {
                criterion_id: "c1".into(),
                places: vec![Place { file_path: "src/app.sh".into(), symbol: "greet".into(), line_start: 1, line_end: 3 }],
                hold: None,
                choice: None,
            },
            at,
            TENANT,
        )
        .expect("place");
    record
        .append(
            &AnchorShaMatched {
                criterion_id: "c1".into(),
                file_path: "src/app.sh".into(),
                symbol: "greet".into(),
                line_start: 1,
                line_end: 3,
                sha: base.clone(),
                lines_hash,
                matched: true,
                checked_at: CheckedAt::Planning,
                tip_matched: None,
            },
            at,
            TENANT,
        )
        .expect("check");
    record
        .append(&TestPlanned { criterion_id: "c1".into(), file_path: "tests/test_c1.sh".into(), will: Will::Create, test_command: None }, at, TENANT)
        .expect("test planned");
    record
        .append(
            &TestStackDetected { codebase_id: "cb1".into(), project_root: "src/".into(), test_stack: "sh".into(), test_command: "sh tests/test_c1.sh".into() },
            at,
            TENANT,
        )
        .expect("stack");
    record.append(&CriterionHead { criterion_id: "c1".into() }, at, TENANT).expect("head");
    record.append(&CriterionAssigned { criterion_id: "c1".into(), agent_conversation_id: SESSION.into() }, at, TENANT).expect("assigned");
    record
        .append(&FixLocationCategory { criterion_id: "c1".into(), category: FixLocation::ThisRepository, where_: String::new(), needs: None }, at, TENANT)
        .expect("claim");
}

fn at<'a>(facts: &'a [familiar_ai_facts::Fact], kind: &str, commit: &str) -> Vec<&'a familiar_ai_facts::Fact> {
    facts
        .iter()
        .filter(|f| f.kind == kind && f.fields.get("commit") == Some(&json!(commit)) && f.fields.get("ran_in") == Some(&json!("laptop")))
        .collect()
}

/// Lands two commits before one PostToolUse call and returns the record once
/// the stamp at the tip is in.
fn landed_late(name: &str) -> (Project, String, String, Vec<familiar_ai_facts::Fact>) {
    let r = project(name);
    write_plan(&r);
    r.write("tests/test_c1.sh", TEST);
    let t1 = r.land("c1: tests first");
    r.write("src/app.sh", FIXED);
    let f1 = r.land("c1: fix");
    let mut post = r.fixture("post_tool_use_bash", json!({}));
    post["tool_input"]["command"] = Value::from("git add -A && git commit -m 'c1: tests first' && git add -A && git commit -m 'c1: fix'");
    r.hook_with("PostToolUse", &post);
    // the runs are detached: wait for the stamp at the tip
    let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let facts = loop {
        let facts = r.record().read_all().expect("read");
        if !at(&facts, "fingerprint_stamped", &f1).is_empty() || std::time::Instant::now() > until {
            break facts;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    };
    (r, t1, f1, facts)
}

#[test]
fn a_tests_first_commit_fed_late_is_run_and_stamped_at_that_commit() {
    let (_r, t1, f1, facts) = landed_late("stamped");
    let observed = |c: &str| -> Value {
        facts
            .iter()
            .find(|f| f.kind == "tests_observed" && f.fields.get("criterion_id") == Some(&json!("c1")) && f.fields.get("commit") == Some(&json!(c)))
            .and_then(|f| f.fields.get("tests_hash").cloned())
            .unwrap_or_else(|| panic!("c1 observed at {c}"))
    };
    let kinds: Vec<_> = facts.iter().map(|f| (&f.kind, f.fields.get("commit"))).collect();
    for (commit, outcome) in [(&t1, "failed"), (&f1, "passed")] {
        let results = at(&facts, "pass_please_pass", commit);
        assert_eq!(results.len(), 1, "one result at {commit}: {kinds:?}");
        assert_eq!(results[0].fields.get("outcome"), Some(&json!(outcome)), "at {commit}");
        let stamps = at(&facts, "fingerprint_stamped", commit);
        assert_eq!(stamps.len(), 1, "a result at {commit}, so a stamp there: {kinds:?}");
        assert_eq!(stamps[0].fields.get("proof_not_tampered"), Some(&observed(commit)), "the stamp is c1's observation at {commit}");
        assert!(results[0].t <= stamps[0].t, "the stamp follows its result");
    }
}

/// Ignored until `holds_of` stops reading stamps by record time: both commits
/// are seen in one hook call before any stamp exists, so c1 holds `not_stamped`.
#[test]
#[ignore = "holds.rs reads stamps by record time, so a tests-first commit fed late holds not_stamped; a rules change"]
fn a_tests_first_commit_fed_late_seals_and_reads_green() {
    let (r, _, _, _) = landed_late("green");
    let status = r.status_json();
    let c1 = &status["criteria"][0];
    assert_eq!(c1["state_name"], json!("green"), "sealed at the tests-first commit, green at the tip: {c1}");
    assert!(c1["hold"].is_null(), "no hold (not two tries not red): {c1}");
}
