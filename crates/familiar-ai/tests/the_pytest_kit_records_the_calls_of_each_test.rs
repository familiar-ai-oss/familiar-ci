//! The pytest kit's per-test CALL RECORDER (FAMILIAR_AI_TRACE, off by default): on, the plugin
//! puts the test's nodeid in FAMILIAR_AI_TEST_ID for the test's call and re-arms the shared
//! recorder (familiar_ai_calls; a child python gets it from the kit's sitecustomize, which
//! chains to the project's own sitecustomize); a child leaves its calls in a <pid>.json the
//! controller merges at session finish into one calls line per test. Off, the report is as it
//! is, and a project's own sitecustomize still runs beside the kit's.
//!
//! Live, through the kit's files as the binary ships them (kit_dir_in), on every python with
//! pytest; not run, like the toolchain tests, when there is none.

mod schema_truth;

use familiar_ai::door::readers::kit_dir_in;
use schema_truth::{pythons, run_python, Dir, Live, Reading};
use serde_json::Value;
use std::path::Path;

/// The project's module: f calls g (the recorded pair), h stands alone.
const MOD: &str = "def f():\n    return g()\n\ndef g():\n    return 42\n\ndef h():\n    return 7\n";

/// A failing test that calls f (which calls g) and a passing test that calls h only; the
/// tests' folder is not recorded.
const CALLS_TESTS: &str = "from src.pkg import mod\n\n\ndef test_fails():\n    assert mod.f() == 1\n\n\ndef test_passes():\n    assert mod.h() == 7\n";

/// A test that starts a child python that calls g (the parent never calls it).
const CHILD_TESTS: &str = "import os\nimport subprocess\nimport sys\n\n\ndef test_child():\n    here = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))\n    done = subprocess.run([sys.executable, \"-c\", \"import src.pkg.mod; src.pkg.mod.g()\"], cwd=here, capture_output=True)\n    assert done.returncode == 0\n";

/// A test that clears the profiler mid-run, and the test after it.
const CLEARS_TESTS: &str = "import sys\n\nfrom src.pkg import mod\n\n\ndef test_clears():\n    def victim():\n        return 9\n\n    assert victim() == 9\n    sys.setprofile(None)\n\n\ndef test_after():\n    assert mod.g() == 42\n";

/// The project's own sitecustomize: writes a marker next to itself.
const PERSON_SITE: &str = "import os\n\nopen(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'ran'), 'w').write('yes')\n";

/// A project of `files` (in the tests' folder, the recorder does not record it) plus
/// src/pkg/mod.py and a root conftest that puts the root on sys.path.
fn project(files: &[(&str, &str)]) -> Dir {
    let d = Dir::new("calls-project");
    d.put("conftest.py", "import os\nimport sys\n\nsys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))\n")
        .put("src/__init__.py", "")
        .put("src/pkg/__init__.py", "")
        .put("src/pkg/mod.py", MOD);
    for (name, contents) in files {
        d.put(name, contents);
    }
    d
}

/// The pythons on PATH this machine can run the kit's pytest with: empty, and the test is
/// not run, like the toolchain tests skip without their toolchain.
fn with_pytest() -> Vec<(String, u32, u32)> {
    pythons().into_iter().filter(|(name, _, _)| {
        std::process::Command::new(name)
            .args(["-c", "import pytest"])
            .stdin(std::process::Stdio::null())
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }).collect()
}

