//! Lane AKreports (plan of record 2026-10-19, A section: the report-file readers). A VSTest TRX
//! report (`dotnet test` with `VSTestLogger=trx%3BLogFileName=…`, or the added `--logger` flag
//! when the command has its own) becomes the per-test result file and reads its truth through it.
//!
//! ```text
//!   results.trx ──▶ trx::to_report_files ──▶ session · test… · end ──▶ the reader
//!        │                  │
//!        │                  └─ older than run_started? ignored
//!        └─ none (a build error writes none; the person's own --logger beat the env)? no session
//! ```
//!
//! The recordings are the A0 spike's: dotnet SDK 8.0.426, VSTest 17.11.1, xunit 2.9.2. A build
//! error (P3 CS0117, P4 CS0246) writes no TRX, so those two probes read can't-tell here and stay
//! with the tables; P6 (no test in the assembly) writes a TRX with no results and reads "nothing
//! ran". A TRX carries a message and a stack trace but no exception type, so the kind is read
//! from the message's words: an assertion only on positive evidence (xunit's and MSTest's
//! `Assert.X`, NUnit's `Expected:`), an exception where the message says `Type : message`, and
//! anything else an exception (which at the planned test's own line asks instead of sealing).

mod probe_runs_support;

use familiar_ai::door::convert::schema::{Context, Native};
use familiar_ai::door::convert::trx;
use probe_runs_support::*;
use serde_json::Value;
use std::time::SystemTime;

const DOTNET: [(&str, Verdict); 8] = [
    ("P1", Verdict::Right),
    ("P2", Verdict::Right),
    ("P3", Verdict::CantTell),
    ("P3b", Verdict::Right),
    ("P4", Verdict::CantTell),
    ("P5", Verdict::Right),
    ("P6", Verdict::Right),
    ("P7", Verdict::Right),
];

/// Setups whose TRX was written: the env logger, a German console (TRX is locale-proof), the
/// added flag, and the added flag beside the person's own `--logger`.
const WRITTEN: [&str; 4] = ["plain", "de", "plain-flag", "ownlogger-flag"];

fn lines(out: &str) -> Vec<Value> {
    out.lines().map(|l| serde_json::from_str(l).expect("every line is JSON")).collect()
}

fn natives(files: &[(String, String)], modified: SystemTime) -> Vec<Native<'_>> {
    files.iter().map(|(n, t)| Native { name: n, text: t, modified }).collect()
}

fn trx_files(dir: &std::path::Path) -> Vec<(String, String)> {
    files_under(&dir.join("report"), &|n| n.ends_with(".trx"))
}

/// The project root the recorded run saw: the folder above `src/Calc.cs` in a stack frame.
fn dotnet_root(trx: &str) -> Option<String> {
    let at = trx.find(" in /")? + 4;
    let rest = &trx[at..];
    let end = rest.find("/src/Calc.cs").or_else(|| rest.find("/tests/ProbeTests.cs"))?;
    Some(rest[..end].to_string())
}

fn convert(files: &[(String, String)], exit: i32) -> String {
    let root = files.iter().find_map(|(_, t)| dotnet_root(t));
    let ctx = Context::new("dotnet-vstest", "csharp");
    let ctx = match &root {
        Some(r) => ctx.with_root(r),
        None => ctx,
    };
    trx::to_report_files(&natives(files, after_start()), &ctx, Some(run_started()), exit)
}

#[test]
fn dotnet_xunit_trx_reports_read_their_truths() {
    for setup in WRITTEN {
        for (probe, expected) in DOTNET {
            let dir = recording("dotnet-xunit", setup, probe);
            let out = convert(&trx_files(&dir), exit_of(&dir));
            let reading = read_schema(&out, exit_of(&dir));
            assert_eq!(verdict(&reading, probe), expected, "dotnet {setup} {probe}: read {reading:?}");
        }
    }
}

/// The person's own `--logger` beats the env property (a command-line global property beats an
/// env one): the env-only reader reads nothing there, and the reader asks.
#[test]
fn a_command_with_its_own_logger_loses_the_env_property_and_asks() {
    for probe in PROBES {
        let dir = recording("dotnet-xunit", "ownlogger", probe);
        let files = trx_files(&dir);
        assert!(files.is_empty(), "the env logger wrote no TRX: {probe}");
        let out = convert(&files, exit_of(&dir));
        assert_eq!(verdict(&read_schema(&out, exit_of(&dir)), probe), Verdict::CantTell, "{probe}");
    }
}

