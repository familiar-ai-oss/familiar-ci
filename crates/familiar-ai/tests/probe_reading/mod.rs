//! Lane AKjs (plan of record 2026-10-19, A): the probe recordings and the reading of a per-test
//! result file that the kit and converter tests share.
//!
//! `read` is the plan's `door/reported.rs` order (first match wins) written for tests, because
//! that module is lane A-py's: no session or no end line is "no structured output" (the next
//! reader takes over), a result that contradicts the exit code is `unclassified`, then a missing
//! name, a collect error, a setup error, a failed test, a pass, and last nothing ran. A `kit_error`
//! line anywhere is the same as no structured output: the kit lost something and says so.

#![allow(dead_code)]

use serde_json::Value;
use std::path::{Path, PathBuf};

/// Where the spike's recordings live in this repository.
pub fn probe_runs() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/probe_runs")
}

/// The reading of one run: a label (`next_reader` when no structured output) and the kind a failed
/// test carries (`""` for any other label).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reading {
    pub label: String,
    pub kind: String,
    pub why: String,
}

impl Reading {
    fn new(label: &str, kind: &str, why: &str) -> Reading {
        Reading { label: label.into(), kind: kind.into(), why: why.into() }
    }
    /// "label" or "label/kind", the spelling of the probes' truths.
    pub fn spelled(&self) -> String {
        if self.kind.is_empty() { self.label.clone() } else { format!("{}/{}", self.label, self.kind) }
    }
}

/// A verdict against a truth: the reading is the truth, "can't tell", or wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Right,
    CantTell,
    Wrong,
}

/// The truth of a probe, as `kits::<runner>::PROBES` spells it: (label, kind).
pub fn verdict(reading: &Reading, truth: (&str, &str)) -> Verdict {
    if reading.label == truth.0 && reading.kind == truth.1 {
        Verdict::Right
    } else if reading.label == "next_reader" || reading.label == "unclassified" {
        Verdict::CantTell
    } else {
        Verdict::Wrong
    }
}

fn lines_of(report: &str) -> Vec<Value> {
    report.lines().filter(|l| !l.trim().is_empty()).map(|l| serde_json::from_str::<Value>(l).unwrap_or_else(|_| serde_json::json!({"type": "garbage"}))).collect()
}

fn kind_of(v: &Value) -> &str {
    v.pointer("/error/kind").and_then(Value::as_str).unwrap_or("")
}

/// The plan's reading of a per-test result file and the run's exit code.
pub fn read(report: &str, exit: Option<i32>) -> Reading {
    let lines = lines_of(report);
    if lines.is_empty() {
        return Reading::new("next_reader", "", "no file");
    }
    let of = |t: &str| -> Vec<&Value> { lines.iter().filter(|l| l.get("type").and_then(Value::as_str) == Some(t)).collect() };
    // the kit hit an error of its own: whatever else the file says may be missing a test
    if !of("kit_error").is_empty() {
        return Reading::new("next_reader", "", "kit_error");
    }
    if of("session").is_empty() {
        return Reading::new("next_reader", "", "no session");
    }
    if of("end").is_empty() {
        return Reading::new("next_reader", "", "no end");
    }
    let tests = of("test");
    let collects = of("collect_error");
    let outcome = |t: &&Value| t.get("outcome").and_then(Value::as_str).unwrap_or("").to_string();
    let failed: Vec<&Value> = tests.iter().copied().filter(|t| matches!(outcome(t).as_str(), "failed" | "error")).collect();
    let passed = tests.iter().filter(|t| outcome(t) == "passed").count();
    let bad = !failed.is_empty() || !collects.is_empty();
    if let Some(code) = exit {
        if (code == 0 && bad) || (code != 0 && !bad && passed > 0) {
            return Reading::new("unclassified", "", &format!("contradicts exit {code}"));
        }
    }
    if collects.iter().any(|c| kind_of(c) == "missing_name") {
        return Reading::new("missing_target_symbol", "", "collect_error missing_name");
    }
    if !collects.is_empty() {
        return Reading::new("collection_error", "", "collect_error");
    }
    if failed.iter().any(|t| kind_of(t) == "setup") {
        return Reading::new("setup_error", "", "setup");
    }
    if failed.iter().any(|t| kind_of(t) == "missing_name") {
        return Reading::new("missing_target_symbol", "", "test missing_name");
    }
    if !failed.is_empty() {
        return Reading::new("tests_failed", kind_of(failed[0]), &format!("{} failed", failed.len()));
    }
    if passed > 0 {
        return Reading::new("passed", "", &format!("{passed} passed"));
    }
    Reading::new("no_tests_collected", "", "nothing ran")
}

/// One recorded run: `probe_runs/<runner>/<setup>/<probe>/`.
pub struct Recording {
    pub runner: String,
    pub setup: String,
    pub probe: String,
    pub dir: PathBuf,
}

impl Recording {
    pub fn file(&self, name: &str) -> String {
        std::fs::read_to_string(self.dir.join(name)).unwrap_or_default()
    }
    /// The kit's file of the run, empty when it wrote none.
    pub fn report(&self) -> String {
        self.file("report/results.jsonl")
    }
    pub fn exit(&self) -> Option<i32> {
        self.file("exit").trim().parse().ok()
    }
    pub fn injection(&self) -> Value {
        serde_json::from_str(&self.file("injection.json")).unwrap_or(Value::Null)
    }
}

fn subdirs(p: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(p).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    v.sort();
    v
}

fn name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Every recording of a runner, sorted by setup and probe.
pub fn recordings(runner: &str) -> Vec<Recording> {
    let mut out = Vec::new();
    for setup in subdirs(&probe_runs().join(runner)) {
        for probe in subdirs(&setup) {
            out.push(Recording { runner: runner.into(), setup: name(&setup), probe: name(&probe), dir: probe });
        }
    }
    out
}
