//! Two heads are proved each on its own: each criterion runs its own test
//! command, every criterion whose turn it is runs, and a commit is charged only
//! to the criterion it touches.

mod common;

use common::Project;
use familiar_ai_rules::lifecycle::{attempts, state, tries_not_red};
use familiar_ai_rules::moves::owed;
use familiar_ai_rules::View;
use serde_json::{json, Value};

const SESSION: &str = "9f4b1a5c-3e95-40c2-ae8b-5d6e7f8091a2";

fn project(name: &str) -> Project {
    Project::builder(name).session(SESSION).no_app().file("src/a.sh", "#!/bin/sh\ngreet() { echo hi; }\ngreet\n").file("src/b.sh", "#!/bin/sh\npart() { echo hi; }\npart\n").create()
}

impl Project {
    fn edit(&self, file: &str, old: &str, new: &str) -> Value {
        let path = self.path(file).to_string_lossy().into_owned();
        self.hook("PreToolUse", json!({ "tool_name": "Edit", "tool_input": { "file_path": path, "old_string": old, "new_string": new } }))
    }
}

fn denied(out: &Value) -> String {
    out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").to_string()
}

/// The `t` the refusal's claim line names.
fn named_t(reason: &str) -> String {
    reason.split("intent_submitted --for ").nth(1).and_then(|r| r.split_whitespace().next()).unwrap_or_else(|| panic!("a claim line: {reason}")).to_string()
}

#[test]
fn two_heads_are_proved_each_on_its_own() {
    let p = project("two");
    p.hook("UserPromptSubmit", json!({ "prompt": "a says hello, b says bye" }));
    let reason = denied(&p.edit("src/a.sh", "echo hi", "echo hello"));
    p.claim(&["intent_submitted", "--for", &named_t(&reason), "--text", "a says hello, b says bye"]);
    for (c, file, sym, text) in [("c1", "src/a.sh", "greet", "a says hello"), ("c2", "src/b.sh", "part", "b says bye")] {
        p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
        p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
        p.claim(&["criterion", "--criterion_id", c, "--text", text, "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
        p.cases_kept(c); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
        let places = format!(r#"[{{"file_path":"{file}","symbol":"{sym}","line_start":1,"line_end":3}}]"#);
        p.claim(&["place_chosen", "--criterion_id", c, "--places", &places]);
        let t = format!("tests/test_{c}.sh");
        p.claim(&["test_planned", "--criterion_id", c, "--file_path", &t, "--will", "create", "--test_command", &format!("sh {t}")]);
    }
    // the stack's own command fails: a criterion that ran it could never go green
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "false"]);
    for c in ["c1", "c2"] {
        p.claim(&["criterion_head", "--criterion_id", c]);
        p.claim(&["criterion_assigned", "--criterion_id", c]);
        p.claim(&["fix_location_category", "--criterion_id", c, "--category", "this_repository", "--where", ""]);
    }

    p.write("tests/test_c1.sh", "sh src/a.sh | grep -qx hello\n");
    p.commit("c1: tests first");
    p.write("tests/test_c2.sh", "sh src/b.sh | grep -qx bye\n");
    p.commit("c2: tests first");
    let log = p.facts();
    let v = View::new(&log);
    assert_eq!((state(&v, "c1"), state(&v, "c2")), ("sealed", "sealed"), "both heads seal on their own red");

    // c1's fix lands: c1 green while c2 is still red
    p.write("src/a.sh", "#!/bin/sh\ngreet() { echo hello; }\ngreet\n");
    p.commit("c1: the fix");
    let log = p.facts();
    let v = View::new(&log);
    assert_eq!((state(&v, "c1"), state(&v, "c2")), ("green", "sealed"), "c1 is proved by its own command, c2 is not");
    assert_eq!((attempts(&v, "c2"), tries_not_red(&v, "c2")), (0, 0), "c1's commits charge nothing to c2");

    // a commit outside every anchor charges nobody and is owed by nobody
    p.write("src/other.sh", "echo other\n");
    p.commit("elsewhere");
    let outside = p.head();
    let log = p.facts();
    let v = View::new(&log);
    for c in ["c1", "c2"] {
        assert_eq!((attempts(&v, c), tries_not_red(&v, c)), (0, 0), "{c} is charged nothing for a commit outside its anchor");
        assert!(owed(&v, c).iter().all(|w| w.commit != outside), "{c} owes nothing for it");
    }
    assert_eq!((state(&v, "c1"), state(&v, "c2")), ("green", "sealed"));
}
