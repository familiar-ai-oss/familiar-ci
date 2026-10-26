//! Shared by the go-test, cargo-nextest and kits tests of lane akgorust: a recorded probe run from
//! `tests/fixtures/probe_runs/<runner>/<setup>/<probe>/` (the A0 spike's recordings: `cmd`,
//! `person_cmd`, `injection.json`, `exit`, `version`, `stdout`, `stderr`), and the per-test result
//! file (JSON Lines v1) read the way the plan's `reported.rs` reads it, first match wins, so a
//! converter's output can be judged against the probe's truth. `door::reported` is the real
//! reader; this is the test's oracle, a port of the spike's `summarize.py` `read()`.

#![allow(dead_code)]

use std::path::PathBuf;

use serde_json::Value;

/// One recorded probe run.
pub struct Recorded {
    pub stdout: String,
    pub stderr: String,
    pub exit: i32,
    pub person_cmd: String,
    pub injection: Value,
}

pub fn run_dir(runner: &str, setup: &str, probe: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/probe_runs").join(runner).join(setup).join(probe)
}

pub fn recorded(runner: &str, setup: &str, probe: &str) -> Recorded {
    let d = run_dir(runner, setup, probe);
    let text = |n: &str| std::fs::read_to_string(d.join(n)).unwrap_or_else(|e| panic!("{}: {e}", d.join(n).display()));
    Recorded {
        stdout: text("stdout"),
        stderr: text("stderr"),
        exit: text("exit").trim().parse().unwrap_or_else(|_| panic!("{}: no numeric exit", d.display())),
        person_cmd: text("person_cmd").trim().to_string(),
        injection: serde_json::from_str(&text("injection.json")).expect("injection.json"),
    }
}

/// Every setup folder recorded for a runner, sorted.
pub fn setups(runner: &str) -> Vec<String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/probe_runs").join(runner);
    let mut v: Vec<String> = std::fs::read_dir(&root)
        .unwrap_or_else(|e| panic!("{}: {e}", root.display()))
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    v.sort();
    v
}

/// The probes of the A0 spike, in its order; P3b is stage 2's "the function now exists".
pub const PROBES: [&str; 8] = ["P1", "P2", "P3", "P3b", "P4", "P5", "P6", "P7"];

/// The JSON lines of a converter's output; every line must be a JSON object with a `type`.
pub fn lines(jsonl: &str) -> Vec<Value> {
    jsonl
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Value = serde_json::from_str(l).unwrap_or_else(|e| panic!("not JSON ({e}): {l}"));
            assert!(v.get("type").and_then(Value::as_str).is_some(), "no type: {l}");
            v
        })
        .collect()
}

pub fn of_type<'a>(lines: &'a [Value], ty: &str) -> Vec<&'a Value> {
    lines.iter().filter(|l| l["type"] == ty).collect()
}

/// What the reader says: a label (and for `tests_failed` the first failure's kind), or
/// `next_reader` when there is no structured output to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reading {
    pub label: String,
    pub kind: Option<String>,
}

fn reading(label: &str, kind: Option<&str>) -> Reading {
    Reading { label: label.to_string(), kind: kind.map(str::to_string) }
}

/// The plan's `reported.rs` order. No session or no `end` line: the next reader; a result that
/// contradicts the exit code: unclassified; a `missing_name` collect error: missing_target_symbol;
/// another collect error: collection_error; a setup failure: setup_error; a test's `missing_name`:
/// missing_target_symbol; a failed test: tests_failed (with its kind); a pass and no failure: passed;
/// nothing ran: no_tests_collected.
pub fn read_through_schema(jsonl: &str, exit: i32) -> Reading {
    let all = lines(jsonl);
    if all.is_empty() || of_type(&all, "session").is_empty() || of_type(&all, "end").is_empty() {
        return reading("next_reader", None);
    }
    let tests = of_type(&all, "test");
    let ces = of_type(&all, "collect_error");
    let failed: Vec<&&Value> = tests.iter().filter(|t| t["outcome"] == "failed" || t["outcome"] == "error").collect();
    let passed = tests.iter().filter(|t| t["outcome"] == "passed").count();
    let bad = !failed.is_empty() || !ces.is_empty();
    if (exit == 0 && bad) || (exit != 0 && !bad && passed > 0) {
        return reading("unclassified", None);
    }
    let kind_of = |v: &Value| v["error"]["kind"].as_str().map(str::to_string);
    if ces.iter().any(|c| kind_of(c).as_deref() == Some("missing_name")) {
        return reading("missing_target_symbol", None);
    }
    if !ces.is_empty() {
        return reading("collection_error", None);
    }
    if failed.iter().any(|t| kind_of(t).as_deref() == Some("setup")) {
        return reading("setup_error", None);
    }
    if failed.iter().any(|t| kind_of(t).as_deref() == Some("missing_name")) {
        return reading("missing_target_symbol", None);
    }
    if let Some(t) = failed.first() {
        return Reading { label: "tests_failed".into(), kind: kind_of(t) };
    }
    if passed > 0 {
        return reading("passed", None);
    }
    reading("no_tests_collected", None)
}

/// The truth of each probe (the spike's `TRUTH`): label and, for a failure, its kind.
pub fn truth(probe: &str) -> Reading {
    match probe {
        "P1" | "P3b" | "P5" => reading("passed", None),
        "P2" => reading("tests_failed", Some("assertion")),
        "P3" => reading("missing_target_symbol", None),
        "P4" => reading("collection_error", None),
        "P6" => reading("no_tests_collected", None),
        "P7" => reading("tests_failed", Some("exception")),
        other => panic!("no such probe {other}"),
    }
}

/// `right` when the reading is the probe's truth, `cant_tell` when there is nothing to read or the
/// reader says unclassified, `WRONG` for any other label: the only verdict that must never occur.
pub fn verdict(r: &Reading, probe: &str) -> &'static str {
    let t = truth(probe);
    if r.label == t.label && (t.kind.is_none() || r.kind == t.kind) {
        "right"
    } else if r.label == "next_reader" || r.label == "unclassified" {
        "cant_tell"
    } else {
        "WRONG"
    }
}
