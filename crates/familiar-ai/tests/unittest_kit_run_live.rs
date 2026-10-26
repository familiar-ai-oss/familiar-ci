//! Lane AKunittest (plan of record 2026-10-19, A-2): the shipped `unittest` kit, run live, not
//! as recorded. `unittest_probe_runs_read_their_truth.rs` proves the spike's recordings; this
//! proves the file the binary will ship (`kit/unittest/sitecustomize.py`, through
//! `door::kits::unittest`) on every Python this machine has: the probes read their truths, the
//! person's run is untouched, the file speaks only the schema's words, and a run the kit could
//! not follow to its end is never read.
//!
//! ```text
//!   plain run ───────────────┐
//!                            ├─ same exit, same stdout, same stderr (the kit never prints, never raises)
//!   run + READERS[0] env ────┘
//!        └─ $FAMILIAR_AI_REPORT ─▶ read() ─▶ the probe's truth, or "next reader"
//! ```

mod schema_truth;

use familiar_ai::door::kits::unittest as kit;
use schema_truth::{pythons, read, run_python, without_timing, Dir, Live, Reading};
use serde_json::Value;
use std::path::Path;

/// The kit's files written where the cache would put them.
fn kit_folder() -> Dir {
    let d = Dir::new("kit");
    for (path, contents) in kit::KIT_FILES {
        d.put(path, contents);
    }
    d
}

/// The environment the reporter reader asks for: its operations over the person's own values.
fn reporter_env(kit: &Path, report: &Path, person: &[(String, String)]) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = person.to_vec();
    for (var, op, value, sep) in kit::READERS[0].2 {
        let value = value.replace("{kit}", &kit.to_string_lossy()).replace("{report}", &report.to_string_lossy());
        let current = env.iter().find(|(k, _)| k == var).map(|(_, v)| v.clone()).filter(|v| !v.is_empty());
        let next = match *op {
            "set" => Some(value),
            "unset" => None,
            "append" => Some(current.map_or_else(|| value.clone(), |c| format!("{c}{sep}{value}"))),
            "prepend" => Some(current.map_or_else(|| value.clone(), |c| format!("{value}{sep}{c}"))),
            other => panic!("an op the data module should not have: {other}"),
        };
        env.retain(|(k, _)| k != var);
        if let Some(v) = next {
            env.push((var.to_string(), v));
        }
    }
    env
}

fn plain(python: &str, args: &[&str], project: &Path, person: &[(String, String)]) -> Live {
    run_python(python, args, project, person, None)
}

fn with_kit(python: &str, args: &[&str], project: &Path, person: &[(String, String)]) -> Live {
    let kit = kit_folder();
    let rep = Dir::new("report");
    let report = rep.path.join("results.jsonl");
    run_python(python, args, project, &reporter_env(&kit.path, &report, person), Some(&report))
}

/// A project folder holding one probe's files.
fn project_of(probe: &kit::Probe) -> Dir {
    let d = Dir::new(probe.0);
    for (name, contents) in probe.2 {
        d.put(name, contents);
    }
    d
}

fn probe(id: &str) -> &'static kit::Probe {
    kit::PROBES.iter().find(|p| p.0 == id).unwrap_or_else(|| panic!("no probe {id}"))
}

fn lines(report: &str) -> Vec<Value> {
    report.lines().filter(|l| !l.trim().is_empty()).map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{e}: {l}"))).collect()
}

fn of_type<'a>(lines: &'a [Value], ty: &str) -> Vec<&'a Value> {
    lines.iter().filter(|l| l["type"] == ty).collect()
}

/// Runs `files` as a project under `python -m unittest <args>` with the kit.
fn scenario(python: &str, files: &[(&str, &str)], args: &[&str]) -> (Live, Vec<Value>) {
    let project = Dir::new("scenario");
    for (name, contents) in files {
        project.put(name, contents);
    }
    let mut argv = vec!["-m", "unittest"];
    argv.extend_from_slice(args);
    let live = with_kit(python, &argv, &project.path, &[]);
    let parsed = live.report.as_deref().map(lines).unwrap_or_default();
    (live, parsed)
}

fn each_python() -> Vec<(String, u32, u32)> {
    let found = pythons();
    assert!(!found.is_empty(), "python3 is not on PATH: the kit cannot be run");
    found
}

#[test]
fn every_probe_reads_its_truth_live_on_every_python_found() {
    for (py, major, minor) in each_python() {
        for p in kit::PROBES {
            let project = project_of(p);
            let live = with_kit(&py, &["-m", "unittest", p.3], &project.path, &[]);
            let want = Reading::is(p.4, p.5);
            assert_eq!(read(live.report.as_deref(), live.exit), want, "{py} ({major}.{minor}) {}: exit {}, stderr {}", p.0, live.exit, live.stderr);
        }
    }
}