#[test]
fn no_recording_reads_wrong() {
    let mut read = 0;
    for setup in ["plain", "de", "plain-flag", "ownlogger", "ownlogger-flag"] {
        for probe in PROBES {
            let dir = recording("dotnet-xunit", setup, probe);
            let out = convert(&trx_files(&dir), exit_of(&dir));
            assert_ne!(verdict(&read_schema(&out, exit_of(&dir)), probe), Verdict::Wrong, "dotnet {setup} {probe} reads wrong");
            read += 1;
        }
    }
    assert_eq!(read, 5 * 8);
}

#[test]
fn the_converter_says_what_the_spike_converter_said() {
    let mut compared = 0;
    for setup in WRITTEN {
        for probe in PROBES {
            let dir = recording("dotnet-xunit", setup, probe);
            let Ok(spike) = std::fs::read_to_string(dir.join("report/converted.jsonl")) else { continue };
            let ours = convert(&trx_files(&dir), exit_of(&dir));
            assert_eq!(norm(&ours), norm(&spike), "dotnet {setup} {probe}");
            compared += 1;
        }
    }
    // six probes wrote a TRX in each of the four setups
    assert_eq!(compared, 4 * 6);
}

#[test]
fn a_trx_older_than_the_run_is_ignored() {
    let dir = recording("dotnet-xunit", "plain", "P2");
    let files = trx_files(&dir);
    let ctx = Context::new("dotnet-vstest", "csharp");
    let stale = trx::to_report_files(&natives(&files, before_start()), &ctx, Some(run_started()), 1);
    assert!(lines(&stale).iter().all(|l| l["type"] != "session"), "{stale}");
    let blind = trx::to_report_files(&natives(&files, before_start()), &ctx, None, 1);
    assert!(blind.contains("\"session\""), "with no start to judge by the file is read");
    let fresh = trx::to_report_files(&natives(&files, run_started()), &ctx, Some(run_started()), 1);
    assert!(fresh.contains("\"session\""), "written at the start is fresh");
}

#[test]
fn the_frames_are_the_projects_with_the_deepest_last_and_paths_relative() {
    let dir = recording("dotnet-xunit", "plain", "P7");
    let out = convert(&trx_files(&dir), 1);
    let t = lines(&out).into_iter().find(|l| l["type"] == "test").unwrap();
    let frames: Vec<(String, u64)> = t["error"]["frames"].as_array().unwrap().iter().map(|f| (f["file"].as_str().unwrap().to_string(), f["line"].as_u64().unwrap())).collect();
    // the test calls the code: the test's frame first, the code's (where it threw) last
    assert_eq!(frames, vec![("tests/ProbeTests.cs".to_string(), 11), ("src/Calc.cs".to_string(), 9)]);
    assert_eq!(t["error"]["type"], "System.FormatException");
    assert_eq!(t["error"]["kind"], "exception");
    assert_eq!(t["id"], "Tests.ProbeTests.Parses");
    assert_eq!((t["file"].as_str(), t["line"].as_u64()), (Some("tests/ProbeTests.cs"), Some(11)), "the failing test's own place is its outermost project frame");
}

const NS: &str = "http://microsoft.com/schemas/VisualStudio/TeamTest/2010";

fn trx_with(results: &[(&str, &str, &str)]) -> String {
    // (testName, outcome, message)
    let mut r = String::new();
    for (i, (name, outcome, message)) in results.iter().enumerate() {
        let output = if message.is_empty() { String::new() } else { format!("<Output><ErrorInfo><Message>{message}</Message></ErrorInfo></Output>") };
        r.push_str(&format!("<UnitTestResult testId=\"id{i}\" testName=\"{name}\" outcome=\"{outcome}\">{output}</UnitTestResult>"));
    }
    format!("<?xml version=\"1.0\" encoding=\"utf-8\"?><TestRun xmlns=\"{NS}\"><Results>{r}</Results><TestDefinitions></TestDefinitions></TestRun>")
}

