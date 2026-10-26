//! An allowed edit tells the agent before it lands: its PreToolUse carries the
//! words as `additionalContext`, never a `permissionDecision`. An edit of a
//! planned test not yet sealed, by a tool or by the shell, is told what the
//! test is for; an edit outside the place the plan named, by a tool or by the
//! shell, is told the place and the way out, and after the edit the same line
//! goes to the person instead, so each reads it once.

mod common;

use common::Project;
use serde_json::{json, Value};

const SESSION: &str = "5e4d3c2b-1a09-4f8e-9d7c-6b5a4f3e2d1c";

fn planned(name: &str) -> Project {
    let p = Project::builder(name).session(SESSION).file("src/other.sh", "echo other\n").create();
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
    p
}

impl Project {
    fn edit(&self, event: &str, file: &str) -> Value {
        let path = self.path(file).to_string_lossy().into_owned();
        self.tool(event, "Edit", json!({ "file_path": path, "old_string": "x", "new_string": "y" }))
    }
}

/// The words of an allowed PreToolUse: context for the agent, no decision, nothing for the person.
fn told_before(out: &Value) -> String {
    assert!(!out.to_string().contains("permissionDecision"), "an allow sets no decision: {out}");
    assert!(out.get("systemMessage").is_none(), "{out}");
    assert_eq!(out["hookSpecificOutput"]["hookEventName"], "PreToolUse", "{out}");
    out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or_else(|| panic!("the agent is told: {out}")).to_string()
}

#[test]
fn an_edit_of_the_planned_test_is_told_what_it_is_for_before_it_lands() {
    let p = planned("planned-test");
    let context = told_before(&p.edit("PreToolUse", "tests/test_c1.sh"));
    assert_eq!(
        context,
        [
            "familiar-ai: tests/test_c1.sh is the planned test of \"greet says hello\"",
            "  asked for: make greet say hello instead of hi",
            "  the agent's reading: greet with hello",
            "  it must fail on the code as it is, committed alone, before any fix; familiar-ai runs sh tests/test_c1.sh after each commit",
        ]
        .join("\n")
    );
    // a shell write of the planned test is an edit of it, told the same before it runs
    let shell = p.bash("PreToolUse", "cat > tests/test_c1.sh << 'EOF'\nsh src/app.sh | grep -qx hello\nEOF");
    assert_eq!(told_before(&shell), context);
    // an edit at the place the plan named has nothing to tell
    assert_eq!(p.edit("PreToolUse", "src/app.sh"), json!({}));
    // once the test is sealed, its edit is no longer told what it is for
    p.write("tests/test_c1.sh", "sh src/app.sh | grep -qx hello\n");
    p.commit("c1: tests first");
    assert_eq!(p.state_name("c1"), "sealed");
    let out = p.edit("PreToolUse", "tests/test_c1.sh");
    assert!(!out.to_string().contains("is the planned test of"), "{out}");
}

#[test]
fn an_edit_outside_the_place_is_told_the_place_and_the_way_out_before_it_and_the_person_after_it() {
    let p = planned("outside");
    let line = "familiar-ai: this edit is outside the place the plan named for \"greet says hello\", src/app.sh greet lines 1–3; it goes ahead\n  \
                if the fix belongs elsewhere: ask the person with AskUserQuestion, and on a yes claim the new place, said first: \
                familiar-ai claim place_chosen --criterion_id c1 --places '[{\"file_path\":\"src/other.sh\",\"symbol\":\"greet\",\"line_start\":1,\"line_end\":3}]'; \
                the proof of \"greet says hello\" starts over at the new place";
    assert_eq!(told_before(&p.edit("PreToolUse", "src/other.sh")), line);
    // after the edit the same line is the person's, not the agent's again
    let out = p.edit("PostToolUse", "src/other.sh");
    assert_eq!(out["systemMessage"], line, "{out}");
    assert!(!out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("").contains("outside the place"), "{out}");
    // a shell write outside the place: the agent reads it before, the person after
    let write = "cat > src/other.sh << 'EOF'\necho other, too\nEOF";
    assert_eq!(told_before(&p.bash("PreToolUse", write)), line);
    let out = p.bash("PostToolUse", write);
    assert_eq!(out["systemMessage"], line, "{out}");
    assert!(!out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("").contains("outside the place"), "{out}");
}