#[test]
fn p7_is_located_inside_the_probes_function() {
    let (file, function) = probe("P7").6.and_then(|p| p.split_once("::")).expect("P7 names its place");
    for (py, major, minor) in each_python() {
        let project = project_of(probe("P7"));
        let live = with_kit(&py, &["-m", "unittest", probe("P7").3], &project.path, &[]);
        let parsed = lines(live.report.as_deref().expect("a file"));
        let failed = of_type(&parsed, "test");
        let deepest = failed[0]["error"]["frames"].as_array().and_then(|f| f.last()).cloned().unwrap_or(Value::Null);
        assert_eq!((deepest["file"].as_str(), deepest["function"].as_str()), (Some(file), Some(function)), "{py} ({major}.{minor}): the deepest project frame");
    }
}

#[test]
fn the_persons_run_is_untouched_by_the_kit() {
    for (py, major, minor) in each_python() {
        for p in kit::PROBES {
            let project = project_of(p);
            let args = ["-m", "unittest", p.3];
            let bare = plain(&py, &args, &project.path, &[]);
            let kitted = with_kit(&py, &args, &project.path, &[]);
            assert_eq!(kitted.exit, bare.exit, "{py} ({major}.{minor}) {}: the exit", p.0);
            assert_eq!(without_timing(&kitted.stdout), without_timing(&bare.stdout), "{py} ({major}.{minor}) {}: stdout", p.0);
            assert_eq!(without_timing(&kitted.stderr), without_timing(&bare.stderr), "{py} ({major}.{minor}) {}: stderr", p.0);
        }
    }
}

#[test]
fn the_end_lines_exit_is_the_exit_the_process_had() {
    // python 3.12 made "no tests ran" exit 5; before it, 0. The file says what this python did.
    for (py, major, minor) in each_python() {
        for p in kit::PROBES {
            let project = project_of(p);
            let live = with_kit(&py, &["-m", "unittest", p.3], &project.path, &[]);
            let parsed = lines(live.report.as_deref().expect("a file"));
            let end = parsed.last().expect("an end line");
            assert_eq!(end["type"], "end");
            assert_eq!(end["exit"].as_i64(), Some(live.exit as i64), "{py} ({major}.{minor}) {}: the end line's exit against the process's", p.0);
        }
    }
}

#[test]
fn the_persons_own_sitecustomize_runs_beside_the_kit() {
    for (py, major, minor) in each_python() {
        let person = Dir::new("person");
        person.put("sitecustomize.py", "import os\nopen(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'ran'), 'w').write('yes')\n");
        let project = project_of(probe("P1"));
        let inherited = vec![("PYTHONPATH".to_string(), person.path.to_string_lossy().to_string())];
        let live = with_kit(&py, &["-m", "unittest", probe("P1").3], &project.path, &inherited);
        assert!(person.path.join("ran").exists(), "{py} ({major}.{minor}): the person's sitecustomize did not run");
        assert_eq!(read(live.report.as_deref(), live.exit), Reading::is("passed", None), "{py} ({major}.{minor})");
    }
}

#[test]
fn python_dash_i_and_dash_e_load_nothing_and_change_nothing() {
    for (py, major, minor) in each_python() {
        for flag in ["-I", "-E"] {
            for id in ["P1", "P2"] {
                let project = project_of(probe(id));
                let args = [flag, "-m", "unittest"];
                let bare = plain(&py, &args, &project.path, &[]);
                let kitted = with_kit(&py, &args, &project.path, &[]);
                assert_eq!(kitted.report, None, "{py} ({major}.{minor}) {flag} {id}: the kit loaded under a flag that ignores PYTHONPATH");
                assert_eq!((kitted.exit, without_timing(&kitted.stdout), without_timing(&kitted.stderr)), (bare.exit, without_timing(&bare.stdout), without_timing(&bare.stderr)), "{py} ({major}.{minor}) {flag} {id}");
            }
        }
    }
}

#[test]
fn without_a_report_path_the_kit_does_nothing() {
    for (py, major, minor) in each_python() {
        let kit = kit_folder();
        let project = project_of(probe("P2"));
        let args = ["-m", "unittest", probe("P2").3];
        let bare = plain(&py, &args, &project.path, &[]);
        let before: Vec<_> = std::fs::read_dir(&project.path).unwrap().flatten().map(|e| e.file_name()).filter(|n| n != "__pycache__").collect();
        let loaded = run_python(&py, &args, &project.path, &[("PYTHONPATH".to_string(), kit.path.to_string_lossy().to_string())], None);
        let after: Vec<_> = std::fs::read_dir(&project.path).unwrap().flatten().map(|e| e.file_name()).filter(|n| n != "__pycache__").collect();
        assert_eq!(before.len(), after.len(), "{py} ({major}.{minor}): the kit wrote a file nobody asked for");
        assert_eq!((loaded.exit, without_timing(&loaded.stdout), without_timing(&loaded.stderr)), (bare.exit, without_timing(&bare.stdout), without_timing(&bare.stderr)), "{py} ({major}.{minor})");
    }
}

