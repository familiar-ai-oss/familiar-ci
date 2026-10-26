//! [draft] VSTest TRX to the per-test result file: `dotnet test` with the added logger
//! (`VSTestLogger=trx%3BLogFileName=<tmp>/results.trx` and `VSTestResultsDirectory`, the `;`
//! escaped as `%3B`; or the added `--logger "trx;LogFileName=…"` flag when the command has its
//! own `--logger`, which beats the env property). It ports the A0 spike's
//! `kit/dotnet/trx_to_schema.py`, with what the spike learned and what honesty adds:
//!
//! - A build error (CS0117, CS0246) writes no TRX: this reader never gives
//!   `missing_target_symbol` or `collection_error`; no TRX is no structured output.
//! - A TRX holds the message and the stack trace but no exception type, so the kind is read from
//!   the words. An assertion needs positive evidence (xunit's and MSTest's `Assert.X`, NUnit's
//!   `Expected:`); `Type : message` (xunit) and MSTest's `threw exception:` name an exception;
//!   any other failure is an exception, which at the planned test's own line asks instead of
//!   sealing. This reader never gives `missing_name`: a member the test calls and lacks is a
//!   build error (no TRX), and a `MissingMethodException` at run time is a binary mismatch
//!   (stale assemblies), not a name the test asks for. (The spike called
//!   every failure that was not `Type : message` an assertion: proven for xunit only.)
//! - Only `Passed` is a pass, `Failed` a failure and `NotExecuted` a skip. Any other outcome
//!   (Timeout, Aborted, Inconclusive, NotRunnable, Error, ...) is an `error` with no place: a
//!   test that did not finish is never a pass.
//! - A data-driven test's inner results are read, not the parent that sums them.
//! - A report older than the run's start is ignored; a TRX that cannot be read whole leaves the
//!   run unread (no session, no end).

use super::schema::{self, Context, Counts, Failure, Frame, Native};
use super::xml::{self, Element};
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::SystemTime;

macro_rules! pattern {
    ($name:ident, $re:expr) => {
        fn $name() -> &'static Regex {
            static CELL: OnceLock<Regex> = OnceLock::new();
            CELL.get_or_init(|| Regex::new($re).expect("a fixed pattern"))
        }
    };
}

// xunit writes an exception as "<Type> : <message>"; anywhere at a line's start, for MSTest's "threw exception:\n<Type>: <message>"
pattern!(exception, r"(?s)(?:^|\n)\s*([A-Za-z_][\w.]*(?:Exception|Error))\s*:\s*(.*)");
pattern!(frame, r"^\s*at (.+?) in (.+):line (\d+)\s*$");

/// One TRX document: no staleness to judge, no root to strip.
pub fn to_report(native: &str, exit: i32) -> String {
    let files = [Native { name: "results.trx", text: native, modified: SystemTime::UNIX_EPOCH }];
    to_report_files(&files, &Context::new("dotnet-vstest", "csharp"), None, exit)
}

