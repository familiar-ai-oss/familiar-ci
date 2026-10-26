//! The example command is `python -m pytest -q <files>` when `python` is on PATH where
//! the example is printed, else `python3 -m pytest -q <files>` (the owner, 2026-10-15,
//! §9 Q3: "try one then do the other if command not found"). Never both in one command:
//! a red test exits non-zero and would run twice.

mod common;

use common::Project;

const SESSION: &str = "3f1c2a4b-5d6e-4f70-8a9b-0c1d2e3f4a5b";

/// A folder holding only `python` (a stub) and the tools the binary calls, made the
/// binary's whole PATH.
fn only_on_path(p: &Project, python: &str) {
    let bin = p.dir.join(format!("path-{python}"));
    std::fs::create_dir_all(&bin).expect("bin");
    std::fs::write(bin.join(python), "#!/bin/sh\nexit 0\n").expect("stub");
    std::process::Command::new("chmod").arg("+x").arg(bin.join(python)).status().expect("chmod");
    for tool in ["git", "sh"] {
        let found = ["/usr/bin", "/bin"].iter().map(|d| std::path::Path::new(d).join(tool)).find(|f| f.exists()).expect(tool);
        std::os::unix::fs::symlink(found, bin.join(tool)).expect("link");
    }
    p.set_env("PATH", Some(bin.to_str().expect("utf-8")));
}

fn planned(name: &str) -> Project {
    let p = Project::builder(name).session(SESSION).create();
    p.claim(&["intent_submitted", "--for", "0", "--text", "greet with hello"]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "greet says hello", "--path", "full"]);
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"src/app.sh","symbol":"greet","line_start":1,"line_end":3}]"#]);
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_greet.py", "--will", "create"]);
    p
}

#[test]
fn with_python_on_path_the_example_says_python_dash_m_pytest() {
    let p = planned("python");
    only_on_path(&p, "python");
    assert!(p.status().contains("\n    7 the command     claim it: familiar-ai claim test_stack_detected --project_root . --test_stack pytest --test_command 'python -m pytest -q tests/test_greet.py'\n"), "{}", p.status());
    let q = Project::builder("python-form").create();
    only_on_path(&q, "python");
    let (_, out, _) = q.run(&["claim", "test_stack_detected"], "");
    assert!(out.contains("--test_command 'python -m pytest -q tests/test_greet.py'\n"), "{out}");
}

#[test]
fn with_only_python3_on_path_the_example_says_python3_dash_m_pytest() {
    let p = planned("python3");
    only_on_path(&p, "python3");
    assert!(p.status().contains("\n    7 the command     claim it: familiar-ai claim test_stack_detected --project_root . --test_stack pytest --test_command 'python3 -m pytest -q tests/test_greet.py'\n"), "{}", p.status());
    let q = Project::builder("python3-form").create();
    only_on_path(&q, "python3");
    let (_, out, _) = q.run(&["claim", "test_stack_detected"], "");
    assert!(out.contains("--test_command 'python3 -m pytest -q tests/test_greet.py'\n"), "{out}");
    // the form's example pasted verbatim is still the example
    let (code, _, err) = q.try_claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "pytest", "--test_command", "python3 -m pytest -q tests/test_greet.py"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.starts_with("familiar-ai: not claimed yet · test_command 'python3 -m pytest -q tests/test_greet.py' is the example;"), "{err}");
}