/// Runs the project's tests the way the reader does (the kit's files as the binary ships
/// them, its folder prepended to PYTHONPATH, the reporter through PYTEST_ADDOPTS), `trace`
/// the recorder on or off, `person` an extra PYTHONPATH entry, after the kit's folder (a
/// project's own sitecustomize).
fn run(py: &str, d: &Dir, trace: bool, person: Option<&Path>) -> Live {
    let base = std::env::temp_dir().join(format!("familiar-ai-calls-kit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let kit = kit_dir_in(&base).expect("the kit's files");
    let rep = Dir::new("report");
    let report = rep.path.join("results.jsonl");
    let mut pp = kit.join("pytest").to_string_lossy().to_string();
    if let Some(p) = person {
        pp = format!("{pp}:{}{}", std::path::MAIN_SEPARATOR, p.to_string_lossy());
    }
    let mut env = vec![
        ("PYTHONPATH".to_string(), pp),
        ("PYTEST_ADDOPTS".to_string(), "-p familiar_ai_pytest".to_string()),
        ("FAMILIAR_AI_REPORT".to_string(), report.to_string_lossy().to_string()),
    ];
    if trace {
        env.push(("FAMILIAR_AI_TRACE".to_string(), "1".to_string()));
    }
    run_python(py, &["-m", "pytest", "-q"], &d.path, &env, Some(&report))
}

/// The report's lines.
fn lines(report: &str) -> Vec<Value> {
    report.lines().filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{e}: {l}")))
        .collect()
}

/// The calls lines of the report.
fn calls(all: &[Value]) -> Vec<Value> {
    all.iter().filter(|l| l["type"] == "calls").cloned().collect()
}

/// The calls line of the test whose nodeid ends in `suffix`: its (file, function, def line)
/// triples; None when the test has none (no recorded calls, or the recorder off).
fn calls_of(all: &[Value], suffix: &str) -> Option<Vec<(String, String, i64)>> {
    let line = all.iter().find(|l| l["type"] == "calls" && l["id"].as_str().is_some_and(|i| i.ends_with(suffix)))?;
    Some(line["funcs"].as_array().expect("a calls line has its funcs")
        .iter()
        .map(|r| (r[0].as_str().unwrap().to_string(), r[1].as_str().unwrap().to_string(), r[2].as_i64().unwrap()))
        .collect())
}

/// Every call index a calls line names is the per-process counter: set, and greater than 0.
fn positive_indexes(all: &[Value]) {
    for l in calls(all) {
        for r in l["funcs"].as_array().expect("funcs") {
            assert!(r[3].as_u64().is_some_and(|i| i > 0), "an unset or 0 call index: {l:?}");
        }
    }
}

#[test]
fn without_the_switch_the_report_is_as_it_is_and_has_no_calls_lines() {
    let found = with_pytest();
    if found.is_empty() {
        eprintln!("no python with pytest: not run");
        return;
    }
    for (py, major, minor) in found {
        let d = project(&[("tests/test_calls.py", CALLS_TESTS)]);
        let live = run(&py, &d, false, None);
        let parsed = lines(live.report.as_deref().expect("the report"));
        assert!(parsed.iter().all(|l| l["type"] != "calls"), "{py} ({major}.{minor}): a calls line with the switch off: exit {}, stderr {}", live.exit, live.stderr);
        assert_eq!(schema_truth::read(live.report.as_deref(), live.exit), Reading::is("tests_failed", Some("assertion")), "{py} ({major}.{minor}): exit {}, stderr {}", live.exit, live.stderr);
    }
}

#[test]
fn a_test_gets_the_calls_it_made_and_not_the_tests_folder() {
    let found = with_pytest();
    if found.is_empty() {
        eprintln!("no python with pytest: not run");
        return;
    }
    for (py, major, minor) in found {
        let d = project(&[("tests/test_calls.py", CALLS_TESTS)]);
        let live = run(&py, &d, true, None);
        let parsed = lines(live.report.as_deref().expect("the report"));
        // the failing test called f, which called g: its line names both, from the module's own file
        assert_eq!(calls_of(&parsed, "test_fails"), Some(vec![("src/pkg/mod.py".into(), "f".into(), 1), ("src/pkg/mod.py".into(), "g".into(), 4)]), "{py} ({major}.{minor}): exit {}, stderr {}", live.exit, live.stderr);
        // the passing test called h only
        assert_eq!(calls_of(&parsed, "test_passes"), Some(vec![("src/pkg/mod.py".into(), "h".into(), 7)]), "{py} ({major}.{minor}): exit {}, stderr {}", live.exit, live.stderr);
        // and nothing from the tests' folder (or from outside the project)
        for l in calls(&parsed) {
            for r in l["funcs"].as_array().expect("funcs") {
                assert!(!r[0].as_str().unwrap_or("").starts_with("tests/"), "{py} ({major}.{minor}): a call from the tests' folder: {l:?}");
            }
        }
        positive_indexes(&parsed);
    }
}

#[test]
fn a_child_python_the_test_starts_is_recorded_on_the_tests_line() {
    let found = with_pytest();
    if found.is_empty() {
        eprintln!("no python with pytest: not run");
        return;
    }
    for (py, major, minor) in found {
        let d = project(&[("tests/test_child.py", CHILD_TESTS)]);
        let live = run(&py, &d, true, None);
        let parsed = lines(live.report.as_deref().expect("the report"));
        // the parent never calls g: its child does, and the child's calls go on the test's line
        assert_eq!(calls_of(&parsed, "test_child"), Some(vec![("src/pkg/mod.py".into(), "g".into(), 4)]), "{py} ({major}.{minor}): exit {}, stderr {}", live.exit, live.stderr);
        assert_eq!(schema_truth::read(live.report.as_deref(), live.exit), Reading::is("passed", None), "{py} ({major}.{minor}): exit {}, stderr {}", live.exit, live.stderr);
    }
}

#[test]
fn a_projects_own_sitecustomize_still_runs_beside_the_recorder() {
    let found = with_pytest();
    if found.is_empty() {
        eprintln!("no python with pytest: not run");
        return;
    }
    for (py, major, minor) in found {
        let person = Dir::new("person-site");
        person.put("sitecustomize.py", PERSON_SITE);
        let d = project(&[("tests/test_calls.py", CALLS_TESTS)]);
        let live = run(&py, &d, true, Some(&person.path));
        assert!(person.path.join("ran").exists(), "{py} ({major}.{minor}): the project's sitecustomize did not run (the kit's shadowed it)");
        let parsed = lines(live.report.as_deref().expect("the report"));
        // the chain did not break the recorder
        assert_eq!(calls_of(&parsed, "test_passes"), Some(vec![("src/pkg/mod.py".into(), "h".into(), 7)]), "{py} ({major}.{minor}): exit {}, stderr {}", live.exit, live.stderr);
        positive_indexes(&parsed);
    }
}

#[test]
fn a_test_that_clears_the_profiler_does_not_blind_the_next_test() {
    let found = with_pytest();
    if found.is_empty() {
        eprintln!("no python with pytest: not run");
        return;
    }
    for (py, major, minor) in found {
        let d = project(&[("tests/test_clears.py", CLEARS_TESTS)]);
        let live = run(&py, &d, true, None);
        let parsed = lines(live.report.as_deref().expect("the report"));
        // test_clears calls only a function of the tests' folder, then clears the profiler: no line for it
        assert_eq!(calls_of(&parsed, "test_clears"), None, "{py} ({major}.{minor}): exit {}, stderr {}", live.exit, live.stderr);
        // the plugin re-arms the profiler at the start of the next test's call
        assert_eq!(calls_of(&parsed, "test_after"), Some(vec![("src/pkg/mod.py".into(), "g".into(), 4)]), "{py} ({major}.{minor}): exit {}, stderr {}", live.exit, live.stderr);
    }
}
