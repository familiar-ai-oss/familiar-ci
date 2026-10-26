//! The stack's `claim it:` line runs the planned test file, not the form's
//! example; the example claimed verbatim, or a command naming a file neither
//! in the tree nor planned, is refused.

mod common;

use std::process::Command;

use common::{planned_to_line_7, run_binary, stdout, Scratch};

#[test]
fn the_claim_it_line_for_the_stack_runs_the_planned_tests_and_the_example_verbatim_is_refused() {
    let p = Scratch::new("aware-stack");
    p.git_init();
    std::fs::create_dir_all(p.dir.join("pydantic")).expect("folder");
    std::fs::write(p.dir.join("pydantic/json_schema.py"), "def schema():\n    return {}\n").expect("write");
    assert_eq!(p.run(&["setup"]).0, 0);
    for args in [vec!["add", "-A"], vec!["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", "commit", "-q", "-m", "base"]] {
        let out = Command::new("git").args(&args).current_dir(&p.dir).output().expect("git");
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    }
    let (code, out, err) = p.run(&["claim", "intent_submitted", "--for", "0", "--text", "MutableSequence in pydantic/json_schema.py is not handled"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("claim it: familiar-ai claim criterion --criterion_id c1 --text"), "{out}");
    let (code, out, err) = p.run(&["claim", "criterion", "--criterion_id", "c1", "--text", "a MutableSequence field gets an array schema", "--path", "light"]);
    assert_eq!(code, 0, "{err}");
    // the file the intent names, in the repository, and its first whole symbol (def schema, lines 1–2)
    assert!(out.contains(r#"claim it: familiar-ai claim place_chosen --criterion_id c1 --places '[{"file_path":"pydantic/json_schema.py","symbol":"schema","line_start":1,"line_end":2}]'"#), "{out}");
    let (code, out, err) = p.run(&["claim", "place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"pydantic/json_schema.py","symbol":"schema","line_start":1,"line_end":2}]"#]);
    assert_eq!(code, 0, "{err}");
    // definition change (lane l1, 2026-10-15): a test file is named after what it tests, never a plan id; the example is named after the anchor's symbol
    assert!(out.contains("claim it: familiar-ai claim test_planned --criterion_id c1 --file_path tests/test_schema.py --will create"), "{out}");
    let (code, out, err) = p.run(&["claim", "test_planned", "--criterion_id", "c1", "--file_path", "tests/test_mutable_sequence.py", "--will", "create"]);
    assert_eq!(code, 0, "{err}");
    // definition change (the owner, 2026-10-15, §9 Q3): the example runs python -m pytest, python3 when no python is on PATH
    let line_7 = format!("claim it: familiar-ai claim test_stack_detected --project_root . --test_stack pytest --test_command '{py} -m pytest -q tests/test_mutable_sequence.py'", py = familiar_ai::teach::example::python());
    assert!(out.contains(&line_7), "the planned file, not the example's: {out}");
    assert!(!out.contains("tests/test_schema.py"), "{out}");
    // the hook's own line says the same
    let input = format!(
        "{{\"hook_event_name\":\"PreToolUse\",\"cwd\":\"{0}\",\"tool_name\":\"Edit\",\"tool_input\":{{\"file_path\":\"{0}/pydantic/json_schema.py\"}}}}",
        p.dir.display()
    );
    let output = run_binary(&["hook", "PreToolUse"], &input);
    let hook_out = stdout(&output);
    assert!(hook_out.contains(&line_7.replace('"', "\\\"")), "{hook_out}");
    // the fieldless form inside the project carries the same example
    let (code, out, _) = p.run(&["claim", "test_stack_detected"]);
    assert_eq!(code, 0);
    // definition change (the owner, 2026-10-15, §9 Q3): the example runs python -m pytest, python3 when no python is on PATH
    assert!(out.contains(&format!("example:\n  familiar-ai claim test_stack_detected --project_root . --test_stack pytest --test_command '{py} -m pytest -q tests/test_mutable_sequence.py'\n", py = familiar_ai::teach::example::python())), "{out}");
    // the example claimed verbatim: refused at the door, naming the planned file, then the project's form
    // definition change (lane l1, 2026-10-15): a test file is named after what it tests, never a plan id
    // definition change (the owner, 2026-10-15, §9 Q3): the example runs python -m pytest, python3 when no python is on PATH
    let (code, out, err) = p.run(&["claim", "test_stack_detected", "--project_root", ".", "--test_stack", "pytest", "--test_command", &format!("{py} -m pytest -q tests/test_greet.py", py = familiar_ai::teach::example::python())]);
    assert_eq!(code, 1, "{out}");
    assert_eq!(out, "");
    // definition change (the owner, 2026-10-15, §9 Q3): the example runs python -m pytest, python3 when no python is on PATH
    assert!(err.starts_with(&format!("familiar-ai: not claimed yet · test_command '{py} -m pytest -q tests/test_greet.py' is the example; name the command that runs tests/test_mutable_sequence.py\n", py = familiar_ai::teach::example::python())), "{err}");
    // definition change (the owner, 2026-10-15, §9 Q3): the example runs python -m pytest, python3 when no python is on PATH
    assert!(err.contains(&format!("--test_command '{py} -m pytest -q tests/test_mutable_sequence.py'", py = familiar_ai::teach::example::python())), "{err}");
    // a command naming a file that is neither in the tree nor planned: refused naming the file
    let (code, _, err) = p.run(&["claim", "test_stack_detected", "--project_root", ".", "--test_stack", "pytest", "--test_command", "pytest -q tests/test_other.py"]);
    assert_eq!(code, 1);
    assert!(err.starts_with("familiar-ai: not claimed yet · test_command 'pytest -q tests/test_other.py' names tests/test_other.py, which is neither in the working tree nor a planned test file; name the command that runs tests/test_mutable_sequence.py\n"), "{err}");
    // a command with no path, and the planned file's command, are accepted
    let (code, _, err) = p.run(&["claim", "test_stack_detected", "--project_root", ".", "--test_stack", "pytest", "--test_command", "pytest -q"]);
    assert_eq!(code, 0, "{err}");
    assert!(!p.dir.join("tests/test_mutable_sequence.py").exists(), "the planned file is not written yet: it will be created after the plan");
    let (code, out, err) = p.run(&["claim", "test_stack_detected", "--project_root", ".", "--test_stack", "pytest", "--test_command", "pytest -q tests/test_mutable_sequence.py"]);
    assert_eq!(code, 0, "a planned file that does not exist yet is accepted: {err}");
    assert!(out.contains("claim it: familiar-ai claim criterion_head --criterion_id c1"), "{out}");
    // another criterion's planned file, not yet written, is accepted too
    let (code, _, err) = p.run(&["claim", "test_planned", "--criterion_id", "c1", "--file_path", "tests/test_tuple.py", "--will", "modify"]);
    assert_eq!(code, 0, "{err}");
    let (code, _, err) = p.run(&["claim", "test_stack_detected", "--project_root", ".", "--test_stack", "pytest", "--test_command", "python -m pytest tests/test_tuple.py tests/test_mutable_sequence.py"]);
    assert_eq!(code, 0, "{err}");
    // a second criterion: the next free id, and its tests file named for it
    let (code, out, err) = p.run(&["claim", "criterion", "--criterion_id", "c2", "--text", "a tuple field too", "--path", "light"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("--criterion_id c2 --places"), "line 4 for c2: {out}");
    let (code, out, _) = p.run(&["claim", "criterion"]);
    assert_eq!(code, 0);
    assert!(out.contains("example:\n  familiar-ai claim criterion --criterion_id c3 "), "{out}");
}

/// A test committed to an unplanned file makes the hook and the status name
/// the claim that plans it; a commit of the planned file does not.
#[test]
fn a_test_committed_to_a_file_the_plan_does_not_name_is_taught_the_claim_that_plans_it() {
    // definition change (lane l1, 2026-10-15): a test file is named after what it tests, never a plan id
    let line = "familiar-ai: tests/test_c1_by_alias.py is not a planned test file; the plan names tests/test_schema.py; plan it: familiar-ai claim test_planned --criterion_id c1 --file_path tests/test_c1_by_alias.py --will create, or move the test into tests/test_schema.py";
    for (file, said) in [("tests/test_c1_by_alias.py", true), ("tests/test_schema.py", false)] {
        let p = planned_to_line_7(if said { "unplanned-test" } else { "planned-test" });
        for args in [
            vec!["claim", "test_stack_detected", "--project_root", ".", "--test_stack", "sh", "--test_command", "true"],
            vec!["claim", "criterion_head", "--criterion_id", "c1"],
            vec!["claim", "criterion_assigned", "--criterion_id", "c1", "--agent_conversation_id", "s1"],
        ] {
            let (code, out, err) = p.run(&args);
            assert_eq!(code, 0, "{args:?}: {out}{err}");
        }
        std::fs::create_dir_all(p.dir.join("tests")).expect("folder");
        std::fs::write(p.dir.join(file), "def test_alias():\n    assert False\n").expect("write");
        p.git(&["add", "-A"]);
        p.git(&["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", "commit", "-q", "-m", "tests"]);
        let input = format!(
            "{{\"hook_event_name\":\"PostToolUse\",\"cwd\":\"{0}\",\"session_id\":\"s1\",\"tool_name\":\"Bash\",\"tool_input\":{{\"command\":\"git commit -m tests\"}},\"tool_response\":{{}}}}",
            p.dir.display()
        );
        let hook_out = stdout(&run_binary(&["hook", "PostToolUse"], &input));
        let (_, status, _) = p.run(&["status"]);
        assert_eq!(hook_out.contains(line), said, "{file}: {hook_out}");
        assert_eq!(status.contains(line), said, "{file}: {status}");
        if said {
            // the claim the line names is accepted, and the line is gone
            let (code, _, err) = p.run(&["claim", "test_planned", "--criterion_id", "c1", "--file_path", "tests/test_c1_by_alias.py", "--will", "create"]);
            assert_eq!(code, 0, "{err}");
            let (_, status, _) = p.run(&["status"]);
            assert!(!status.contains("is not a planned test file"), "{status}");
        }
    }
}
