//! New work beside an existing symbol (`new_work_spot_chosen`) is touched by a
//! commit that changes the spot's file, so the criterion goes green rather than
//! reading as landing elsewhere.

mod common;

use common::Project;

const SESSION: &str = "8e3a0f4b-2d84-4fb1-9d7a-4c5d6e7f8091";

#[test]
fn new_work_beside_a_symbol_goes_green_when_its_file_changes() {
    let p = Project::builder("beside").session(SESSION).create();
    p.claim(&["intent_submitted", "--for", "0", "--text", "say goodbye too"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "farewell says goodbye", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["new_work_spot_chosen", "--criterion_id", "c1", "--file_path", "src/app.sh", "--symbol", "farewell", "--beside", "greet"]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    p.write("tests/test_c1.sh", ". src/app.sh >/dev/null; farewell | grep -qx goodbye\n");
    let sealed = p.commit("c1: tests first");
    assert!(sealed.to_string().contains("they fail, as they should before the fix · stamped"), "{sealed}");
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hi; }\nfarewell() { echo goodbye; }\ngreet\n");
    let green = p.commit("c1: farewell");
    let told = green.to_string();
    assert!(told.contains("they pass · stamped"), "{green}");
    assert!(!told.contains("landed outside the place the plan named"), "the spot's file is its place: {green}");
    let (_, status, _) = p.run(&["status"], "");
    assert!(status.contains("\"farewell says goodbye\" · locally green"), "{status}");
}
