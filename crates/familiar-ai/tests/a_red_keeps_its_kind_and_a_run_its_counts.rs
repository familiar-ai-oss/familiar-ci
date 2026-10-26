//! Lane H3 (plan of record 2026-10-19, D7): the binary keeps why the output points where it does
//! (`location.kind` [draft]) and what the runner's own summary counted (`counts` [draft]).
//!
//! The location rules for a failed test:
//!
//! ```text
//!  (1) the chosen candidate's kind is stored         (frame → exception)
//!  (2) one file:line from an assertion row and       → assertion
//!      another row
//!  (3) an exception row matched with no file/line    → no location
//!  (4) in-project candidates of different kinds      → no location
//!      at different places
//! ```
//!
//! `counts` is written on every new translation, `{}` when the output counted nothing; the
//! backstop's reading of the cut (`classified_on: cut`) writes none.

use familiar_ai::door::kept::{classify, clean, count, count_compiles, in_project, locations, table_named, test_log_translation, Streams, Translation, TABLES};
use familiar_ai_facts::{Counts, FactFields, Location, LocationKind};
use familiar_ai_run::{details_for, End};
use std::path::{Path, PathBuf};

const RECORDED_AT: &[&str] = &["/tmp/claude-1000/-home-user-projects-familiar-ai-oss/ce46f81f-ec40-4c7d-885d-547ef9d911b3/scratchpad/run_logs", "/r"];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// The run copy the recordings' roots become (as `a_reading_is_the_truth_or_cant_tell.rs`).
const COPY: &str = "/tmp/familiar-ai-run-4242-7";

/// A file of a recorded fixture as a run copy prints it.
fn text(p: &Path, runner: &str, folder: &str) -> String {
    let mut t = clean(&String::from_utf8_lossy(&std::fs::read(p).unwrap_or_default()));
    for at in RECORDED_AT {
        t = t.replace(&format!("{at}/{runner}/{folder}/"), &format!("{COPY}/")).replace(&format!("{at}/{runner}/{folder}"), COPY);
    }
    t
}

/// A recorded fixture as a run copy prints it: (stdout, stderr, report, exit, command).
fn recorded(runner: &str, folder: &str) -> (String, String, String, i32, String) {
    let d = root().join("run_logs").join(runner).join(folder);
    let mut files: Vec<PathBuf> = Vec::new();
    let mut stack = vec![d.join("report")];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            if e.path().is_dir() {
                stack.push(e.path());
            } else {
                files.push(e.path());
            }
        }
    }
    files.sort();
    let report = files.iter().map(|p| text(p, runner, folder)).collect::<Vec<_>>().join("\n");
    let code = std::fs::read_to_string(d.join("exit")).unwrap_or_default().trim().parse().expect("an exit code");
    let cmd = std::fs::read_to_string(d.join("cmd")).unwrap_or_default().trim().to_string();
    (text(&d.join("stdout"), runner, folder), text(&d.join("stderr"), runner, folder), report, code, cmd)
}

/// The translation the binary writes for a recorded fixture, its sources (or its runner's
/// exception fixture's) standing in for git.
fn stored(runner: &str, folder: &str) -> familiar_ai_facts::TestLogTranslation {
    let (stdout, stderr, report, code, cmd) = recorded(runner, folder);
    let t = classify(End::Exited(code), &Streams { stdout: &stdout, stderr: &stderr, report: &report }, details_for(&cmd)).expect("a reading");
    let own = root().join("run_sources").join(runner).join(folder);
    let dir = if own.is_dir() { own } else { root().join("run_sources").join(runner).join("tests_failed_exception") };
    let read = move |p: &str| std::fs::read(dir.join(p)).ok();
    test_log_translation(t, "r1", "c1", "abc", "full", |c| in_project(c, &read))
}

fn at(file: &str, line: u64, kind: LocationKind) -> Location {
    Location { file_path: file.to_string(), line, function: None, kind: Some(kind) }
}

fn translation(label: &str, candidates: Vec<Location>) -> Translation {
    Translation { by: "regex", label: label.to_string(), evidence: String::new(), failing_tests: Vec::new(), location: candidates.first().cloned(), missing_name: None, candidates, counts: Counts::default() }
}

/// Every path but one under node_modules is in the project.
fn project(c: &[Location]) -> Option<Location> {
    c.iter().find(|l| !l.file_path.starts_with("node_modules/")).cloned()
}

