//! Lane AKjs (plan of record 2026-10-19, A): node --test's TAP, as `door::convert::tap` turns it into
//! the per-test result file [draft], read through the same order as the kit's own file.
//!
//! ```text
//!   node ≤ 22 on a pipe prints TAP        stdout ──▶ tap::to_report_in(stdout, exit, root) ──▶ JSON Lines
//!   (the kit's second reporter)                                                               │
//!   spike recordings: stdout + the kit's own report/results.jsonl for the same run  ◀── equal ─┘
//! ```
//!
//! TAP gives no file for a test that passed and no versions; it does give the failing test's
//! location, error, name and stack. The converter never reads wrong: output that is not a complete
//! TAP run (no header, no summary) converts to nothing, which has no structured output.

mod probe_reading;

use familiar_ai::door::convert::tap;
use probe_reading::{read, recordings, verdict, Verdict};
use serde_json::Value;
use std::path::Path;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tap").join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn lines(report: &str) -> Vec<Value> {
    report.lines().map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{l}: {e}"))).collect()
}

fn of<'a>(lines: &'a [Value], t: &str) -> Vec<&'a Value> {
    lines.iter().filter(|l| l["type"] == t).collect()
}

/// The probes were laid out `<root>/test/<probe>.test.js`: the root is what precedes `/test/` in the
/// first location the run printed.
fn root_of(tap: &str) -> String {
    tap.lines().find_map(|l| l.trim().strip_prefix("location: '")).and_then(|l| l.split("/test/").next()).unwrap_or("").to_string()
}

fn is_tap(stdout: &str) -> bool {
    stdout.starts_with("TAP version")
}

/// The truth of a probe, as SCHEMA.txt has it.
fn truth_of(probe: &str) -> (&'static str, &'static str) {
    match probe {
        "P1" | "P3b" | "P5" => ("passed", ""),
        "P2" => ("tests_failed", "assertion"),
        "P3" => ("missing_target_symbol", ""),
        "P4" => ("collection_error", ""),
        "P6" => ("no_tests_collected", ""),
        "P7" => ("tests_failed", "exception"),
        other => panic!("probe {other}"),
    }
}

#[test]
fn every_recorded_tap_reads_its_probes_truth() {
    let mut read_right = 0;
    for rec in recordings("node-test") {
        let stdout = rec.file("stdout");
        if !is_tap(&stdout) {
            continue; // node 24's spec, the dot reporter, the controls
        }
        let report = tap::to_report_in(&stdout, rec.exit().unwrap(), &root_of(&stdout));
        let got = read(&report, rec.exit());
        assert_eq!(verdict(&got, truth_of(&rec.probe)), Verdict::Right, "{}/{}: read {} ({}) from\n{report}", rec.setup, rec.probe, got.spelled(), got.why);
        read_right += 1;
    }
    assert!(read_right >= 24, "only {read_right} TAP recordings");
}

/// A run cut anywhere converts to nothing or to the whole truth: the summary comes after every test,
/// so a cut that keeps it keeps them all. And nothing panics on the way.
#[test]
fn no_prefix_of_a_tap_run_converts_to_a_wrong_reading_or_a_panic() {
    let (mut tried, mut converted) = (0, 0);
    for rec in recordings("node-test") {
        let stdout = rec.file("stdout");
        if !is_tap(&stdout) {
            continue;
        }
        let root = root_of(&stdout);
        for end in (0..=stdout.len()).filter(|i| stdout.is_char_boundary(*i)) {
            let report = tap::to_report_in(&stdout[..end], rec.exit().unwrap(), &root);
            tried += 1;
            if report.is_empty() {
                continue;
            }
            converted += 1;
            let got = read(&report, rec.exit());
            assert_eq!(verdict(&got, truth_of(&rec.probe)), Verdict::Right, "{}/{} cut at {end}: read {} ({})", rec.setup, rec.probe, got.spelled(), got.why);
        }
    }
    assert!(tried > 15_000 && converted > 24, "{tried} cuts, {converted} converted");
}

/// What the TAP and the kit agree on, test by test: outcome, kind, where the deepest frame is.
fn essence(report: &str) -> Vec<String> {
    let ls = lines(report);
    let mut out = Vec::new();
    for l in &ls {
        match l["type"].as_str().unwrap() {
            "test" => {
                let frames = l["error"]["frames"].as_array().cloned().unwrap_or_default();
                let at = frames.last().map(|f| format!("{}:{}", f["file"].as_str().unwrap_or(""), f["line"])).unwrap_or_default();
                out.push(format!("test {} {} {} {}", l["id"].as_str().unwrap().rsplit("::").next().unwrap(), l["outcome"], l["error"]["kind"], at));
            }
            "collect_error" => out.push(format!("collect_error {} {}", l["file"], l["error"]["kind"])),
            "end" => {
                let c = &l["counts"];
                out.push(format!("end passed {} failed {} skipped {} errors {}", c["passed"], c["failed"], c["skipped"], c["errors"]));
            }
            _ => {}
        }
    }
    out
}

#[test]
fn the_converted_run_says_what_the_kit_said() {
    let mut compared = 0;
    for rec in recordings("node-test") {
        let stdout = rec.file("stdout");
        if !is_tap(&stdout) || rec.report().is_empty() {
            continue;
        }
        let report = tap::to_report_in(&stdout, rec.exit().unwrap(), &root_of(&stdout));
        assert_eq!(essence(&report), essence(&rec.report()), "{}/{}: the TAP and the kit disagree", rec.setup, rec.probe);
        compared += 1;
    }
    assert!(compared >= 24, "only {compared} runs compared");
}

