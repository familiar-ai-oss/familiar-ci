//! The per-test result file (plan A, JSON Lines v1 [draft]) that Familiar's own reporter writes,
//! read into a translation `by: "reporter"`, first match wins:
//!
//! 1. no session line, no end line, a kit error, or no exit code (a signal, a timeout) → the
//!    next reader (`None`); an end whose `exit` is null (a kit the runner never tells) takes the
//!    process's;
//! 2. a line that is not JSON, a test twice, or an end contradicting the exit code → `unclassified`;
//! 3. a collection error: a module missing where the project's code (not the test file) imports it
//!    → `environment_error`; naming a missing name → `missing_target_symbol`; another → `collection_error`;
//! 4. a test's setup error → `setup_error`; a test's missing name → `missing_target_symbol`;
//! 5. a failed test → `tests_failed` (an assertion at its assert line, an exception at its deepest
//!    project frame first);
//! 6. one passed and none failed → `passed`; nothing ran → `no_tests_collected`.

use familiar_ai_facts::{Counts, Location, LocationKind};
use serde_json::Value;

use crate::door::kept::Translation;

/// What the reporter's file says about a run: its translation and, filtered to the plan's test
/// files, the tests that ran and were skipped; with the runner's and its plugins' versions.
#[derive(Debug, Clone, PartialEq)]
pub struct Reported {
    pub translation: Translation,
    /// The deciding error's frames as the file recorded them, from the test down to where it was
    /// raised (`test_log_translation.frames` [draft]); empty when it recorded none.
    pub frames: Vec<Location>,
    pub ran_tests: Vec<String>,
    pub skipped_tests: Vec<String>,
    pub versions: String,
}

fn text<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

/// The name a missing-name message names: `cannot import name 'x'`, `has no attribute 'x'`,
/// `name 'x' is not defined`.
fn missing_name(message: &str) -> Option<String> {
    for lead in ["cannot import name '", "has no attribute '", "name '"] {
        if let Some(i) = message.find(lead) {
            let rest = &message[i + lead.len()..];
            return rest.find('\'').map(|j| rest[..j].to_string());
        }
    }
    None
}

/// The places an error points, most wanted first: an assertion's deepest frame in the test's
/// own file (the assert line), else an exception's frames deepest first.
fn places(error: &Value, own_file: Option<&str>) -> Vec<Location> {
    let frames: Vec<&Value> = error.get("frames").and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
    let loc = |f: &Value, kind: LocationKind| -> Option<Location> {
        let line = f.get("line").and_then(Value::as_u64)?;
        Some(Location { file_path: text(f, "file").to_string(), line, function: None, kind: Some(kind) })
    };
    let assertion = text(error, "kind") == "assertion";
    if assertion {
        if let Some(l) = frames.iter().rev().filter(|f| own_file.is_none_or(|o| text(f, "file") == o)).find_map(|f| loc(f, LocationKind::Assertion)) {
            return vec![l];
        }
    }
    frames.iter().rev().filter_map(|f| loc(f, if assertion { LocationKind::Assertion } else { LocationKind::Exception })).collect()
}

/// The error's frames in the file's order (the test first, where it was raised last), each
/// function as the runner named it; at most `FRAMES_KEPT`, the first and the last half kept.
fn call_path(error: &Value) -> Vec<Location> {
    let all: Vec<Location> = error
        .get("frames")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|f| {
                    let line = f.get("line").and_then(Value::as_u64)?;
                    let file = text(f, "file");
                    let function = text(f, "function");
                    (!file.is_empty()).then(|| Location { file_path: file.to_string(), line, function: (!function.is_empty()).then(|| function.to_string()), kind: None })
                })
                .collect()
        })
        .unwrap_or_default();
    let keep = familiar_ai_facts::FRAMES_KEPT;
    if all.len() <= keep {
        return all;
    }
    let half = keep / 2;
    all[..half].iter().chain(all[all.len() - half..].iter()).cloned().collect()
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or("")
}

