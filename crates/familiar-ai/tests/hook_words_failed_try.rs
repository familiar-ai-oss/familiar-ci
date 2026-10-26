//! A failed try is told at that moment: the PostToolUse of a code commit after
//! the seal whose run failed, and that counts as the first try, says the tests
//! still fail and what one more failed try brings, what the person asked for,
//! where the code goes, and the last forty lines of the kept run.

mod common;

use common::Project;
use serde_json::json;

const SESSION: &str = "9b8a7c6d-5e4f-4321-8a0b-1c2d3e4f5a6b";

/// A test that prints 45 lines, then checks greet says hello.
const TEST: &str = "i=1; while [ $i -le 45 ]; do echo \"line $i\"; i=$((i+1)); done\nsh src/app.sh | grep -qx hello\n";

fn sealed(name: &str) -> Project {
    let p = Project::builder(name).session(SESSION).create();
    p.hook("UserPromptSubmit", json!({ "prompt": "make greet say hello instead of hi" }));
    let t = p.status_json()["said"][0]["for"].as_u64().expect("the prompt's t");
    p.claim(&["intent_submitted", "--for", &t.to_string(), "--text", "greet with hello"]);
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
    p.write("tests/test_c1.sh", TEST);
    let out = p.commit("c1: tests first");
    assert_eq!(out["hookSpecificOutput"]["additionalContext"], "familiar-ai: ran the tests of \"greet says hello\": they fail, as they should before the fix · stamped\n  the code goes on src/app.sh greet lines 1–3; the test stays as it is");
    p
}

/// A fix on the anchor that still fails, committed; what the agent is told.
fn failed_fix(p: &Project, word: &str) -> String {
    p.write("src/app.sh", &format!("#!/bin/sh\ngreet() {{ echo {word}; }}\ngreet\n"));
    let out = p.commit(&format!("c1: greet says {word}"));
    out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("").to_string()
}

#[test]
fn the_first_failed_try_is_told_with_the_persons_words_the_place_and_the_last_lines_of_the_run() {
    let p = sealed("first");
    let context = failed_fix(&p, "howdy");
    let mut want = vec![
        "familiar-ai: ran the tests of \"greet says hello\": they still fail · try 1 of 2; two failed tries in a row send a stronger model".to_string(),
        "  asked for: make greet say hello instead of hi".to_string(),
        "  the agent's reading: greet with hello".to_string(),
        "  the code goes on src/app.sh greet lines 1–3; the test stays as it is".to_string(),
        "  the last lines of the run:".to_string(),
    ];
    // the last forty of the run's 45 lines
    want.extend((6..=45).map(|n| format!("    line {n}")));
    assert_eq!(context, want.join("\n"));
    assert_eq!(p.state_name("c1"), "sealed");
}

#[test]
fn the_second_failed_try_is_not_told_as_a_try_its_hold_says_it() {
    let p = sealed("second");
    failed_fix(&p, "howdy");
    let context = failed_fix(&p, "hey");
    assert!(!context.contains("they still fail · try"), "{context}");
    assert!(context.starts_with("familiar-ai: \"greet says hello\" · two tries failed"), "{context}");
}
