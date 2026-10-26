//! [draft] JUnit XML to the per-test result file: Maven surefire and failsafe
//! (`target/{surefire,failsafe}-reports/TEST-*.xml`), Gradle (`build/test-results/**/*.xml`),
//! PHPUnit/Pest `--log-junit`, and the C/C++ runners' own JUnit-shaped files (googletest's
//! `GTEST_OUTPUT=xml:`, Catch2 v3's `XML_OUTPUT_FILE` under `BAZEL_TEST=1`, `ctest --output-junit`).
//! It ports the A0 spike's `kit/junit/junit_to_schema.py`, with what the spike learned:
//!
//! - The kind comes from the exception type, never from `<failure>` versus `<error>` (Gradle
//!   writes every exception as `<failure>`): an assertion type is an assertion; a missing name
//!   (PHP's "Call to undefined method", the one run-time shape of it: PHP has no compile step) is
//!   `missing_name`; the rest, a Java linkage error included, is an exception. A failure naming
//!   no type is an assertion only on positive evidence (the message opens with a `file:line`
//!   header, as googletest writes it), else an exception, which at the planned test's own line
//!   asks instead of sealing.
//! - Reports older than the run's start are ignored (`run_started`): a compile error leaves the
//!   last run's XML behind and read as this run's it reads a wrong reading.
//! - Nothing readable, or any fresh report unreadable (empty, cut, not XML), gives no session
//!   and no end line: the next reader decides. A report set read in part is not read.
//! - A report reader never gives `missing_target_symbol` or `collection_error` on its own on the
//!   JVM or .NET: a compile error writes no report. Those stay with the tables.
//!
//! The googletest, Catch2 and ctest shapes are written from their documentation, not from a
//! recording: no probe kit calibrates them (see `kits::gtest`).

use super::schema::{self, Context, Counts, Failure, Frame, Native};
use super::xml::{self, Element};
use regex::Regex;
use serde_json::Value;
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

// the name a test asked for does not exist: PHP's Error messages (PHP has no compile step, so this
// is a run-time error). Java's NoSuchMethodError and NoClassDefFoundError are not here: a missing
// name the test calls is a compile error that writes no report, and at run time those are stale
// classes or a broken classpath (NoClassDefFoundError "Could not initialize class" even follows an
// exception in a static initializer). Read as a missing name they would seal a red that is not one.
pattern!(missing, r#"(?:^|: )(?:Call to undefined (?:method|function)|Class "[^"]+" not found|Undefined constant)"#);
// the runner's assertion types, by the class's own name (never a longer name that merely contains one)
pattern!(assertion_type, r"(?:^|[.$\\/:])(?:AssertionError|AssertionFailedError|ExpectationFailedException|ComparisonFailure|MultipleFailuresError|AssertionFailure)$");
// Catch2's JUnit reporter names the macro that failed
pattern!(catch2_macro, r"^(?:REQUIRE|CHECK|ASSERT|EXPECT)(?:_[A-Z_]+)?$");
pattern!(java_frame, r"^\s*at\s+([\w$.<>]+)\.([\w$<>]+)\(([^:()]+):(\d+)\)");
pattern!(path_frame, r"^(/[^\s:]+|[\w./-]+\.(?:php|py|js|ts|cs|kt|java|c|cc|cpp|cxx|h|hh|hpp)):(\d+)$");
// Catch2's failure text ends with "at /path/file.cpp:42"
pattern!(at_frame, r"^at\s+(.+?):(\d+)$");

/// A Java frame in the JDK, the test framework or the build tool is not the project's.
const NOISE: [&str; 10] = ["java.", "jdk.", "sun.", "org.junit.", "org.opentest4j.", "org.apache.maven.", "org.gradle.", "worker.org.gradle.", "com.sun.", "kotlin."];

/// One report document, as the spike's CLI took one file: no staleness to judge, no root to strip.
pub fn to_report(native: &str, exit: i32) -> String {
    let files = [Native { name: "report.xml", text: native, modified: SystemTime::UNIX_EPOCH }];
    to_report_files(&files, &Context::new("junit", ""), None, exit)
}

/// The reports a run wrote as the per-test result file; `exit` is the process's. With
/// `run_started`, a report last modified before it is ignored. Empty when there was nothing to
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
        if root.name != "testsuite" && root.name != "testsuites" {
            continue;
        }
        if let Some(why) = hides_failures(&root) {
            problems.push(schema::kit_error(&format!("{}: {why}", f.name)));
            continue;
        }
        seen += 1;
        for testcase in root.descendants("testcase") {
            tests.push(test_of(testcase, ctx, &mut counts));
        }
    }
    if seen == 0 || !problems.is_empty() {
        return schema::jsonl(&problems);
    }
    let mut lines = vec![schema::session(ctx, "junit", seen)];
    lines.extend(tests);
    lines.push(schema::end(exit, &counts));
    schema::jsonl(&lines)
}

