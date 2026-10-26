//! A test file is told as not planned only when the agent added or changed it
//! after the planning sha and no `test_planned` names it; test files already
//! in the repository and untouched are never mentioned.

mod common;

use common::Project;
use serde_json::json;

const SESSION: &str = "84be6339-0cc7-40ec-91b9-cae44365931b";

const TOLD: &str = "is not a planned test file";

impl Project {
    /// Returns the not-planned lines `status` and `plan` (an empty array) print.
    fn told(&self) -> (Vec<String>, Vec<String>) {
        let (code, status, err) = self.run(&["status"], "");
        assert_eq!(code, 0, "status: {err}");
        let (code, plan, err) = self.run(&["plan"], "[]");
        assert_eq!(code, 0, "plan: {err}");
        let lines = |out: &str| out.lines().filter(|l| l.contains(TOLD)).map(str::to_string).collect::<Vec<_>>();
        (lines(&status), lines(&plan))
    }
}

/// A project whose earlier commit landed four test files, then a new ask
/// planned to c1 with tests/test_c5.sh.
fn planned(name: &str) -> Project {
    let p = Project::builder(name).session(SESSION).create();
    p.hook("UserPromptSubmit", json!({ "prompt": "look around the tests" }));
    p.claim(&["intent_submitted", "--text", "look around the tests"]);
    for n in 1..=4 {
        p.write(&format!("tests/test_c{n}.sh"), &format!("grep -q hi src/app.sh # {n}\n"));
    }
    p.git(&["add", "-A"]);
    p.git(&["commit", "-q", "-m", "four test files"]);
    p.hook("UserPromptSubmit", json!({ "prompt": "make greet say hello" }));
    p.claim(&["intent_submitted", "--text", "greet with hello"]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "light"]);
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c5.sh", "--will", "create"]);
    p
}

#[test]
fn a_test_file_that_existed_at_the_planning_sha_and_is_untouched_is_never_told() {
    let p = planned("existing");
    let (status, plan) = p.told();
    assert!(status.is_empty(), "status tells no existing test file: {status:?}");
    assert!(plan.is_empty(), "plan tells no existing test file: {plan:?}");
}

#[test]
fn a_test_file_the_agent_writes_outside_the_plan_is_told_once() {
    let p = planned("sixth");
    p.write("tests/test_c6.sh", "grep -q hello src/app.sh\n");
    let (status, plan) = p.told();
    assert_eq!(status.len(), 1, "one line, for the sixth: {status:?}");
    assert!(status[0].starts_with("familiar-ai: tests/test_c6.sh is not a planned test file; the plan names tests/test_c5.sh"), "{status:?}");
    assert_eq!(plan, status, "plan tells the same line");

    // committed, it is still the one line
    p.git(&["add", "-A"]);
    p.git(&["commit", "-q", "-m", "c6 test"]);
    p.hook("PostToolUse", json!({ "tool_name": "Bash", "tool_input": { "command": "git commit -m 'c6 test'" }, "tool_response": {} }));
    let (status, plan) = p.told();
    assert_eq!(status.len(), 1, "one line, for the sixth: {status:?}");
    assert!(status[0].contains("tests/test_c6.sh is not a planned test file"), "{status:?}");
    assert_eq!(plan, status);
}