#[test]
fn a_run_that_is_not_a_complete_tap_converts_to_nothing() {
    let mut cut = 0;
    for rec in recordings("node-test") {
        let stdout = rec.file("stdout");
        if !is_tap(&stdout) {
            // spec, dot and nothing at all are not TAP
            assert_eq!(tap::to_report(&stdout, rec.exit().unwrap()), "", "{}/{}: {stdout:?}", rec.setup, rec.probe);
            continue;
        }
        // the summary cut off: the run did not finish
        let upto = stdout.find("\n# tests").expect("a summary");
        assert_eq!(tap::to_report(&stdout[..upto], 1), "", "{}/{}: cut before the summary", rec.setup, rec.probe);
        // the header cut off
        assert_eq!(tap::to_report(stdout.split_once('\n').unwrap().1, 1), "", "{}/{}: no header", rec.setup, rec.probe);
        cut += 1;
    }
    assert!(cut >= 24);
    for junk in ["", "\n", "not tap at all", "TAP version 13", "TAP version 13\n1..0", "# tests 1\n# pass 1\n# fail 0"] {
        assert_eq!(tap::to_report(junk, 0), "", "{junk:?}");
    }
}

#[test]
fn nested_suites_skips_and_todos_are_read_as_node_prints_them() {
    let report = tap::to_report_in(&fixture("node22_n.tap"), 1, "/w");
    let ls = lines(&report);
    let tests = of(&ls, "test");
    let got: Vec<(String, String)> = tests.iter().map(|t| (t["id"].as_str().unwrap().to_string(), t["outcome"].as_str().unwrap().to_string())).collect();
    assert_eq!(
        got,
        [
            ("::calc > add > adds small numbers", "passed"),
            ("::calc > add > is skipped", "skipped"),
            ("n.test.cjs::calc > fails on purpose", "failed"),
            ("::todo item", "skipped"),
            ("::has - a dash and # a hash", "passed"),
        ]
        .map(|(a, b)| (a.to_string(), b.to_string())),
        "suites are not tests; a passing test has no file in TAP"
    );
    assert_eq!(tests[1]["skip_reason"], "later");
    assert_eq!(tests[3]["skip_reason"], "not yet");
    let failed = tests[2];
    assert_eq!(failed["file"], "n.test.cjs");
    assert_eq!(failed["line"], 8);
    assert_eq!(failed["error"]["type"], "AssertionError");
    assert_eq!(failed["error"]["kind"], "assertion");
    assert!(failed["error"]["message"].as_str().unwrap().contains("2 !== 3"));
    // outermost first, deepest last, and only the project's own frames
    assert_eq!(failed["error"]["frames"], serde_json::json!([{"file": "n.test.cjs", "line": 8, "function": "TestContext.<anonymous>"}]));
    let end = of(&ls, "end")[0];
    assert_eq!(end["exit"], 1);
    assert_eq!(end["counts"], serde_json::json!({"passed": 2, "failed": 1, "skipped": 2, "deselected": 0, "errors": 0}));
    assert_eq!(read(&report, Some(1)).spelled(), "tests_failed/assertion");
}

#[test]
fn a_failing_hook_is_a_setup_error_and_a_cancelled_child_is_a_failure_with_no_place() {
    let report = tap::to_report_in(&fixture("node22_e.tap"), 1, "/w");
    let got = read(&report, Some(1));
    assert_eq!(got.label, "setup_error", "{report}");
    // a hook that failed in a suite fails the suite (node says hookFailed, once, on the suite); its
    // child is cancelled, which is a failure with no place; the test after it passed
    let report = tap::to_report_in(&fixture("node22_h.tap"), 1, "/w");
    let ls = lines(&report);
    let tests = of(&ls, "test");
    assert_eq!(tests.len(), 3, "{report}");
    assert_eq!((tests[0]["id"].as_str(), tests[0]["outcome"].as_str()), (Some("h.test.cjs::suite > inner"), Some("failed")));
    assert_eq!(tests[0]["error"]["frames"], serde_json::json!([]), "no stack, no place");
    assert_eq!((tests[1]["id"].as_str(), tests[1]["outcome"].as_str(), tests[1]["error"]["kind"].as_str()), (Some("h.test.cjs::suite (hook)"), Some("failed"), Some("setup")));
    assert_eq!(tests[1]["error"]["message"], "boom in before");
    assert_eq!(tests[2]["outcome"], "passed");
    assert_eq!(read(&report, Some(1)).label, "setup_error");
}

#[test]
fn files_are_relative_to_the_root_when_it_is_given_and_as_printed_when_not() {
    let native = fixture("node22_n.tap");
    let with = lines(&tap::to_report_in(&native, 1, "/w"));
    assert_eq!(of(&with, "test")[2]["file"], "n.test.cjs");
    let without = lines(&tap::to_report(&native, 1));
    assert_eq!(of(&without, "test")[2]["file"], "/w/n.test.cjs");
    // a root that is not a directory prefix does not strip: /w is not /wx
    let other = lines(&tap::to_report_in(&native, 1, "/wx"));
    assert_eq!(of(&other, "test")[2]["file"], "/w/n.test.cjs");
}

#[test]
fn the_session_names_the_runner_and_claims_no_version() {
    let ls = lines(&tap::to_report(&fixture("node22_e.tap"), 1));
    let session = of(&ls, "session")[0];
    assert_eq!((session["v"].clone(), session["runner"].as_str(), session["language"].as_str()), (serde_json::json!(1), Some("node-test"), Some("javascript")));
    assert!(session["runner_version"].is_null() && session["language_version"].is_null(), "TAP carries no version");
    assert_eq!(ls[0]["type"], "session");
    assert_eq!(ls.last().unwrap()["type"], "end");
}
