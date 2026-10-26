//! The owner (2026-10-16, S8g): a `test_stack_detected` claim whose command replaces the
//! command under an already sealed part is told with a warning naming those parts, never
//! refused. (Stacks are per project root: the latest claim at a root is every part's command
//! there, so the sealed part reads "the test command changed after the test was sealed".)

mod common;

use common::Project;
use serde_json::json;

const SESSION: &str = "9f4b1a5c-3e95-40c2-ae8b-5d6e7f8091a2";

fn sealed(name: &str) -> Project {
    let p = Project::builder(name).session(SESSION).no_app().file("src/app.sh", "#!/bin/sh\ngreet() { echo hi; }\ngreet\n").env("FAMILIAR_AI_RUN_INLINE_MS", "60000").create();
    p.hook("UserPromptSubmit", json!({ "prompt": "make greet say hello" }));
    let t = p.facts().iter().find(|f| f.kind == "person_said").map(|f| f.t).expect("the prompt").to_string();
    p.claim(&["intent_submitted", "--for", &t, "--text", "greet says hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1", "--agent_conversation_id", SESSION]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    // a red that shows its check (H-2, lane H5: a silent exit 1 is can't tell)
    p.write("tests/test_c1.sh", "grep -q hello src/app.sh\n");
    p.commit("c1: tests first");
    assert!(p.facts().iter().any(|f| f.kind == "fingerprint_stamped"), "c1 is sealed: {:?}", p.facts().iter().map(|f| f.kind).collect::<Vec<_>>());
    p
}

#[test]
fn a_new_command_under_a_sealed_part_is_claimed_with_a_warning_naming_it() {
    let p = sealed("warns");
    let (code, out, err) = p.try_claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh -e tests/test_c1.sh"]);
    assert_eq!(code, 0, "never refused: {out}{err}");
    let said = format!("{out}{err}");
    assert!(said.contains("replaces the test command of sealed") && said.contains("\"greet says hello\""), "the warning names the sealed part: {said}");
}

#[test]
fn the_same_command_again_warns_of_nothing() {
    let p = sealed("same");
    let (code, out, err) = p.try_claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(!format!("{out}{err}").contains("replaces the test command"), "{out}{err}");
}
