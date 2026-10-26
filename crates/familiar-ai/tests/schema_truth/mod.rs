//! What a reader makes of the per-test result file (plan of record 2026-10-19, "The per-test
//! result file", JSON Lines v1 [draft]; the spike's `kit/SCHEMA.txt`), and the recordings it is
//! proven on. Lane AKunittest's own helper: `door/reported.rs` (lane A-py) will be the real reader;
//! until it lands these tests read through this one, which is the plan's order with the safety
//! rules below, and the tests then switch to the real one.
//!
//! The reading, first match wins:
//!
//! ```text
//!  no file, an empty one, a line that is not JSON object, a kit_error line,
//!  not exactly one session first and one end last, an end whose counts
//!  disagree with the lines                      → Next   (no structured output)
//!  the exit contradicts the lines               → unclassified (can't tell)
//!  a collect_error missing_name                 → missing_target_symbol
//!  another collect_error                        → collection_error
//!  a failed test of kind setup                  → setup_error
//!  a failed test of kind missing_name           → missing_target_symbol
//!  a failed test                                → tests_failed / the first one's kind
//!  one passed test or more                      → passed
//!  no test line at all                          → no_tests_collected
//!  tests, all skipped                           → unclassified
//! ```
//!
//! Beyond the plan's order, this reader asks (`Next` or `unclassified`) wherever the plan's would
//! have to guess: a file with a kit error, a dropped line, a second session, a nothing-ran run
//! whose exit is neither 0 nor 5, a run of skipped tests only.

#![allow(dead_code)]

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// What the file reads as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reading {
    /// No structured output the reader can trust: the next reader takes over.
    Next(&'static str),
    /// A label of the outcome vocabulary, with the kind of the failed test's error when it has one.
    Is(String, Option<String>),
}

impl Reading {
    /// The label as the spike's summary prints it: `tests_failed/assertion`, `passed`, `next_reader`.
    pub fn word(&self) -> String {
        match self {
            Reading::Next(_) => "next_reader".to_string(),
            Reading::Is(label, None) => label.clone(),
            Reading::Is(label, Some(kind)) => format!("{label}/{kind}"),
        }
    }

    pub fn is(label: &str, kind: Option<&str>) -> Reading {
        Reading::Is(label.to_string(), kind.map(str::to_string))
    }
}

fn kind_of(v: &Value) -> Option<String> {
    v.get("error").and_then(|e| e.get("kind")).and_then(Value::as_str).map(str::to_string)
}

/// Reads the per-test result file `file` of a run that exited `exit`.
pub fn read(file: Option<&str>, exit: i32) -> Reading {
    let Some(text) = file else { return Reading::Next("no file") };
    let mut lines: Vec<Value> = Vec::new();
    for raw in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        match serde_json::from_str::<Value>(raw) {
            Ok(v) if v.is_object() => lines.push(v),
            _ => return Reading::Next("a line that is not a JSON object"),
        }
    }
    if lines.is_empty() {
        return Reading::Next("an empty file");
    }
    let ty = |l: &Value| l.get("type").and_then(Value::as_str).unwrap_or("").to_string();
    if lines.iter().any(|l| ty(l) == "kit_error") {
        return Reading::Next("the kit reported an error");
    }
    if ty(&lines[0]) != "session" || lines.iter().filter(|l| ty(l) == "session").count() != 1 {
        return Reading::Next("no single session line first");
    }
    let end = lines.last().filter(|l| ty(l) == "end").cloned();
    let Some(end) = end else { return Reading::Next("no end line last") };
    if lines.iter().filter(|l| ty(l) == "end").count() != 1 {
        return Reading::Next("more than one end line");
    }
    let tests: Vec<&Value> = lines.iter().filter(|l| ty(l) == "test").collect();
    let collect_errors: Vec<&Value> = lines.iter().filter(|l| ty(l) == "collect_error").collect();
    let outcome = |t: &&Value| t.get("outcome").and_then(Value::as_str).unwrap_or("").to_string();
    let count_outcome = |o: &str| tests.iter().filter(|t| outcome(t) == o).count() as u64;
    let failed: Vec<&Value> = tests.iter().copied().filter(|t| matches!(outcome(t).as_str(), "failed" | "error")).collect();
    let counts = end.get("counts").cloned().unwrap_or(Value::Null);
    let counted = |k: &str| counts.get(k).and_then(Value::as_u64).unwrap_or(0);
    if counted("passed") != count_outcome("passed")
        || counted("failed") != count_outcome("failed")
        || counted("skipped") != count_outcome("skipped")
        || counted("errors") != count_outcome("error") + collect_errors.len() as u64
    {
        return Reading::Next("the end's counts disagree with the lines");
    }
    let passed = count_outcome("passed");
    let skipped = count_outcome("skipped");
    let bad = !failed.is_empty() || !collect_errors.is_empty();
    let ran_nothing = tests.is_empty() && collect_errors.is_empty();
    // exit 0 with a failure; a failing exit with tests that all went well; nothing ran, but not exit 5
    let contradicts = if exit == 0 {
        bad
    } else if ran_nothing {
        exit != 5
    } else {
        !bad
    };
    if contradicts {
        return Reading::is("unclassified", None);
    }
    if !collect_errors.is_empty() {
        let missing = collect_errors.iter().any(|c| kind_of(c).as_deref() == Some("missing_name"));
        return Reading::is(if missing { "missing_target_symbol" } else { "collection_error" }, None);
    }
    if failed.iter().any(|t| kind_of(t).as_deref() == Some("setup")) {
        return Reading::is("setup_error", None);
    }
    if failed.iter().any(|t| kind_of(t).as_deref() == Some("missing_name")) {
        return Reading::is("missing_target_symbol", None);
    }
    if let Some(first) = failed.first() {
        return Reading::Is("tests_failed".to_string(), kind_of(first));
    }
    if passed >= 1 {
        return Reading::is("passed", None);
    }
    if ran_nothing {
        return Reading::is("no_tests_collected", None);
    }
    debug_assert!(skipped >= 1);
    Reading::is("unclassified", None)
}

