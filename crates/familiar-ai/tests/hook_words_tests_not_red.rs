//! Tests that did not go red are told so at that moment: the PostToolUse of
//! the tests-first commit whose run passed says, after the hold's line, what
//! the person asked for, the agent's reading when it differs, what to write
//! and the command the binary runs, and what a second passing run means.

mod common;

use common::Project;
use serde_json::json;

const SESSION: &str = "7a2b3c4d-5e6f-4a1b-8c2d-3e4f5a6b7c8d";

fn planned(name: &str, reading: &str) -> Project {
    let p = Project::builder(name).session(SESSION).create();
    p.hook("UserPromptSubmit", json!({ "prompt": "make greet say hello instead of hi" }));
    let t = p.status_json()["said"][0]["for"].as_u64().expect("the prompt's t");
    p.claim(&["intent_submitted", "--for", &t.to_string(), "--text", reading]);
    p.claim(&["one_reading", "--why", "the ask names what must change and how"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    let out = p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    assert!(out.contains("familiar-ai: the plan is confirmed"), "{out}");
    p
}

/// A test that passes on the code as it is, committed alone, and the hook after the commit.
fn passing_tests_first(p: &Project) -> String {
    p.write("tests/test_c1.sh", "grep -q hi src/app.sh\n");
    let out = p.commit("c1: tests first");
    out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or_else(|| panic!("the agent is told: {out}")).to_string()
}

const HELD: &str = "familiar-ai: \"greet says hello\" · the test must fail on the code as it is, committed alone before any fix; reverting a fix does not count";
const TEACHING: [&str; 2] = [
    "  write a test that fails on the code as it is now; familiar-ai runs sh tests/test_c1.sh after each commit",
    "  a second run that passes counts as a failed try; two in a row and the person decides",
];

#[test]
fn the_commit_whose_tests_passed_is_told_the_persons_words_the_agents_reading_and_what_to_write() {
    let p = planned("reading", "greet with hello");
    let context = passing_tests_first(&p);
    assert_eq!(
        context,
        [HELD, "  asked for: make greet say hello instead of hi", "  the agent's reading: greet with hello", TEACHING[0], TEACHING[1]].join("\n")
    );
    // the stop still says it, as before
    let out = p.hook("Stop", json!({}));
    let reason = out["reason"].as_str().unwrap_or("");
    assert!(reason.ends_with(&format!("\n{}\n{}", TEACHING[0], TEACHING[1])), "{reason}");
}

#[test]
fn the_agents_reading_is_left_out_when_it_is_the_persons_words() {
    let p = planned("same", "make greet say hello instead of hi");
    let context = passing_tests_first(&p);
    assert_eq!(context, [HELD, "  asked for: make greet say hello instead of hi", TEACHING[0], TEACHING[1]].join("\n"));
}