#[test]
fn a_report_path_that_cannot_be_written_is_not_the_persons_problem() {
    for (py, major, minor) in each_python() {
        let kit = kit_folder();
        let project = project_of(probe("P2"));
        let args = ["-m", "unittest", probe("P2").3];
        let bare = plain(&py, &args, &project.path, &[]);
        for report in [project.path.join("no/such/folder/results.jsonl"), project.path.clone()] {
            let env = vec![("PYTHONPATH".to_string(), kit.path.to_string_lossy().to_string()), ("FAMILIAR_AI_REPORT".to_string(), report.to_string_lossy().to_string())];
            let live = run_python(&py, &args, &project.path, &env, None);
            assert_eq!((live.exit, without_timing(&live.stdout), without_timing(&live.stderr)), (bare.exit, without_timing(&bare.stdout), without_timing(&bare.stderr)), "{py} ({major}.{minor}) {}", report.display());
        }
    }
}

const VOCABULARY: &str = "import unittest\n\n\nclass T(unittest.TestCase):\n    def test_ok(self):\n        self.assertTrue(True)\n\n    @unittest.expectedFailure\n    def test_xfail(self):\n        self.assertEqual(1, 2)\n\n    @unittest.expectedFailure\n    def test_xpass(self):\n        self.assertEqual(1, 1)\n";

#[test]
fn the_file_speaks_only_the_schemas_words() {
    // outcome: passed|failed|skipped|error; counts: passed, failed, skipped, deselected, errors.
    // An expected failure did not pass (skipped, with its reason); an unexpected success failed the run.
    for (py, major, minor) in each_python() {
        let (live, parsed) = scenario(&py, &[("test_vocab.py", VOCABULARY)], &["test_vocab.py"]);
        assert_eq!(live.exit, 1, "{py}: an unexpected success fails unittest's run");
        for t in of_type(&parsed, "test") {
            assert!(["passed", "failed", "skipped", "error"].contains(&t["outcome"].as_str().unwrap_or("")), "{py} ({major}.{minor}): outcome {}", t["outcome"]);
        }
        let end = parsed.last().expect("an end line");
        let mut keys: Vec<&str> = end["counts"].as_object().expect("counts").keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["deselected", "errors", "failed", "passed", "skipped"], "{py} ({major}.{minor})");
        let by_name = |n: &str| of_type(&parsed, "test").into_iter().find(|t| t["id"].as_str().is_some_and(|i| i.ends_with(n))).cloned().unwrap_or_else(|| panic!("no test line for {n}"));
        assert_eq!((by_name("test_xfail")["outcome"].as_str(), by_name("test_xfail")["skip_reason"].as_str()), (Some("skipped"), Some("expected failure")), "{py} ({major}.{minor})");
        assert_eq!((by_name("test_xpass")["outcome"].as_str(), by_name("test_xpass")["error"]["type"].as_str()), (Some("failed"), Some("UnexpectedSuccess")), "{py} ({major}.{minor})");
        assert_eq!(read(live.report.as_deref(), live.exit), Reading::is("tests_failed", Some("exception")), "{py} ({major}.{minor})");
    }
}

const SUBTESTS: &str = "import unittest\n\n\nclass T(unittest.TestCase):\n    def test_many(self):\n        for i in range(3):\n            with self.subTest(i=i):\n                self.assertEqual(i, 1)\n";

#[test]
fn a_failing_subtest_is_located_in_the_tests_own_file() {
    for (py, major, minor) in each_python() {
        let (live, parsed) = scenario(&py, &[("test_sub.py", SUBTESTS)], &["test_sub.py"]);
        let tests = of_type(&parsed, "test");
        assert_eq!(tests.len(), 2, "{py} ({major}.{minor}): the subtests i=0 and i=2 failed");
        for t in tests {
            assert_eq!(t["file"].as_str(), Some("test_sub.py"), "{py} ({major}.{minor}): a subtest's file is its test case's, not unittest's own");
            assert_eq!(t["error"]["kind"].as_str(), Some("assertion"), "{py} ({major}.{minor})");
            assert_eq!(t["error"]["frames"][0]["file"].as_str(), Some("test_sub.py"), "{py} ({major}.{minor})");
        }
        assert_eq!(read(live.report.as_deref(), live.exit), Reading::is("tests_failed", Some("assertion")), "{py} ({major}.{minor})");
    }
}

const DIES: &str = "import os\nimport unittest\n\n\nclass T(unittest.TestCase):\n    def test_a_first(self):\n        pass\n\n    def test_b_dies(self):\n        os._exit(7)\n";

