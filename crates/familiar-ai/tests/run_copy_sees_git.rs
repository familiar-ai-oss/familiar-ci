//! A run at a commit, by the hook or by `verify`, happens in a copy that is a
//! real git checkout of that commit, so a test command that asks git gets the
//! commit's answer and a failing tests-first commit is recorded as failed.

mod common;

use std::time::Duration;

use common::Project;
use familiar_ai::{Surroundings, FAMILIAR_FOLDER, PLAN_FILE, TENANT};
use familiar_ai_facts::{
    AnchorShaMatched, CheckedAt, Criterion, CriterionAssigned, CriterionHead, FixLocation, FixLocationCategory, IntentSubmitted,
    Path as CriterionPath, Place, PlaceChosen, Record, TestPlanned, TestStackDetected, Will,
};
use familiar_ai_plan::Claim;
use serde_json::{json, Value};

const SESSION: &str = "5c67ae1d-1111-4222-8333-944455566677";
const APP: &str = "#!/bin/sh\ngreet() { echo hi; }\ngreet\n";
const FIXED: &str = "#!/bin/sh\ngreet() { echo hello; }\ngreet\n";
const TEST: &str = "grep -q hello src/app.sh\n";

/// A test command that runs whatever test files git says changed.
fn git_command(base: &str) -> String {
    format!("for f in $(git diff --name-only {base} HEAD -- tests); do sh $f || exit 1; done")
}

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
            &TestStackDetected { codebase_id: "cb1".into(), project_root: "src/".into(), test_stack: "sh".into(), test_command: git_command(&base) },
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

#[test]
fn a_tests_first_commit_whose_test_fails_is_recorded_failed_when_the_command_asks_git() {
    let r = project("hook");
    write_plan(&r);
    // the tests-first commit lands: its test fails against today's code
    r.write("tests/test_c1.sh", TEST);
    let t1 = r.land("c1: tests first");
    let mut post = r.fixture("post_tool_use_bash", json!({}));
    post["tool_input"]["command"] = Value::from("git add -A && git commit -m 'c1: tests first'");
    let out = r.hook_with("PostToolUse", &post);
    let context = out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("").to_string();

    let facts = r.record().read_all().expect("read");
    let result: Vec<&familiar_ai_facts::Fact> = facts.iter().filter(|f| f.kind == "pass_please_pass" && f.fields.get("commit") == Some(&json!(t1))).collect();
    assert_eq!(result.len(), 1, "one result for the tests-first commit: {context}");
    assert_eq!(result[0].fields.get("outcome"), Some(&json!("failed")), "git in the copy answers for the commit, so the test ran and failed: {:?}", result[0].fields);
    assert_eq!(context, "familiar-ai: ran the tests of \"greet says hello\": they fail, as they should before the fix · stamped\n  the code goes on src/app.sh greet lines 1–3; the test stays as it is");
}

fn around(dir: &std::path::Path) -> Surroundings {
    Surroundings {
        now: 1.0,
        project_dir: Some(dir.to_path_buf()),
        cwd: Some(dir.to_path_buf()),
        session_id: None,
        data_dir: None,
        record_wait: Duration::from_secs(1),
        test_run_wait: Duration::from_secs(20),
        run_inline: Duration::ZERO,
        runner: None,
        judge_dir: None,
        judge_wait: Duration::ZERO,
        investigator: None,
        conversation: None,
        bench: false,
    }
}

#[test]
fn verify_sees_a_tests_first_commit_that_fails_red_when_the_command_asks_git() {
    let r = project("verify");
    let base = r.head();
    let claims: Vec<(&str, Value)> = vec![
        ("intent_submitted", json!({"text": "greet with hello", "sha": base})),
        ("behavior", json!({"behavior_id": "b1", "text": "the greeting"})),
        ("promise", json!({"promise_id": "p1", "behavior_id": "b1", "accepts": "a name", "gives_back": "a greeting", "refuses": "nothing"})),
        ("criterion", json!({"criterion_id": "c1", "text": "greet says hello", "path": "full", "behavior_id": "b1", "promise_id": "p1"})),
        ("place_chosen", json!({"criterion_id": "c1", "places": [{"file_path": "src/app.sh", "symbol": "greet", "line_start": 1, "line_end": 3}]})),
        ("test_planned", json!({"criterion_id": "c1", "file_path": "tests/test_c1.sh", "will": "create"})),
        ("test_stack_detected", json!({"codebase_id": "fx", "project_root": "src/", "test_stack": "sh", "test_command": git_command(&base)})),
        ("criterion_head", json!({"criterion_id": "c1"})),
        ("criterion_assigned", json!({"criterion_id": "c1", "agent_conversation_id": "s1"})),
        ("fix_location_category", json!({"criterion_id": "c1", "category": "this_repository", "where": ""})),
        ("test_written", json!({"criterion_id": "c1", "file_path": "tests/test_c1.sh", "test_name": ""})),
    ];
    let claims: Vec<Claim> = claims.iter().map(|(k, f)| Claim::new(k, f.as_object().cloned().unwrap_or_default()).unwrap_or_else(|e| panic!("{k}: {e}"))).collect();
    familiar_ai_plan::write(&r.path(FAMILIAR_FOLDER).join(PLAN_FILE), &claims).expect("plan");
    r.write("tests/test_c1.sh", TEST);
    r.land("c1: tests first");
    r.write("src/app.sh", FIXED);
    r.land("c1: fix");

    let out = familiar_ai::run(["verify"], "", &around(&r.repo()));
    let text = format!("{}{}", out.stdout_text, out.stderr_text);
    assert_eq!(out.exit_code, 0, "the tests-first run was red, the head green, the teeth bit: {text}");
    assert!(text.contains("proven · red at ") && text.contains("(exit 1) · green at "), "{text}");
}