#[test]
fn rule_1_the_chosen_candidates_kind_is_stored() {
    use LocationKind::{Assertion, Compile, Exception};
    let t = translation("tests_failed", vec![at("node_modules/x.js", 1, Exception), at("tests/t.py", 2, Assertion)]);
    let got = test_log_translation(t, "r1", "c1", "abc", "full", project);
    assert_eq!(got.location, Some(at("tests/t.py", 2, Assertion)), "the first one inside the project, with its own kind");
    let kind = |r: &str, f: &str| stored(r, f).location.and_then(|l| l.kind);
    assert_eq!(kind("pytest", "tests_failed"), Some(Assertion));
    assert_eq!(kind("pytest", "tests_failed_exception"), Some(Exception));
    assert_eq!(kind("pytest", "missing_target_symbol"), Some(Compile));
    assert_eq!(kind("jest", "tests_failed"), Some(Assertion));
    assert_eq!(kind("go-test", "tests_failed_exception"), Some(Exception));
    // a frame row's place is an exception's
    let s = "\tat calc.Calc.parse(Calc.java:9)\n";
    let (all, _) = locations(table_named("gradle").expect("gradle"), "tests_failed", &Streams { report: s, ..Default::default() });
    assert_eq!(all.iter().map(|l| l.kind).collect::<Vec<_>>(), [Some(Exception)], "{all:?}");
}

#[test]
fn rule_1_the_kind_survives_the_records_round_trip() {
    let t = stored("pytest", "tests_failed");
    let fields = t.to_fields().expect("fields");
    assert_eq!(fields["location"]["kind"], "assertion");
    assert_eq!(fields["counts"], serde_json::json!({"failed": 1}));
    let back: familiar_ai_facts::TestLogTranslation = serde_json::from_value(serde_json::Value::Object(fields)).expect("read back");
    assert_eq!(back, t);
}

#[test]
fn rule_2_one_place_from_an_assertion_row_and_another_row_is_an_assertion() {
    let (stdout, stderr, _, _, _) = recorded("node-test", "tests_failed");
    let s = Streams { stdout: &stdout, stderr: &stderr, report: "" };
    let (all, _) = locations(table_named("node-test").expect("node-test"), "tests_failed", &s);
    let here: Vec<&Location> = all.iter().filter(|l| l.file_path.ends_with("test/calc.test.js") && l.line == 6).collect();
    assert_eq!(here.len(), 1, "one place, once: {all:?}");
    assert_eq!(here[0].kind, Some(LocationKind::Assertion), "the exception row and the assertion row read the same block: {all:?}");
    assert_eq!(stored("node-test", "tests_failed").location.and_then(|l| l.kind), Some(LocationKind::Assertion));
}

#[test]
fn rule_3_an_exception_row_with_no_file_or_line_leaves_no_location() {
    // ctest: the uncaught exception's row (no throw site) comes before glibc assert()'s (a file:line)
    let ctest = table_named("ctest").expect("ctest");
    let both = "terminate called after throwing an instance of 'std::runtime_error'\ncalc_test: src/calc.cpp:12: int parse(): Assertion `x' failed.\n";
    let s = Streams { stdout: both, ..Default::default() };
    assert_eq!(locations(ctest, "tests_failed", &s).0, Vec::<Location>::new(), "the first exception row says no place: none");
    assert!(locations(ctest, "crashed", &s).0.iter().any(|l| l.file_path == "src/calc.cpp"), "rule (3) is a failed test's");
    let alone = Streams { stdout: "calc_test: src/calc.cpp:12: int parse(): Assertion `x' failed.\n", ..Default::default() };
    assert_eq!(locations(ctest, "tests_failed", &alone).0.len(), 1, "a located exception alone keeps its place");
    // the recorded runs the tables' own contract names
    for runner in ["catch2", "ctest", "googletest"] {
        let t = stored(runner, "tests_failed_exception");
        assert_eq!((t.label.as_str(), t.location), ("tests_failed", None), "{runner}");
    }
    // nunit: a block whose stack has no path
    let nunit = table_named("dotnet-nunit").expect("nunit");
    let block = "  Error Message:\n   System.InvalidOperationException : boom\n  Stack Trace:\n     at Tests.CalcTests.Adds()\n\n";
    assert_eq!(locations(nunit, "tests_failed", &Streams { stdout: block, ..Default::default() }).0, Vec::<Location>::new());
}

