//! Lane AKreports (plan of record 2026-10-19, A section): reading the A0 spike's probe recordings
//! (`tests/fixtures/probe_runs/<runner>/<setup>/<probe>/`) through the per-test result file
//! (the spike's `kit/SCHEMA.txt`, JSON Lines v1), the way the plan's `reported.rs` reads it: the
//! first match wins; no session or no `end` line is no structured output (the next reader); a
//! result that contradicts the exit code is `unclassified`.
//!
//! This reader is the test's own copy of the spike's `bin/summarize.py` `read()`; lane A-py owns
//! `door/reported.rs` and, once it lands, its reader replaces `read_schema` here. A verdict is
//! `Right` (the probe's truth), `CantTell` (the next reader, or `unclassified`) or `Wrong`
//! (anything else): the honesty rule is that no recording ever reads `Wrong`.

#![allow(dead_code)]

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The probes of the dry run: stage 1 P1-P6, stage 2 P3b (the function now exists) and P7.
pub const PROBES: [&str; 8] = ["P1", "P2", "P3", "P3b", "P4", "P5", "P6", "P7"];

/// What each probe truly is (`kit/SCHEMA.txt`): the label and, for a failure, its kind.
pub fn truth(probe: &str) -> (&'static str, Option<&'static str>) {
    match probe {
        "P1" => ("passed", None),
        "P2" => ("tests_failed", Some("assertion")),
        "P3" => ("missing_target_symbol", None),
        "P4" => ("collection_error", None),
        "P5" => ("passed", None),
        "P6" => ("no_tests_collected", None),
        "P3b" => ("passed", None),
        "P7" => ("tests_failed", Some("exception")),
        other => panic!("there is no probe {other}"),
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Reading {
    /// No session or no end line: this reader saw no structured output.
    NextReader(String),
    /// A label, and the kind of the first failed test.
    Read { label: String, kind: Option<String> },
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Verdict {
    Right,
    CantTell,
    Wrong,
}

fn lines_of(jsonl: &str) -> Vec<Value> {
    jsonl
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap_or_else(|_| serde_json::json!({"type": "garbage"})))
        .collect()
}

fn kind_of(t: &Value) -> Option<String> {
    t["error"]["kind"].as_str().map(str::to_string)
}

/// The plan's `reported.rs` order, over one result file and the process's exit code.
pub fn read_schema(jsonl: &str, exit: i32) -> Reading {
    let lines = lines_of(jsonl);
    if lines.is_empty() {
        return Reading::NextReader("no file".into());
    }
    let of = |t: &str| lines.iter().filter(|l| l["type"] == t).collect::<Vec<_>>();
    if of("session").is_empty() {
        return Reading::NextReader("no session".into());
    }
    if of("end").is_empty() {
        return Reading::NextReader("no end".into());
    }
    let tests = of("test");
    let collect_errors = of("collect_error");
    let failed: Vec<&&Value> = tests.iter().filter(|t| matches!(t["outcome"].as_str(), Some("failed" | "error"))).collect();
    let passed = tests.iter().filter(|t| t["outcome"] == "passed").count();
    let bad = !failed.is_empty() || !collect_errors.is_empty();
    let say = |label: &str, kind: Option<String>| Reading::Read { label: label.into(), kind };
    if (exit == 0 && bad) || (exit != 0 && !bad && passed > 0) {
        return say("unclassified", None);
    }
    if collect_errors.iter().any(|c| c["error"]["kind"] == "missing_name") {
        return say("missing_target_symbol", None);
    }
    if !collect_errors.is_empty() {
        return say("collection_error", None);
    }
    if failed.iter().any(|t| t["error"]["kind"] == "setup") {
        return say("setup_error", None);
    }
    if failed.iter().any(|t| t["error"]["kind"] == "missing_name") {
        return say("missing_target_symbol", None);
    }
    if let Some(first) = failed.first() {
        return say("tests_failed", kind_of(first));
    }
    if passed > 0 {
        return say("passed", None);
    }
    say("no_tests_collected", None)
}

/// Right when it is the probe's truth, can't-tell when it asks, wrong otherwise.
pub fn verdict(reading: &Reading, probe: &str) -> Verdict {
    let (label, kind) = truth(probe);
    match reading {
        Reading::NextReader(_) => Verdict::CantTell,
        Reading::Read { label: l, .. } if l == "unclassified" => Verdict::CantTell,
        Reading::Read { label: l, kind: k } if l == label && (kind.is_none() || k.as_deref() == kind) => Verdict::Right,
        Reading::Read { .. } => Verdict::Wrong,
    }
}

pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/probe_runs")
}

/// One recording: `<runner>/<setup>/<probe>/`.
pub fn recording(runner: &str, setup: &str, probe: &str) -> PathBuf {
    fixtures().join(runner).join(setup).join(probe)
}

/// The process's exit code as the run recorded it.
pub fn exit_of(dir: &Path) -> i32 {
    std::fs::read_to_string(dir.join("exit")).expect("a recorded exit").trim().parse().expect("an exit code")
}

/// Every file under `dir` for which `keep` says yes: (name relative to `dir`, text), sorted by name.
pub fn files_under(dir: &Path, keep: &dyn Fn(&str) -> bool) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let name = p.strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/");
                if keep(&name) {
                    let text = String::from_utf8_lossy(&std::fs::read(&p).expect("a recorded file")).to_string();
                    out.push((name, text));
                }
            }
        }
    }
    out.sort();
    out
}