/// Only Passed is a pass and only NotExecuted a skip: an outcome the converter does not know is
/// an error, never a pass (a timed-out or aborted test read as passed is a false green).
#[test]
fn an_outcome_that_is_not_known_is_never_a_pass() {
    let names = ["Passed", "Failed", "NotExecuted", "Timeout", "Aborted", "Inconclusive", "NotRunnable", "Error", "Warning", "Disconnected", "PassedButRunAborted", "Pending", "InProgress", "Completed", "Whatever"];
    let results: Vec<(String, &str, &str)> = names.iter().map(|n| (format!("T.{n}"), *n, "x")).collect();
    let refs: Vec<(&str, &str, &str)> = results.iter().map(|(a, b, c)| (a.as_str(), *b, *c)).collect();
    let out = lines(&trx::to_report(&trx_with(&refs), 1));
    let outcome = |n: &str| out.iter().find(|l| l["id"] == format!("T.{n}")).unwrap()["outcome"].as_str().unwrap().to_string();
    assert_eq!(outcome("Passed"), "passed");
    assert_eq!(outcome("Failed"), "failed");
    assert_eq!(outcome("NotExecuted"), "skipped");
    for n in &names[3..] {
        assert_eq!(outcome(n), "error", "{n}");
        let t = out.iter().find(|l| l["id"] == format!("T.{n}")).unwrap();
        assert_eq!(t["error"]["kind"], "exception", "{n}: no place and no type, so it asks");
        assert!(t["error"]["message"].as_str().unwrap().contains(n), "{n}: the message names the outcome");
    }
    let end = out.iter().find(|l| l["type"] == "end").unwrap();
    assert_eq!(end["counts"]["passed"], 1);
    assert_eq!(end["counts"]["failed"], 1);
    assert_eq!(end["counts"]["skipped"], 1);
    assert_eq!(end["counts"]["errors"], 12);
}

/// A TRX holds a message and no exception type: an assertion needs positive evidence in the
/// words; the spike's converter called everything that was not `Type : message` an assertion
/// (xunit-proven only), this one asks.
#[test]
fn the_kind_comes_from_the_words_and_an_assertion_needs_evidence() {
    let cases = [
        ("xunit assertion", "Assert.Equal() Failure: Values differ&#xD;&#xA;Expected: 5&#xD;&#xA;Actual:   4", "assertion"),
        ("xunit exception", "System.FormatException : The input string 'x1' was not in a correct format.", "exception"),
        ("mstest assertion", "Assert.AreEqual failed. Expected:&lt;5&gt;. Actual:&lt;4&gt;.", "assertion"),
        ("mstest exception", "Test method Tests.X.Y threw exception: &#xA;System.FormatException: Input string was not in a correct format.", "exception"),
        ("nunit assertion", "  Expected: 5&#xA;  But was:  4&#xA;", "assertion"),
        // a missing member the test calls is a build error (CS0117), which writes no TRX; at run time
        // it is a binary mismatch (stale assemblies), not a name the test asks for: an exception
        ("missing method at run time", "System.MissingMethodException : Method not found: 'Int32 CalcLib.Calc.Mul(Int32, Int32)'.", "exception"),
        ("no words", "something failed", "exception"),
        ("empty", "", "exception"),
    ];
    for (what, message, kind) in cases {
        let out = lines(&trx::to_report(&trx_with(&[("T.t", "Failed", message)]), 1));
        let t = out.iter().find(|l| l["type"] == "test").unwrap();
        assert_eq!(t["error"]["kind"], kind, "{what}");
    }
}

#[test]
fn a_not_executed_test_is_a_skip_with_its_reason() {
    let out = lines(&trx::to_report(&trx_with(&[("T.later", "NotExecuted", "not yet"), ("T.now", "Passed", "")]), 0));
    let later = out.iter().find(|l| l["id"] == "T.later").unwrap();
    assert_eq!(later["outcome"], "skipped");
    assert_eq!(later["skip_reason"], "not yet");
    assert_eq!(read_schema(&trx::to_report(&trx_with(&[("T.later", "NotExecuted", "not yet"), ("T.now", "Passed", "")]), 0), 0), Reading::Read { label: "passed".into(), kind: None });
}

#[test]
fn a_byte_order_mark_is_read_past() {
    let plain = trx_with(&[("T.a", "Passed", "")]);
    let with_bom = format!("\u{feff}{plain}");
    assert_eq!(norm(&trx::to_report(&plain, 0)), norm(&trx::to_report(&with_bom, 0)));
    assert!(trx::to_report(&with_bom, 0).contains("\"passed\":1"));
}