/// A leaf suite that declares more failures and errors than its testcases show (a failed class
/// setup or an engine error with no test to carry it) is an incomplete picture: the green of what
/// it does show would be a wrong reading. A suite is counted by the failure and error elements it
/// shows (Catch2 writes one per failed assertion), a parent suite sums its children and is not
/// counted, and a writer that declares nothing, or fewer than it shows, is read as it shows.
fn hides_failures(root: &Element) -> Option<String> {
    let mut suites: Vec<&Element> = root.descendants("testsuite");
    if root.name == "testsuite" {
        suites.insert(0, root);
    }
    for suite in suites {
        if suite.children.iter().any(|c| c.name == "testsuite") {
            continue;
        }
        let declared: u64 = ["failures", "errors"].iter().filter_map(|a| suite.attr(a)).filter_map(|v| v.trim().parse::<u64>().ok()).sum();
        let shown: u64 = suite.children_named("testcase").map(|tc| (tc.children_named("failure").count() + tc.children_named("error").count()) as u64).sum();
        if declared > shown {
            return Some(format!("the suite {} declares {declared} failures or errors and shows {shown}", suite.attr("name").unwrap_or("")));
        }
    }
    None
}

fn test_of(tc: &Element, ctx: &Context<'_>, counts: &mut Counts) -> Value {
    let class = tc.attr("classname").or_else(|| tc.attr("class")).unwrap_or("");
    let name = tc.attr("name").unwrap_or("");
    let id = if class.is_empty() { name.to_string() } else { format!("{class}::{name}") };
    let file = tc.attr("file").map(|f| schema::relative(f, ctx.root));
    let line = tc.attr("line").and_then(|l| l.parse::<u64>().ok());
    let (fail, err, skip) = (tc.child("failure"), tc.child("error"), tc.child("skipped"));
    if let Some(el) = fail.or(err) {
        let failure = failure_of(el, fail.is_some(), ctx.root);
        let outcome = if fail.is_some() {
            counts.failed += 1;
            "failed"
        } else {
            counts.errors += 1;
            "error"
        };
        return schema::test_line(&id, file.as_deref(), line, outcome, None, Some(&failure));
    }
    // googletest writes a disabled test as status="notrun" result="suppressed"; ctest a disabled one as status="notrun"
    let not_run = skip.is_some() || tc.attr("status") == Some("notrun") || matches!(tc.attr("result"), Some("skipped" | "suppressed"));
    if not_run {
        counts.skipped += 1;
        let reason = skip.and_then(|s| s.attr("message").filter(|m| !m.is_empty()).map(str::to_string).or_else(|| Some(s.text.trim().to_string()).filter(|t| !t.is_empty())));
        return schema::test_line(&id, file.as_deref(), line, "skipped", reason.as_deref(), None);
    }
    counts.passed += 1;
    schema::test_line(&id, file.as_deref(), line, "passed", None, None)
}

fn failure_of(el: &Element, is_failure: bool, root: Option<&str>) -> Failure {
    let error_type = el.attr("type").unwrap_or("").to_string();
    let text = el.text.as_str();
    let mut message = el.attr("message").unwrap_or("").to_string();
    if message.is_empty() {
        // PHPUnit's log has no message attribute: "Class::test", the message, a blank line, the trace
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        message = lines.get(1).or(lines.first()).map(|l| l.trim_end().to_string()).unwrap_or_default();
    }
    let mut frames = frames_of(text, root);
    if frames.is_empty() {
        frames = frames_of(&message, root);
    }
    let kind = if assertion_type().is_match(&error_type) || catch2_macro().is_match(&error_type) {
        "assertion"
    } else if missing().is_match(&message) || missing().is_match(&error_type) {
        "missing_name"
    } else if error_type.is_empty() && is_failure && opens_with_a_place(&message, text) {
        "assertion"
    } else {
        "exception"
    };
    Failure { kind, error_type, message, frames }
}

/// googletest opens a failure with the assertion's own `file:line` line.
fn opens_with_a_place(message: &str, text: &str) -> bool {
    let first = |s: &str| s.lines().map(str::trim).find(|l| !l.is_empty()).map(str::to_string);
    first(message).or_else(|| first(text)).is_some_and(|l| path_frame().is_match(&l))
}

/// The project's frames of a trace, outermost first and the deepest last (traces list the
/// deepest first).
fn frames_of(text: &str, root: Option<&str>) -> Vec<Frame> {
    let mut out = Vec::new();
    for l in text.lines() {
        if let Some(m) = java_frame().captures(l) {
            let class = &m[1];
            if !NOISE.iter().any(|n| class.starts_with(n)) {
                if let Ok(line) = m[4].parse::<u64>() {
                    out.push(Frame { file: m[3].to_string(), line, function: Some(format!("{class}.{}", &m[2])) });
                }
            }
            continue;
        }
        let t = l.trim();
        if let Some(m) = path_frame().captures(t).or_else(|| at_frame().captures(t)) {
            if !m[1].contains("/vendor/") {
                if let Ok(line) = m[2].parse::<u64>() {
                    out.push(Frame { file: schema::relative(&m[1], root), line, function: None });
                }
            }
        }
    }
    out.reverse();
    out
}
