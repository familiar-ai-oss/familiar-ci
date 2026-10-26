//! Sealed says where the code goes: the PostToolUse of the tests-first commit
//! whose run sealed a criterion says the tests fail as they should, then the
//! place the plan named for the code and that the test stays as it is. A judge's
//! no on that seal means the test changes first, so the place is not said then.

mod common;

use common::Project;

const SESSION: &str = "2e1d0c9b-8a7f-4e6d-9c5b-4a3f2e1d0c9b";

const NO: &str = r#"{"type":"result","is_error":false,"result":"{\"answers_intent\": false, \"gap\": \"greet saying hello when it runs\", \"reason\": \"it greps the file instead of running greet.\"}","modelUsage":{"claude-opus-4-6":{}}}"#;

/// c1 planned on src/app.sh greet lines 1–3; with `judge`, a fake `claude` answers the judge `judge`.
fn planned(name: &str, judge: Option<&str>) -> Project {
    let b = Project::builder(name).session(SESSION);
    let p = match judge {
        Some(answer) => {
            let d = b.dir().join("bin");
            let fake = format!("#!/bin/sh\ncat > /dev/null\ncat '{}/answer'\n", d.display());
            let b = b.bin("claude", &fake);
            std::fs::write(d.join("answer"), answer).expect("answer");
            b.env_remove("FAMILIAR_AI_JUDGE").create()
        }
        None => b.create(),
    };
    p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
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

#[test]
fn the_seal_says_where_the_code_goes_and_that_the_test_stays_as_it_is() {
    let p = planned("sealed", None);
    p.write("tests/test_c1.sh", "sh src/app.sh | grep -qx hello\n");
    let out = p.commit("c1: tests first");
    assert_eq!(
        out["hookSpecificOutput"]["additionalContext"],
        "familiar-ai: ran the tests of \"greet says hello\": they fail, as they should before the fix · stamped\n  the code goes on src/app.sh greet lines 1–3; the test stays as it is"
    );
    assert_eq!(p.state_name("c1"), "sealed");
}

#[test]
fn a_judges_no_on_the_seal_does_not_send_the_agent_to_the_code() {
    let p = planned("judged-no", Some(NO));
    p.write("tests/test_c1.sh", "grep -q hello src/app.sh\n");
    let out = p.commit("c1: tests first");
    let context = out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("");
    assert!(context.ends_with("familiar-ai: ran the tests of \"greet says hello\": they fail, as they should before the fix · stamped"), "{context}");
    assert!(!context.contains("the code goes on"), "{context}");
}

#[test]
fn a_run_that_did_not_seal_says_no_place() {
    let p = planned("not-red", None);
    p.write("tests/test_c1.sh", "true\n");
    let out = p.commit("c1: tests first");
    let context = out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("");
    assert!(!context.contains("the code goes on"), "{context}");
    // and the green run of the fix says no place either
    let p = planned("green", None);
    p.write("tests/test_c1.sh", "sh src/app.sh | grep -qx hello\n");
    p.commit("c1: tests first");
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hello; }\ngreet\n");
    let out = p.commit("c1: greet says hello");
    let context = out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("");
    assert!(context.starts_with("familiar-ai: ran the tests of \"greet says hello\": they pass"), "{context}");
    assert!(!context.contains("the code goes on"), "{context}");
}
