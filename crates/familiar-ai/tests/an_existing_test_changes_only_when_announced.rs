//! An existing test's lines change only when announced (the owner, 2026-10-20; the bench's
//! astroid-3152) [draft names]. On a real repository: the binary reads, at every commit, the
//! lines of test definitions that stood at the plan's base and that the commit's tree no longer
//! holds as they were, and writes them on `commit_seen` as `existing_tests_changed`. Python's
//! test definitions are the `def test*` functions and the `class Test*` bodies, with the
//! decorators above them; module-level lines (imports, helpers) never count, nor a new test added
//! beside them, nor the agent's own planned tests. While one stands unannounced the part is held
//! `existing_test_changed` and the stop is refused naming the file, the test and the lines; an
//! announcement with a reason or the lines put back clears it. The judge is off here.

mod common;

use common::Project;
use serde_json::{json, Value};

const OLD: &str = "tests/test_old.py";

const OLD_AT_BASE: &str = "\
import os
from app import greet


def helper():
    return 1


def test_init_defaults():
    assert greet() == \"_HAS_DEFAULT_FACTORY\"


class TestGreeting:
    @staticmethod
    def test_twice():
        assert greet() == greet()
";

/// A project planned to c1 (anchor src/app.sh, its own test tests/test_c1.sh, which stood at the
/// base and the plan modifies), with tests/test_old.py at the base.
fn project(name: &str) -> Project {
    let p = Project::builder(name).file(OLD, OLD_AT_BASE).file("tests/test_c1.sh", "exit 0\n").create();
    p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "modify"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "sh tests/test_c1.sh"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    p
}

/// The own test changed and sealed red, then the code that makes it pass: green.
fn green(p: &Project) {
    p.write("tests/test_c1.sh", "sh src/app.sh | grep -qx hello\n");
    p.commit("c1: tests");
    p.write("src/app.sh", "#!/bin/sh\ngreet() { echo hello; }\ngreet\n");
    p.commit("c1: code");
    assert_eq!(p.state_name("c1"), "green", "{}", p.status());
}

fn last_change(p: &Project) -> Option<Value> {
    p.facts_of("commit_seen").last().and_then(|f| f.get("existing_tests_changed").cloned())
}

fn stop(p: &Project) -> Value {
    p.hook("Stop", json!({}))
}

#[test]
fn a_changed_line_of_a_test_that_stood_at_the_base_holds_until_it_is_put_back() {
    let p = project("existing-held");
    green(&p);
    assert!(p.facts_of("commit_seen").iter().all(|f| f.get("existing_tests_changed").is_none()), "the own planned test is never an existing test's change");
    p.write(OLD, &OLD_AT_BASE.replace("\"_HAS_DEFAULT_FACTORY\"", "\"object()\""));
    p.commit("loosen the old test");
    assert_eq!(last_change(&p), Some(json!([{ "file_path": OLD, "tests": ["test_init_defaults"], "lines": [[10, 10]] }])));
    assert_eq!(p.state_name("c1"), "held", "{}", p.status());
    let s = stop(&p);
    assert_eq!(s["decision"], "block", "{s}");
    let r = s["reason"].as_str().unwrap_or("");
    assert!(r.contains("tests/test_old.py (test_init_defaults, line 10)"), "{r}");
    assert!(r.contains("familiar-ai claim existing_test_announced --file_path tests/test_old.py --reason"), "{r}");
    // a later commit elsewhere leaves it standing
    p.write("notes.txt", "x\n");
    p.commit("another file");
    assert_eq!(p.state_name("c1"), "held");
    // the lines put back
    p.write(OLD, OLD_AT_BASE);
    p.commit("put the old test back");
    assert_eq!(last_change(&p), None);
    assert_eq!(p.state_name("c1"), "green", "{}", p.status());
}

#[test]
fn a_decorator_counts_and_an_import_a_helper_or_a_new_test_does_not() {
    let p = project("existing-what-counts");
    green(&p);
    // an import, a helper and a new test function beside the old ones
    let more = OLD_AT_BASE.replace("import os\n", "import os, sys\n").replace("return 1", "return 2") + "\n\ndef test_new():\n    assert True\n";
    p.write(OLD, &more);
    p.commit("add a test");
    assert_eq!(last_change(&p), None);
    assert_eq!(p.state_name("c1"), "green", "{}", p.status());
    // a decorator above a test is the test's
    p.write(OLD, &more.replace("    @staticmethod\n", ""));
    p.commit("drop the decorator");
    assert_eq!(last_change(&p), Some(json!([{ "file_path": OLD, "tests": ["TestGreeting.test_twice"], "lines": [[14, 14]] }])));
    assert_eq!(p.state_name("c1"), "held");
}

#[test]
fn an_announcement_with_a_reason_clears_it() {
    let p = project("existing-announced");
    green(&p);
    p.write(OLD, &OLD_AT_BASE.replace("\"_HAS_DEFAULT_FACTORY\"", "\"hello\""));
    p.commit("the old test expects the new greeting");
    assert_eq!(p.state_name("c1"), "held");
    p.claim(&["existing_test_announced", "--file_path", OLD, "--reason", "the person asked that greet say hello"]);
    assert_eq!(p.state_name("c1"), "green", "{}", p.status());
    let s = stop(&p);
    assert_ne!(s["decision"], "block", "{s}");
}
