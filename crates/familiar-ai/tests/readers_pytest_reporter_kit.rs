//! The pytest reader `pytest:reporter` (plan A, lane a-py): Familiar's own reporter, embedded in
//! the binary, written once per version outside the work tree, loaded through `PYTEST_ADDOPTS`
//! with the kit prepended to `PYTHONPATH`, the person's own values kept; refused where Python
//! ignores `PYTHONPATH` (`python -I`, `-E`).

use familiar_ai::door::readers::{env_for, kit_dir_in, Op, READERS};

#[test]
fn the_reader_list_has_pytests_reporter_then_its_tables() {
    let ids: Vec<&str> = READERS.iter().filter(|r| r.runner == "pytest").map(|r| r.id).collect();
    assert_eq!(ids, ["pytest:reporter", "pytest:tables"], "best first");
}

#[test]
fn the_kit_is_written_once_per_version_outside_the_work_tree() {
    let base = std::env::temp_dir().join(format!("familiar-ai-kit-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let dir = kit_dir_in(&base).expect("written");
    assert!(dir.starts_with(&base));
    assert!(dir.file_name().unwrap().to_string_lossy().starts_with("familiar-ai-kit-"), "{}", dir.display());
    let plugin = dir.join("pytest").join("familiar_ai_pytest.py");
    let text = std::fs::read_to_string(&plugin).expect("the pytest plugin");
    assert!(text.contains("def pytest_sessionfinish") && text.contains("FAMILIAR_AI_REPORT"));
    // written once: a second call leaves the file as it is
    std::fs::write(&plugin, "kept").unwrap();
    assert_eq!(kit_dir_in(&base).unwrap(), dir);
    assert_eq!(std::fs::read_to_string(&plugin).unwrap(), "kept");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn the_reporter_adds_itself_to_the_persons_options_and_path() {
    let kit = std::path::Path::new("/k");
    let report = std::path::Path::new("/r/results.jsonl");
    let ops = env_for("pytest:reporter", "python -m pytest -q", kit, report).expect("a reporter run");
    assert_eq!(
        ops,
        [
            Op::Append("PYTEST_ADDOPTS".into(), "-p familiar_ai_pytest".into()),
            Op::Prepend("PYTHONPATH".into(), "/k/pytest".into()),
            Op::Set("FAMILIAR_AI_REPORT".into(), "/r/results.jsonl".into()),
        ]
    );
    assert_eq!(env_for("pytest:tables", "pytest", kit, report).unwrap(), [], "the tables add nothing");
}

#[test]
fn the_reporter_is_refused_where_python_ignores_its_path() {
    let kit = std::path::Path::new("/k");
    let report = std::path::Path::new("/r");
    for c in ["python -I -m pytest", "python3 -E -m pytest tests", "uv run python -IB -m pytest"] {
        assert!(env_for("pytest:reporter", c, kit, report).is_err(), "{c}");
    }
    assert!(env_for("pytest:reporter", "python -B -m pytest", kit, report).is_ok());
    assert!(env_for("no:such", "pytest", kit, report).is_err(), "an unknown reader");
}