/// The TRX files a run wrote as the per-test result file; `exit` is the process's. With
/// `run_started`, a file last modified before it is ignored. Empty when there was nothing to
/// read, `kit_error` lines alone when something fresh could not be read.
pub fn to_report_files(files: &[Native<'_>], ctx: &Context<'_>, run_started: Option<SystemTime>, exit: i32) -> String {
    let candidates = schema::fresh(files, run_started);
    let mut tests: Vec<Value> = Vec::new();
    let mut counts = Counts::default();
    let mut problems: Vec<Value> = Vec::new();
    let mut seen = 0usize;
    for f in candidates {
        let root = match xml::parse(f.text) {
            Ok(root) => root,
            Err(e) => {
                problems.push(schema::kit_error(&format!("{}: {e}", f.name)));
                continue;
            }
        };
        if root.name != "TestRun" {
            continue;
        }
        seen += 1;
        // the class of each test definition, by its id
        let classes: HashMap<&str, &str> = root
            .descendants("UnitTest")
            .into_iter()
            .filter_map(|ut| Some((ut.attr("id")?, ut.child("TestMethod")?.attr("className")?)))
            .collect();
        let bad_before = counts.failed + counts.errors;
        let mut these: Vec<Value> = Vec::new();
        for result in root.descendants("UnitTestResult") {
            if !result.descendants("UnitTestResult").is_empty() {
                continue; // a parent of inner results: the inner ones are the tests
            }
            these.push(test_of(result, &classes, ctx, &mut counts));
        }
        if let Some(why) = hides_failures(&root, counts.failed + counts.errors - bad_before) {
            problems.push(schema::kit_error(&format!("{}: {why}", f.name)));
            continue;
        }
        tests.extend(these);
    }
    if seen == 0 || !problems.is_empty() {
        return schema::jsonl(&problems);
    }
    let mut lines = vec![schema::session(ctx, "trx", seen)];
    lines.extend(tests);
    lines.push(schema::end(exit, &counts));
    schema::jsonl(&lines)
}

/// The run's own counters say how many results failed, errored, timed out or aborted: more than
/// the results list is an incomplete picture (no counters, or fewer, is read as it lists).
fn hides_failures(root: &Element, listed: u64) -> Option<String> {
    let counters = root.child("ResultSummary")?.child("Counters")?;
    let declared: u64 = ["failed", "error", "timeout", "aborted"].iter().filter_map(|a| counters.attr(a)).filter_map(|v| v.trim().parse::<u64>().ok()).sum();
    (declared > listed).then(|| format!("the counters count {declared} failed, error, timeout or aborted results and the results list {listed}"))
}

fn test_of(res: &Element, classes: &HashMap<&str, &str>, ctx: &Context<'_>, counts: &mut Counts) -> Value {
    let name = res.attr("testName").unwrap_or("");
    let class = res.attr("testId").and_then(|id| classes.get(id)).copied().unwrap_or("");
    let id = if class.is_empty() || name.starts_with(class) { name.to_string() } else { format!("{class}.{name}") };
    let info = res.child("Output").and_then(|o| o.child("ErrorInfo"));
    let message = info.and_then(|i| i.child("Message")).map(|m| m.text.as_str()).unwrap_or("");
    let stack = info.and_then(|i| i.child("StackTrace")).map(|m| m.text.as_str()).unwrap_or("");
    match res.attr("outcome").unwrap_or("") {
        "Passed" => {
            counts.passed += 1;
            schema::test_line(&id, None, None, "passed", None, None)
        }
        "Failed" => {
            counts.failed += 1;
            let failure = failure_of(message, stack, ctx.root);
            // the test's own place is its outermost project frame (the spike's `fr[-1]` of a deepest-first trace)
            let place = failure.frames.first().cloned();
            schema::test_line(&id, place.as_ref().map(|f| f.file.as_str()), place.as_ref().map(|f| f.line), "failed", None, Some(&failure))
        }
        "NotExecuted" => {
            counts.skipped += 1;
            let stdout = res.child("Output").and_then(|o| o.child("StdOut")).map(|s| s.text.trim()).unwrap_or("");
            let reason = Some(message.trim()).filter(|m| !m.is_empty()).or(Some(stdout).filter(|s| !s.is_empty()));
            schema::test_line(&id, None, None, "skipped", reason, None)
        }
        other => {
            // Timeout, Aborted, Inconclusive, NotRunnable, Error, ...: it did not pass, and nothing says where
            counts.errors += 1;
            let text = if message.trim().is_empty() { format!("outcome {other}") } else { format!("outcome {other}: {}", message.trim()) };
            let failure = Failure { kind: "exception", error_type: String::new(), message: text, frames: Vec::new() };
            schema::test_line(&id, None, None, "error", None, Some(&failure))
        }
    }
}

fn failure_of(message: &str, stack: &str, root: Option<&str>) -> Failure {
    let text = message.trim();
    let frames = frames_of(stack, root);
    // xunit and MSTest open an assertion's message with Assert.<Name>, NUnit with its Expected: line
    let assertion = text.starts_with("Assert.") || text.starts_with("Expected:");
    if assertion {
        return Failure { kind: "assertion", error_type: "assertion".to_string(), message: text.to_string(), frames };
    }
    match exception().captures(text) {
        Some(m) => {
            // a MissingMethodException at run time is a binary mismatch (stale assemblies): the
            // name a test calls and lacks is a build error, which writes no TRX; never `missing_name`
            Failure { kind: "exception", error_type: m[1].to_string(), message: m[2].trim().to_string(), frames }
        }
        None => Failure { kind: "exception", error_type: String::new(), message: text.to_string(), frames },
    }
}

/// The frames of a .NET stack trace that name a file and line, outermost first and the deepest
/// last (traces list the deepest first).
fn frames_of(stack: &str, root: Option<&str>) -> Vec<Frame> {
    let mut out: Vec<Frame> = stack
        .lines()
        .filter_map(|l| {
            let m = frame().captures(l)?;
            Some(Frame { file: schema::relative(&m[2], root), line: m[3].parse().ok()?, function: Some(m[1].to_string()) })
        })
        .collect();
    out.reverse();
    out
}
