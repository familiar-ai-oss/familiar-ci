//! A run that ran ends with its evidence: the exit code, and each stream
//! cleaned and cut, with a row for each stream that was cut (plan §3, lane S3).

mod common;

use std::time::{Duration, Instant};

use common::Project;
use familiar_ai_facts::Fact;
use serde_json::{json, Value};

const SESSION: &str = "9f4b1a5c-3e95-40c2-ae8b-5d6e7f8091a2";

/// Plans criterion c1 with `test_body` and `command`, then makes the
/// tests-first commit.
fn committed(name: &str, command: &str, test_body: &str) -> Project {
    let p = Project::builder(name).session(SESSION).env("FAMILIAR_AI_RUN_INLINE_MS", "0").create();
    p.hook("UserPromptSubmit", json!({ "prompt": "make greet say hello" }));
    let path = p.path("src/app.sh").to_string_lossy().into_owned();
    let out = p.hook("PreToolUse", json!({ "tool_name": "Edit", "tool_input": { "file_path": path, "old_string": "echo hi", "new_string": "echo hello" } }));
    let reason = out["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap_or("").to_string();
    let t = reason.split("intent_submitted --for ").nth(1).and_then(|r| r.split_whitespace().next()).unwrap_or_else(|| panic!("a claim line: {reason}")).to_string();
    p.claim(&["intent_submitted", "--for", &t, "--text", "greet says hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", command]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    p.write("tests/test_c1.sh", test_body);
    p.git(&["add", "-A"]);
    p.git(&["commit", "-q", "-m", "c1: tests first"]);
    p.bash("PostToolUse", "git commit -m 'c1: tests first'");
    p
}

fn ended(p: &Project) -> Fact {
    let until = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(f) = p.facts().into_iter().find(|f| f.kind == "run_ended") {
            return f;
        }
        assert!(Instant::now() < until, "gave up waiting for run_ended");
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn a_failing_run_ends_with_its_exit_code_and_its_cleaned_output() {
    let p = committed("evidence", "sh tests/test_c1.sh", "printf '\\033[31mFAILED\\033[0m test_greet\\r\\n'\necho 'boom' >&2\nexit 3\n");
    let f = ended(&p);
    assert_eq!(f.fields.get("outcome"), Some(&json!("failed")));
    assert_eq!(f.fields.get("exit_code"), Some(&json!(3)));
    assert_eq!(f.fields.get("signal"), None);
    assert_eq!(f.fields.get("stdout_tail"), Some(&json!("FAILED test_greet\n")));
    assert_eq!(f.fields.get("stderr_tail"), Some(&json!("boom\n")));
    assert_eq!(f.fields.get("truncated"), None, "nothing was cut");
}

#[test]
fn each_result_names_the_run_it_came_from() {
    let p = committed("evidence-run-id", "sh tests/test_c1.sh", "exit 1\n");
    let run_id = ended(&p).fields.get("run_id").cloned().unwrap_or(Value::Null);
    let until = Instant::now() + Duration::from_secs(60);
    let results = loop {
        let facts = p.facts();
        if facts.iter().any(|f| f.kind == "fingerprint_stamped") {
            break facts.into_iter().filter(|f| f.kind == "pass_please_pass").collect::<Vec<Fact>>();
        }
        assert!(Instant::now() < until, "gave up waiting for the results");
        std::thread::sleep(Duration::from_millis(100));
    };
    assert!(run_id.is_string() && !results.is_empty(), "{run_id} {results:?}");
    for r in &results {
        assert_eq!(r.fields.get("run_id"), Some(&run_id), "every result at the commit, from the one run: {:?}", r.fields);
    }
}

#[test]
fn a_long_output_is_stored_cut_with_a_row_for_its_stream() {
    let p = committed("evidence-long", "sh tests/test_c1.sh", "i=0\nwhile [ $i -lt 3000 ]; do echo \"line $i\"; i=$((i+1)); done\nexit 1\n");
    let f = ended(&p);
    let tail = f.fields.get("stdout_tail").and_then(Value::as_str).unwrap_or("").to_string();
    assert!(tail.starts_with("line 0\nline 1\n"), "the head: {}", &tail[..40.min(tail.len())]);
    assert!(tail.ends_with("line 2999\n"), "the tail");
    assert!(tail.contains(" chars dropped]\n"), "the marker");
    assert!(tail.chars().count() <= 2_000 + 6_000 + 40);
    let total: u64 = (0..3000).map(|i: u64| format!("line {i}\n").len() as u64).sum();
    assert_eq!(f.fields.get("truncated"), Some(&json!([{ "stream": "stdout", "total_bytes": total, "dropped_bytes": 0 }])));
    assert_eq!(f.fields.get("stderr_tail"), Some(&json!("")));
}