/// A recorded probe run, as the spike's `probe_run.py` wrote it.
pub struct Recorded {
    pub exit: i32,
    pub report: Option<String>,
    pub cmd: String,
    pub person_cmd: String,
    pub person_site: Option<String>,
}

pub fn fixtures(runner: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/probe_runs").join(runner)
}

/// The recording of `probe` under `setup`: exit, the result file when the kit wrote one, the command.
pub fn recorded(runner: &str, setup: &str, probe: &str) -> Recorded {
    let d = fixtures(runner).join(setup).join(probe);
    let text = |n: &str| std::fs::read_to_string(d.join(n)).ok();
    Recorded {
        exit: text("exit").unwrap_or_default().trim().parse().unwrap_or_else(|_| panic!("{}: an exit code", d.display())),
        report: text("report/results.jsonl"),
        cmd: text("cmd").unwrap_or_default().trim().to_string(),
        person_cmd: text("person_cmd").unwrap_or_default().trim().to_string(),
        person_site: text("person_site"),
    }
}

/// A scratch folder, removed on drop.
pub struct Dir {
    pub path: PathBuf,
}

impl Dir {
    pub fn new(name: &str) -> Dir {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!("familiar-ai-kit-test-{}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst), name));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("a scratch folder");
        Dir { path }
    }

    /// Writes `contents` at `rel` (made with its folders).
    pub fn put(&self, rel: &str, contents: &str) -> &Dir {
        let p = self.path.join(rel);
        std::fs::create_dir_all(p.parent().expect("a parent")).expect("folders");
        std::fs::write(p, contents).expect("a file");
        self
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// The interpreters on PATH this machine can test with, `python3` first: (name, major, minor).
pub fn pythons() -> Vec<(String, u32, u32)> {
    let mut out = Vec::new();
    for name in ["python3", "python3.9", "python3.10", "python3.11", "python3.12", "python3.13", "python3.14"] {
        let Ok(o) = Command::new(name).args(["-c", "import sys; print(sys.version_info[0], sys.version_info[1])"]).stdin(Stdio::null()).output() else { continue };
        if !o.status.success() {
            continue;
        }
        let text = String::from_utf8_lossy(&o.stdout).to_string();
        let mut it = text.split_whitespace().filter_map(|n| n.parse::<u32>().ok());
        let (Some(major), Some(minor)) = (it.next(), it.next()) else { continue };
        if !out.iter().any(|(_, a, b): &(String, u32, u32)| (*a, *b) == (major, minor)) {
            out.push((name.to_string(), major, minor));
        }
    }
    out
}

/// What a live run left.
pub struct Live {
    pub exit: i32,
    pub stdout: String,
    pub stderr: String,
    pub report: Option<String>,
}

/// Runs `python <args>` in `cwd` with `env` (name, value) set on top of an otherwise empty
/// environment that keeps PATH; `report` is the result file to read back if the run made it.
pub fn run_python(python: &str, args: &[&str], cwd: &Path, env: &[(String, String)], report: Option<&Path>) -> Live {
    let mut cmd = Command::new(python);
    cmd.args(args).current_dir(cwd).env_clear().stdin(Stdio::null());
    if let Ok(path) = std::env::var("PATH") {
        cmd.env("PATH", path);
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    let o = cmd.output().expect("run python");
    Live {
        exit: o.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&o.stdout).to_string(),
        stderr: String::from_utf8_lossy(&o.stderr).to_string(),
        report: report.and_then(|r| std::fs::read_to_string(r).ok()),
    }
}

/// `unittest`'s timing line varies: "Ran 1 test in 0.000s" → "Ran 1 test in Ns".
pub fn without_timing(text: &str) -> String {
    text.lines()
        .map(|l| match l.find(" in ") {
            Some(at) if l.starts_with("Ran ") && l.ends_with('s') => format!("{} in Ns", &l[..at]),
            _ => l.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}