#[test]
fn a_test_is_named_by_its_class_and_method_when_the_name_has_no_class() {
    let xml = format!(
        "<TestRun xmlns=\"{NS}\"><Results><UnitTestResult testId=\"a\" testName=\"Parses\" outcome=\"Passed\"/><UnitTestResult testId=\"b\" testName=\"Tests.X.Full\" outcome=\"Passed\"/></Results>\
         <TestDefinitions><UnitTest name=\"Parses\" id=\"a\"><TestMethod className=\"Tests.X\" name=\"Parses\"/></UnitTest><UnitTest name=\"Tests.X.Full\" id=\"b\"><TestMethod className=\"Tests.X\" name=\"Full\"/></UnitTest></TestDefinitions></TestRun>"
    );
    let ids: Vec<String> = lines(&trx::to_report(&xml, 0)).iter().filter(|l| l["type"] == "test").map(|l| l["id"].as_str().unwrap().to_string()).collect();
    assert_eq!(ids, vec!["Tests.X.Parses".to_string(), "Tests.X.Full".to_string()]);
}

#[test]
fn what_cannot_be_read_has_no_session_and_no_end() {
    let good = trx_with(&[("T.a", "Passed", "")]);
    let deep = "<a>".repeat(100_000);
    for (what, native) in [
        ("empty", ""),
        ("cut", &good[..good.len() - 40]),
        ("not xml", "Passed!  - Failed: 0, Passed: 1"),
        ("another document", "<testsuite name=\"s\"><testcase name=\"a\"/></testsuite>"),
        ("nested too deep", deep.as_str()),
        ("control bytes", "\u{0}\u{2}<TestRun"),
    ] {
        let out = trx::to_report(native, 1);
        assert!(lines(&out).iter().all(|l| l["type"] == "kit_error"), "{what}: {out}");
        assert!(matches!(read_schema(&out, 1), Reading::NextReader(_)), "{what}");
    }
}

#[test]
fn a_run_with_no_results_is_a_run_where_nothing_ran_and_the_end_line_has_the_exit() {
    let out = trx::to_report(&trx_with(&[]), 0);
    assert_eq!(read_schema(&out, 0), Reading::Read { label: "no_tests_collected".into(), kind: None });
    assert_eq!(lines(&out).last().unwrap()["exit"], 0);
    assert_eq!(lines(&out).first().unwrap()["runner"], "dotnet-vstest");
    assert_eq!(lines(&out).first().unwrap()["language"], "csharp");
}

/// The TRX's own counters say how many tests failed, errored, timed out or aborted: a run that
/// counts more than its results list has hidden failures, and what is listed is not the picture.
#[test]
fn a_run_that_counts_failures_it_does_not_list_is_unread() {
    for counters in ["failed=\"1\" error=\"0\" timeout=\"0\" aborted=\"0\"", "failed=\"0\" error=\"1\" timeout=\"0\" aborted=\"0\"", "failed=\"0\" error=\"0\" timeout=\"1\" aborted=\"0\"", "failed=\"0\" error=\"0\" timeout=\"0\" aborted=\"1\""] {
        let xml = format!("<TestRun xmlns=\"{NS}\"><Results><UnitTestResult testId=\"a\" testName=\"T.a\" outcome=\"Passed\"/></Results><ResultSummary outcome=\"Failed\"><Counters total=\"2\" executed=\"2\" passed=\"1\" {counters}/></ResultSummary></TestRun>");
        let out = trx::to_report(&xml, 1);
        assert!(lines(&out).iter().all(|l| l["type"] == "kit_error"), "{counters}: {out}");
        assert!(matches!(read_schema(&out, 1), Reading::NextReader(_)), "{counters}");
    }
}

#[test]
fn counters_that_agree_with_the_results_or_are_not_there_are_read() {
    let listed = format!("<TestRun xmlns=\"{NS}\"><Results><UnitTestResult testId=\"a\" testName=\"T.a\" outcome=\"Failed\"/></Results><ResultSummary outcome=\"Failed\"><Counters total=\"1\" executed=\"1\" passed=\"0\" failed=\"1\" error=\"0\" timeout=\"0\" aborted=\"0\"/></ResultSummary></TestRun>");
    assert!(trx::to_report(&listed, 1).contains("\"session\""));
    let no_summary = trx_with(&[("T.a", "Failed", "x")]);
    assert!(trx::to_report(&no_summary, 1).contains("\"session\""));
    // an inconclusive or not-executed test is not among the counters of bad outcomes
    let off = format!("<TestRun xmlns=\"{NS}\"><Results><UnitTestResult testId=\"a\" testName=\"T.a\" outcome=\"NotExecuted\"/></Results><ResultSummary outcome=\"Completed\"><Counters total=\"1\" executed=\"0\" passed=\"0\" failed=\"0\" error=\"0\" timeout=\"0\" aborted=\"0\" notExecuted=\"1\"/></ResultSummary></TestRun>");
    assert!(trx::to_report(&off, 0).contains("\"session\""));
}
