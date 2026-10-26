//! The per-test result file, JSON Lines v1 (the A0 spike's `kit/SCHEMA.txt`), as the converters
//! write it: a `session` line, one `test` line per test, an `end` line (`exit`, `counts`), and
//! `kit_error` lines for what could not be read. Lane A-py's `door/reported.rs` reads it.
//!
//! The rules every converter keeps:
//! - No session and no end line means "no structured output": a converter that read nothing, or
//!   could not read all of what it was given, writes only `kit_error` lines (or nothing).
//! - An `end` line is written only for a run that was read to its end; counts never claim
//!   "nothing ran" for a report that was not there.
//! - A report older than the run's start is not evidence ([`fresh`]).
//! - A converter never prints and never panics; what it cannot read it says in a `kit_error`.

use serde_json::{json, Value};
use std::time::SystemTime;

/// One native report (a JUnit XML, a TRX) as the run left it: its name, its text and when it
/// was last written. `modified` is what [`fresh`] judges.
#[derive(Debug, Clone, Copy)]
pub struct Native<'a> {
    pub name: &'a str,
    pub text: &'a str,
    pub modified: SystemTime,
}

/// What the converters know about the run besides the report: the session line's runner and
/// language, the runner's version when known, and the project root the paths become relative to.
#[derive(Debug, Clone, Copy)]
pub struct Context<'a> {
    pub runner: &'a str,
    pub runner_version: Option<&'a str>,
    pub language: &'a str,
    pub root: Option<&'a str>,
}

impl<'a> Context<'a> {
    pub fn new(runner: &'a str, language: &'a str) -> Self {
        Context { runner, runner_version: None, language, root: None }
    }

    pub fn with_root(mut self, root: &'a str) -> Self {
        self.root = Some(root);
        self
    }

    pub fn with_version(mut self, version: &'a str) -> Self {
        self.runner_version = Some(version);
        self
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    pub passed: u64,
    pub failed: u64,
    pub skipped: u64,
    pub deselected: u64,
    pub errors: u64,
}

/// One frame of a failure's trace: the file as the report names it (relative to the root when it
/// lies under it), the line, the function when the trace has one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub file: String,
    pub line: u64,
    pub function: Option<String>,
}

/// Why a test failed: the exception or assertion type, the message, the kind (`assertion`,
/// `exception`, `missing_name`, `setup`) and the frames, outermost first and the deepest last.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub kind: &'static str,
    pub error_type: String,
    pub message: String,
    pub frames: Vec<Frame>,
}

pub fn session(ctx: &Context<'_>, converted_from: &str, files: usize) -> Value {
    json!({
        "type": "session", "v": 1, "runner": ctx.runner, "runner_version": ctx.runner_version, "language": ctx.language,
        "plugins": [], "converted_from": converted_from, "files": files,
    })
}

pub fn test_line(id: &str, file: Option<&str>, line: Option<u64>, outcome: &str, skip_reason: Option<&str>, failure: Option<&Failure>) -> Value {
    let error = failure.map(|f| {
        json!({
            "type": f.error_type, "message": f.message, "kind": f.kind,
            "frames": f.frames.iter().map(|x| json!({"file": x.file, "line": x.line, "function": x.function})).collect::<Vec<_>>(),
        })
    });
    json!({"type": "test", "id": id, "file": file, "line": line, "outcome": outcome, "skip_reason": skip_reason, "error": error})
}

pub fn end(exit: i32, counts: &Counts) -> Value {
    json!({
        "type": "end", "exit": exit,
        "counts": {"passed": counts.passed, "failed": counts.failed, "skipped": counts.skipped, "deselected": counts.deselected, "errors": counts.errors},
    })
}

pub fn kit_error(message: &str) -> Value {
    json!({"type": "kit_error", "message": message})
}

/// The lines as JSON Lines: one compact object per line, each ended by a newline.
pub fn jsonl(lines: &[Value]) -> String {
    lines.iter().map(|l| format!("{l}\n")).collect()
}

/// The reports a run wrote: with `run_started`, those last modified at or after it (a compile
/// error leaves the last run's reports behind, and read as this run's they are a wrong reading);
/// without it, all. On a filesystem with coarse mtimes the caller passes the start rounded down.
pub fn fresh<'a, 'b>(files: &'b [Native<'a>], run_started: Option<SystemTime>) -> Vec<&'b Native<'a>> {
    files.iter().filter(|f| run_started.is_none_or(|start| f.modified >= start)).collect()
}

/// `path` relative to `root` when it lies under it, else as written.
pub fn relative(path: &str, root: Option<&str>) -> String {
    match root {
        Some(root) if !root.is_empty() => {
            let prefix = format!("{}/", root.trim_end_matches('/'));
            path.strip_prefix(&prefix).unwrap_or(path).to_string()
        }
        _ => path.to_string(),
    }
}
