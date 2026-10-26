//! A test file planned twice for one part (claimed again, as a plan written
//! twice does) is named once wherever "tested by" lists a part's tests: the
//! status, the refused stop and the pull request body.

mod common;

use common::Project;
use serde_json::json;

#[test]
fn a_test_file_planned_twice_is_named_once() {
    let p = Project::new("once");
    p.hook("UserPromptSubmit", json!({ "prompt": "make greet say hello" }));
    let said = p.facts().iter().find(|f| f.kind == "person_said").map(|f| f.t).expect("said");
    p.claim(&["intent_submitted", "--for", &said.to_string(), "--text", "make greet say hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    for _ in 0..2 {
        p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_greet.sh", "--will", "create"]);
    }
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_bye.sh", "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "for f in tests/test_*.sh; do sh \"$f\" || exit 1; done"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    p.write("tests/test_greet.sh", "grep -q hello src/app.sh\n");
    p.write("tests/test_bye.sh", "grep -q hello src/app.sh\n");
    p.commit("greet: tests");

    let status = p.status();
    assert!(status.contains("\n  tested by: tests/test_greet.sh, tests/test_bye.sh\n"), "{status}");
    let stop = p.hook("Stop", json!({}));
    let reason = stop["reason"].as_str().unwrap_or_default();
    assert!(reason.contains("\n  tested by: tests/test_greet.sh, tests/test_bye.sh\n"), "{stop}");
    let (code, pr, err) = p.run(&["status", "--pr"], "");
    assert_eq!(code, 0, "{err}");
    assert!(pr.contains("\n- tested by: tests/test_greet.sh, tests/test_bye.sh\n"), "{pr}");
}
