//! The touching tests the binary derives (the owner, 2026-10-20) [draft names]. Declared touching
//! tests are opt-in and seldom declared, so when the fix lands the binary finds the project's
//! existing test files that import a source file the fix changed, nearest first and at most five
//! per part, writes each as `touching_test_derived`, and runs them at the head in a run of their
//! own beside the part's own (the owner's plan of 2026-10-21, step 0): a failure there keeps the
//! part from green. On a real repository.

mod common;

use common::Project;

const M_AT_BASE: &str = "def greet():\n    return \"hi\"\n\n\ndef shout():\n    return greet().upper()\n";
/// Its first line hands it to python3 when the stack's `sh` runs it (a string to Python).
const TEST_M: &str = "''''exec python3 \"$0\" \"$@\" # '''\nimport os\nimport sys\n\nsys.path.insert(0, os.path.join(os.path.dirname(__file__), \"..\", \"src\"))\n\nimport m\n\n\ndef test_shout():\n    assert m.shout() == \"HI\"\n\n\nif __name__ == \"__main__\":\n    test_shout()\n";
const TEST_OTHER: &str = "def test_other():\n    assert True\n";
/// The part's own test: greet says hello.
const OWN: &str = "python3 -c 'import sys; sys.path.insert(0, \"src\"); import m; print(m.greet())' | grep -qx hello\n";

fn project(name: &str) -> Project {
    let p = Project::builder(name)
        .no_app()
        .file("src/m.py", M_AT_BASE)
        .file("tests/test_m.py", TEST_M)
        .file("tests/test_other.py", TEST_OTHER)
        .create();
    p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "nothing", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/m.py","symbol":"greet","line_start":1,"line_end":2}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "sh_pytest", "--test_command", "sh"]);
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1"]);
    p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    p
}

fn commands(p: &Project) -> Vec<String> {
    p.facts_of("run_reserved").iter().filter_map(|f| f["command"].as_str().map(str::to_string)).collect()
}

#[test]
fn an_existing_test_importing_the_fixed_module_runs_at_green_and_its_failure_keeps_the_part_from_green() {
    let p = project("touching-derived");
    p.write("tests/test_c1.sh", OWN);
    p.commit("c1: tests");
    assert_eq!(p.state_name("c1"), "sealed", "{}", p.status());
    assert!(p.facts_of("touching_test_derived").is_empty(), "nothing is derived before the fix");
    // the fix breaks shout, which the existing tests/test_m.py checks
    p.write("src/m.py", &M_AT_BASE.replace("return \"hi\"", "return \"hello\""));
    p.commit("c1: greet says hello");
    let derived = p.facts_of("touching_test_derived");
    assert_eq!(derived.len(), 1, "{derived:?}");
    assert_eq!((derived[0]["criterion_id"].as_str(), derived[0]["file_path"].as_str(), derived[0]["imports"].as_str()), (Some("c1"), Some("tests/test_m.py"), Some("src/m.py")));
    // the definition of 2026-10-21 (step 0): the own run is the own files only, the touching
    // tests run under a command of their own
    assert!(commands(&p).iter().any(|c| c == "sh tests/test_c1.sh"), "{:?}", commands(&p));
    assert!(commands(&p).iter().any(|c| c == "sh tests/test_m.py"), "{:?}", commands(&p));
    assert!(!commands(&p).iter().any(|c| c.contains("tests/test_c1.sh") && c.contains("tests/test_m.py")), "{:?}", commands(&p));
    assert_ne!(p.state_name("c1"), "green", "{}", p.status());
    // the fix that keeps shout as it was: green
    p.write("src/m.py", "def greet():\n    return \"hello\"\n\n\ndef shout():\n    return \"HI\"\n");
    p.commit("c1: shout as before");
    assert_eq!(p.facts_of("touching_test_derived").len(), 1, "each file once");
    assert_eq!(p.state_name("c1"), "green", "{}", p.status());
}