#[test]
fn a_run_that_dies_midway_leaves_no_end_and_is_not_read() {
    for (py, major, minor) in each_python() {
        let (live, parsed) = scenario(&py, &[("test_dies.py", DIES)], &["test_dies.py"]);
        assert_eq!(live.exit, 7, "{py}");
        assert_eq!(of_type(&parsed, "session").len(), 1, "{py} ({major}.{minor}): the file starts when the run does");
        assert!(of_type(&parsed, "end").is_empty(), "{py} ({major}.{minor}): an end line for a run that did not end");
        assert!(matches!(read(live.report.as_deref(), live.exit), Reading::Next(_)), "{py} ({major}.{minor}): read a run that never finished");
    }
}

const NO_SETUP: &str = "import unittest\n\n\nclass T(unittest.TestCase):\n    @classmethod\n    def setUpClass(cls):\n        raise RuntimeError('no database')\n\n    def test_a(self):\n        pass\n";

#[test]
fn a_class_that_cannot_set_up_reads_setup_error() {
    for (py, major, minor) in each_python() {
        let (live, _) = scenario(&py, &[("test_setup.py", NO_SETUP)], &["test_setup.py"]);
        assert_eq!(read(live.report.as_deref(), live.exit), Reading::is("setup_error", None), "{py} ({major}.{minor}): {}", live.stderr);
    }
}

const SPAWNS: &str = "import os\nimport subprocess\nimport sys\nimport unittest\n\n\nclass T(unittest.TestCase):\n    def test_spawns_a_python_that_runs_unittest(self):\n        here = os.path.dirname(os.path.abspath(__file__))\n        done = subprocess.run([sys.executable, '-m', 'unittest', 'test_other'], cwd=here, capture_output=True)\n        self.assertEqual(done.returncode, 0)\n";
const OTHER: &str = "import unittest\n\n\nclass U(unittest.TestCase):\n    def test_other(self):\n        pass\n";

#[test]
fn a_python_the_test_starts_does_not_write_into_the_file() {
    for (py, major, minor) in each_python() {
        let (live, parsed) = scenario(&py, &[("test_spawns.py", SPAWNS), ("test_other.py", OTHER)], &["test_spawns.py"]);
        assert_eq!(of_type(&parsed, "session").len(), 1, "{py} ({major}.{minor}): one session, the process Familiar started");
        assert_eq!(of_type(&parsed, "test").len(), 1, "{py} ({major}.{minor}): the child's tests are not this run's");
        assert_eq!(read(live.report.as_deref(), live.exit), Reading::is("passed", None), "{py} ({major}.{minor})");
    }
}

const WRAPPER: &str = "import subprocess\nimport sys\n\ndone = subprocess.run([sys.executable, '-m', 'unittest', 'test_wrapped.py'])\nsys.exit(done.returncode)\n";
const WRAPPED: &str = "import unittest\n\n\nclass T(unittest.TestCase):\n    def test_wrapped(self):\n        self.assertEqual(1, 1)\n";

#[test]
fn a_python_that_only_starts_the_run_does_not_take_it_from_the_run() {
    // poetry run, tox, nox, hatch run: a python program starts `python -m unittest` as a child.
    // The wrapper loads the kit too (PYTHONPATH is inherited) but runs no tests: the child is the run.
    for (py, major, minor) in each_python() {
        let project = Dir::new("wrapper");
        project.put("wrap.py", WRAPPER).put("test_wrapped.py", WRAPPED);
        let live = with_kit(&py, &["wrap.py"], &project.path, &[]);
        assert_eq!(live.exit, 0, "{py} ({major}.{minor}): {}", live.stderr);
        assert_eq!(read(live.report.as_deref(), live.exit), Reading::is("passed", None), "{py} ({major}.{minor}): the wrapper took the file and the run wrote nothing");
    }
}

const STARTS_CLEAN: &str = "import subprocess\nimport sys\nimport unittest\n\n\nclass T(unittest.TestCase):\n    def test_a_python_it_starts_pays_nothing_for_the_kit(self):\n        code = \"import sys; print('unittest' in sys.modules)\"\n        done = subprocess.run([sys.executable, '-c', code], capture_output=True, text=True)\n        self.assertEqual(done.stdout.strip(), 'False')\n";

#[test]
fn a_python_a_test_starts_pays_nothing_for_the_kit() {
    // a test that starts many pythons (a CLI under test) must not pay the kit's imports in each:
    // the run's process has taken the file, so a python it starts returns before importing unittest
    for (py, major, minor) in each_python() {
        let (live, _) = scenario(&py, &[("test_starts_clean.py", STARTS_CLEAN)], &["test_starts_clean.py"]);
        assert_eq!(read(live.report.as_deref(), live.exit), Reading::is("passed", None), "{py} ({major}.{minor}): {}", live.stderr);
    }
}