#[test]
fn rule_4_in_project_places_of_different_kinds_leave_no_location() {
    use LocationKind::{Assertion, Exception};
    let mixed = vec![at("tests/t.py", 2, Assertion), at("src/c.py", 3, Exception)];
    let keep = |label: &str, c: Vec<Location>| test_log_translation(translation(label, c), "r1", "c1", "abc", "full", project).location;
    assert_eq!(keep("tests_failed", mixed.clone()), None, "an assertion here and an exception there: which failed?");
    assert_eq!(keep("crashed", mixed), Some(at("tests/t.py", 2, Assertion)), "rule (4) is a failed test's");
    assert_eq!(keep("tests_failed", vec![at("src/c.py", 3, Exception), at("tests/t.py", 9, Exception)]), Some(at("src/c.py", 3, Exception)), "one kind: the deepest frame first");
    assert_eq!(keep("tests_failed", vec![at("tests/t.py", 2, Assertion), at("node_modules/x.js", 3, Exception)]), Some(at("tests/t.py", 2, Assertion)), "the other kind outside the project");
    assert_eq!(keep("tests_failed", vec![at("node_modules/x.js", 3, Assertion), at("src/c.py", 3, Exception)]), Some(at("src/c.py", 3, Exception)));
}

#[test]
fn counts_add_each_matchs_number_or_one_and_passed_is_the_total_less_the_rest() {
    let pytest = table_named("pytest").expect("pytest");
    let line = "===== 1 failed, 2 passed, 3 skipped, 4 deselected, 1 xfailed, 1 xpassed, 2 errors in 0.12s =====\n";
    let got = count(pytest, &Streams { stdout: line, ..Default::default() });
    assert_eq!(got, Some(Counts { passed: Some(3), failed: Some(3), skipped: Some(4), deselected: Some(4) }), "xpassed passed, xfailed skipped, errors failed");
    let unittest = table_named("unittest").expect("unittest");
    let tail = "Ran 5 tests in 0.001s\n\nFAILED (failures=1, errors=1, skipped=1)\n";
    assert_eq!(count(unittest, &Streams { stderr: tail, ..Default::default() }), Some(Counts { passed: Some(2), failed: Some(2), skipped: Some(1), deselected: None }));
    let go = table_named("go-test").expect("go-test");
    let tally = "--- PASS: TestA (0.00s)\n--- PASS: TestB (0.00s)\n--- FAIL: TestC (0.00s)\n    --- FAIL: TestC/sub (0.00s)\n--- SKIP: TestD (0.00s)\n";
    assert_eq!(count(go, &Streams { stdout: tally, ..Default::default() }), Some(Counts { passed: Some(2), failed: Some(1), skipped: Some(1), deselected: None }), "one per top-level line");
    let cargo = table_named("cargo-test").expect("cargo-test");
    let two = "test result: ok. 3 passed; 0 failed; 1 ignored; 0 measured; 2 filtered out; finished in 0.00s\ntest result: FAILED. 4 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n";
    assert_eq!(count(cargo, &Streams { stdout: two, ..Default::default() }), Some(Counts { passed: Some(7), failed: Some(1), skipped: Some(1), deselected: Some(2) }), "every binary's line, summed");
    assert_eq!(count(pytest, &Streams { stdout: "nothing counted here\n", ..Default::default() }), None);
}

#[test]
fn every_translation_carries_counts_but_the_backstops() {
    let t = || translation("tests_failed", Vec::new());
    let full = test_log_translation(t(), "r1", "c1", "abc", "full", project);
    assert_eq!(full.counts, Some(Counts::default()), "nothing counted is {{}}, the honest-reading marker (D7)");
    assert_eq!(full.to_fields().expect("fields")["counts"], serde_json::json!({}));
    let cut = test_log_translation(t(), "r1", "c1", "abc", "cut", project);
    assert_eq!(cut.counts, None, "the backstop reads the cut: no counts");
    assert!(!cut.to_fields().expect("fields").contains_key("counts"));
    // Familiar's own ends carry them too
    let empty = classify(End::EmptyCommand, &Streams::default(), None).expect("a reading");
    assert_eq!(test_log_translation(empty, "r1", "c1", "abc", "full", project).counts, Some(Counts::default()));
    let timed = classify(End::TimedOut, &Streams { stdout: "# pass 3\n# fail 0\n", ..Default::default() }, details_for("node --test")).expect("a reading");
    assert_eq!((timed.label.as_str(), timed.counts.passed), ("timed_out", Some(3)));
}

#[test]
fn every_runner_but_sh_has_count_rows_and_every_one_compiles() {
    for t in TABLES {
        assert_eq!(t.counts.is_empty(), t.runner == "sh", "{}: count rows", t.runner);
        for i in 0..t.counts.len() {
            assert_eq!(count_compiles(t.runner, i), Some(true), "{} count row {i}", t.runner);
        }
    }
}
