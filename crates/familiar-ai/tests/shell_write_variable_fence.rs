//! A `$VAR` target resolved through an earlier `VAR=<literal>` is checked
//! against `.github` and `.familiar/approvals` like a plain path, and one the
//! text cannot resolve stays refused there (the owner, 2026-10-17, plan row
//! "The l-lanes folded in": S9d-w's fence wins over lane l5's resolution).

mod common;

use common::Project;
use serde_json::Value;

fn refused(v: &Value) -> bool {
    v["hookSpecificOutput"]["permissionDecision"] == "deny"
}

#[test]
fn a_resolved_variable_into_github_or_the_approvals_is_refused() {
    let p = Project::new("var-fence");
    for cmd in [
        "D=.github; cat x > $D/workflows/a.yml",
        "D=.github\ncat > ${D}/workflows/a.yml << 'EOF'\non: push\nEOF",
        "A=.familiar/approvals; cp f.sig $A/7.sig",
    ] {
        assert!(refused(&p.bash("PreToolUse", cmd)), "{cmd}");
    }
}

/// The plan confirmed, so an ordinary file outside the place is told, not refused.
fn confirmed(name: &str) -> Project {
    let p = Project::builder(name).no_session().no_app().file("pydantic/json_schema.py", "def schema():\n    return {}\n").create();
    p.claim(&["intent_submitted", "--for", "0", "--text", "fix the schema in pydantic/json_schema.py"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "the schema names the model", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"pydantic/json_schema.py","symbol":"schema","line_start":1,"line_end":2}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_schema.py", "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "pytest", "--test_command", "pytest -q tests/test_schema.py"]);
    // definition change (the owner's plan of 2026-10-23; owner approved 2026-10-23) [draft]: a pytest
    // part's place holds line 4 after a red try that lists it, the binary's listing
    let listed = serde_json::json!({ "criterion_id": "c1", "run_id": "try-1", "sha": "base", "red": true, "path_size": 1, "ranker": "words", "query": "the schema", "top": [{"rank": 1, "file_path": "pydantic/json_schema.py", "symbol": "schema", "line_start": 1, "line_end": 2}] });
    familiar_ai_facts::Record::open(&p.record_path()).expect("the record").append_raw("trace_listed", listed.as_object().cloned().unwrap_or_default(), 0.0, familiar_ai::TENANT).expect("the try's listing");
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1", "--agent_conversation_id", "s1"]);
    let out = p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    assert!(out.contains("familiar-ai: the plan is confirmed"), "{out}");
    p
}

#[test]
fn an_unresolved_variable_into_workflows_is_refused_even_with_the_plan_confirmed() {
    let p = confirmed("var-fence-unresolved");
    assert!(!refused(&p.bash("PreToolUse", "cat x > $X/notes.txt")), "an unresolved ordinary file is told, not refused");
    for cmd in ["cat x > $X/workflows/a.yml", "cp f.sig $X/approvals/7.sig", "D=.github; cat x > $D/workflows/a.yml"] {
        assert!(refused(&p.bash("PreToolUse", cmd)), "{cmd}");
    }
}

#[test]
fn a_resolved_variable_into_src_is_that_file() {
    let p = confirmed("var-fence-src");
    let plain = p.bash("PreToolUse", "cat x > src/a.rs");
    assert!(!refused(&plain), "{plain}");
    let through = p.bash("PreToolUse", "D=src; cat x > $D/a.rs");
    assert_eq!(through, plain, "the same answer as the plain write to src/a.rs");
}