/// Read the reporter's file `report` for a run that exited `exit` (`None` when a signal or a
/// timeout ended it: the exit code decides, and the next reader reads); `plan_files` are the
/// plan's test files, which `ran_tests` and `skipped_tests` are filtered to.
pub fn read(report: &str, exit: Option<i32>, plan_files: &[String]) -> Option<Reported> {
    let exit = exit?;
    let mut lines: Vec<Result<Value, ()>> = Vec::new();
    for l in report.lines().filter(|l| !l.trim().is_empty()) {
        lines.push(serde_json::from_str::<Value>(l).map_err(|_| ()));
    }
    let parsed: Vec<&Value> = lines.iter().filter_map(|l| l.as_ref().ok()).collect();
    let of = |kind: &str| -> Vec<&Value> { parsed.iter().copied().filter(|v| text(v, "type") == kind).collect() };
    let session = *of("session").first()?;
    let end = *of("end").last()?;
    if !of("kit_error").is_empty() {
        return None;
    }
    // a kit the runner never tells its exit (PHPUnit's extension) writes `exit: null`: the
    // process's own is the run's
    let file_exit = match end.get("exit") {
        Some(Value::Null) => i64::from(exit),
        e => e.and_then(Value::as_i64)?,
    };
    let n = |k: &str| end.get("counts").and_then(|c| c.get(k)).and_then(Value::as_u64);
    let counts = Counts { passed: n("passed"), failed: n("failed").map(|f| f + n("errors").unwrap_or(0)), skipped: n("skipped"), deselected: n("deselected") };
    let tests: Vec<&Value> = of("test");
    let collect_errors: Vec<&Value> = of("collect_error");
    let in_plan = |t: &&&Value| plan_files.iter().any(|p| p == text(t, "file"));
    let ids = |outcome: &str| -> Vec<String> { tests.iter().filter(in_plan).filter(|t| text(t, "outcome") == outcome).map(|t| text(t, "id").to_string()).collect() };
    let plugins: Vec<&str> = session.get("plugins").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    let versions = format!("{} {}; {} {}; {}", text(session, "runner"), text(session, "runner_version"), text(session, "language"), text(session, "language_version"), plugins.join(", "));
    let failed: Vec<&Value> = tests.iter().copied().filter(|t| matches!(text(t, "outcome"), "failed" | "error")).collect();
    let passed = tests.iter().filter(|t| text(t, "outcome") == "passed").count();
    let mut seen = std::collections::HashSet::new();
    let doubled = !tests.iter().all(|t| seen.insert(text(t, "id")));
    let says_failed = !failed.is_empty() || !collect_errors.is_empty();
    // a failing exit with no failure in the file, unless nothing ran (pytest's 5, jest's and
    // vitest's 1 for a file with no test, PHPUnit's)
    let unexplained = exit != 0 && !says_failed && passed > 0;
    let contradicts = (file_exit == 0) != (exit == 0) || (exit == 0 && says_failed) || unexplained;

    let mut t = Translation {
        by: "reporter",
        label: String::new(),
        evidence: String::new(),
        failing_tests: failed.iter().map(|t| text(t, "id").to_string()).collect(),
        location: None,
        missing_name: None,
        candidates: Vec::new(),
        counts,
    };
    let mut frames: Vec<Location> = Vec::new();
    let label = if lines.iter().any(Result::is_err) || doubled || contradicts {
        t.evidence = format!("the reporter's file does not agree with the run (exit {exit}, the file's {file_exit})");
        "unclassified"
    } else if let Some(c) = collect_errors.first() {
        let error = c.get("error").cloned().unwrap_or(Value::Null);
        frames = call_path(&error);
        t.evidence = format!("ERROR collecting {}: {}: {}", text(c, "file"), text(&error, "type"), first_line(text(&error, "message")));
        t.candidates = places(&error, None);
        // a module missing where the project's own code imports it, not the test file: a dependency
        // this environment lacks, as the pytest tables read it (`environment_error`)
        let raised_in = error.get("frames").and_then(Value::as_array).and_then(|f| f.last()).map(|f| text(f, "file"));
        // the collected file may be named from the runner's start folder (unittest's `-s tests`)
        let same = |a: &str, b: &str| a == b || a.ends_with(&format!("/{b}")) || b.ends_with(&format!("/{a}"));
        if text(&error, "type") == "ModuleNotFoundError" && raised_in.is_some_and(|f| !same(f, text(c, "file"))) {
            "environment_error"
        } else if text(&error, "kind") == "missing_name" {
            t.missing_name = missing_name(text(&error, "message"));
            "missing_target_symbol"
        } else {
            "collection_error"
        }
    } else if let Some(f) = failed.first() {
        let error = f.get("error").cloned().unwrap_or(Value::Null);
        frames = call_path(&error);
        t.evidence = format!("FAILED {}: {}: {}", text(f, "id"), text(&error, "type"), first_line(text(&error, "message")));
        let own = text(f, "file");
        t.candidates = places(&error, (!own.is_empty()).then_some(own));
        match text(&error, "kind") {
            "setup" => "setup_error",
            "missing_name" => {
                t.missing_name = missing_name(text(&error, "message"));
                "missing_target_symbol"
            }
            _ => "tests_failed",
        }
    } else if passed > 0 {
        t.evidence = format!("{passed} passed");
        "passed"
    } else {
        t.evidence = "no tests ran".to_string();
        "no_tests_collected"
    };
    t.label = label.to_string();
    t.location = t.candidates.first().cloned();
    Some(Reported { translation: t, frames, ran_tests: ids("passed").into_iter().chain(ids("failed")).chain(ids("error")).collect(), skipped_tests: ids("skipped"), versions })
}