/// The run's start in these tests, and the moments around it.
pub fn run_started() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_800_000_000)
}
pub fn after_start() -> SystemTime {
    run_started() + Duration::from_secs(5)
}
pub fn before_start() -> SystemTime {
    run_started() - Duration::from_secs(3_600)
}

fn basename(p: &str) -> &str {
    p.rsplit('/').next().unwrap_or(p)
}

/// What a result file says, with the file's directories and the runner's own timings left out
/// (the spike's `summarize.py` `norm`): one string per line, sorted.
pub fn norm(jsonl: &str) -> Vec<String> {
    let mut out: Vec<String> = lines_of(jsonl)
        .iter()
        .map(|l| match l["type"].as_str().unwrap_or("?") {
            "session" => "session".to_string(),
            "test" => {
                let last = l["error"]["frames"].as_array().and_then(|f| f.last()).map(|f| format!("{}:{}", basename(f["file"].as_str().unwrap_or("")), f["line"]));
                format!(
                    "test|{}|{}|{}|{}|{}",
                    l["id"].as_str().unwrap_or(""),
                    l["outcome"].as_str().unwrap_or(""),
                    l["error"]["kind"].as_str().unwrap_or(""),
                    l["error"]["type"].as_str().unwrap_or(""),
                    last.unwrap_or_default()
                )
            }
            "collect_error" => format!("collect_error|{}|{}|{}", basename(l["file"].as_str().unwrap_or("")), l["error"]["kind"].as_str().unwrap_or(""), l["error"]["type"].as_str().unwrap_or("")),
            "end" => {
                let mut counts: Vec<String> = l["counts"].as_object().map(|c| c.iter().filter(|(_, v)| v.as_u64().unwrap_or(0) > 0).map(|(k, v)| format!("{k}={v}")).collect()).unwrap_or_default();
                counts.sort();
                format!("end|{}", counts.join(","))
            }
            other => other.to_string(),
        })
        .collect();
    out.sort();
    out
}

/// The `file` attribute's folder above `/tests/` in a PHPUnit log: the project root the run saw.
pub fn php_root(junit: &str) -> Option<String> {
    let at = junit.find("file=\"")? + 6;
    let rest = &junit[at..];
    let end = rest.find("/tests/")?;
    Some(rest[..end].to_string())
}
