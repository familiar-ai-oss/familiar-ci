//! A planned Python test file is named after what it tests, never after a plan
//! id: `tests/test_c1.py` was the example, and agents shipped it (2 of 19). A
//! `.py` file whose stem is `test_c<n>` or `test_<the criterion's id>` is
//! refused at the claim; the example names the anchor's symbol.

mod common;

use common::Project;

const SESSION: &str = "3f1c2a4b-5d6e-4f70-8a9b-0c1d2e3f4a5b";

fn anchored(name: &str, criterion: &str, symbol: &str) -> Project {
    let p = Project::builder(name).session(SESSION).create();
    p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
    p.claim(&["criterion", "--criterion_id", criterion, "--text", "greet says hello", "--path", "full"]);
    let places = format!(r#"[{{"file_path":"src/app.sh","symbol":"{symbol}","line_start":1,"line_end":3}}]"#);
    p.claim(&["place_chosen", "--criterion_id", criterion, "--places", &places]);
    p
}

#[test]
fn a_test_file_named_after_a_plan_id_is_refused_at_the_claim() {
    let p = anchored("plan-id", "c1", "greet");
    for file in ["tests/test_c1.py", "tests/test_c7.py", "test_c12.py"] {
        let (code, out, err) = p.try_claim(&["test_planned", "--criterion_id", "c1", "--file_path", file, "--will", "create"]);
        assert_eq!(code, 1, "{file}: {out}{err}");
        assert_eq!(err.lines().next().map(str::to_string), Some(format!("familiar-ai: not claimed yet · {file} is the example; name the test file after what it tests")), "{err}");
        assert!(err.contains("example:\n  familiar-ai claim test_planned --criterion_id c1 --file_path tests/test_greet.py --will create\n"), "the form's example names the anchor's symbol: {err}");
    }
    let p = anchored("criterion-id", "login", "check_password");
    let (code, _, err) = p.try_claim(&["test_planned", "--criterion_id", "login", "--file_path", "tests/test_login.py", "--will", "create"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.starts_with("familiar-ai: not claimed yet · tests/test_login.py is the example; name the test file after what it tests\n"), "{err}");
}

#[test]
fn a_test_file_named_for_what_it_tests_is_planned_and_a_shell_test_keeps_its_name() {
    let p = anchored("named", "c1", "greet");
    for file in ["tests/test_greet.py", "tests/test_c1_greets_by_name.py", "tests/unit/test_greet_loudly.py"] {
        p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", file, "--will", "create"]);
    }
    // Assumption 8: the walks' tests/test_c1.sh is a .sh file, so the refusal leaves it alone
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1.sh", "--will", "create"]);
}

#[test]
fn the_example_test_file_is_named_after_the_anchors_symbol() {
    let p = anchored("example", "c1", "GenerateJsonSchema.build");
    let status = p.status();
    assert!(
        status.contains("\n    6 its tests       claim it: familiar-ai claim test_planned --criterion_id c1 --file_path tests/test_build.py --will create\n"),
        "{status}"
    );
    let p = anchored("example-class", "c1", "GenerateJsonSchema");
    let (_, out, _) = p.run(&["claim", "test_planned"], "");
    assert!(out.contains("example:\n  familiar-ai claim test_planned --criterion_id c1 --file_path tests/test_generate_json_schema.py --will create\n"), "{out}");
    // with no anchor yet, the form's own example: never a plan id
    let (_, out, _) = Project::builder("example-none").create().run(&["claim", "test_planned"], "");
    assert!(out.contains("--file_path tests/test_greet.py --will create\n"), "{out}");
}
