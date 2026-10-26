//! Lane AKreports (plan of record 2026-10-19, A section: the report-file readers). A JUnit XML
//! report (Maven surefire/failsafe, Gradle, PHPUnit `--log-junit`, and the C/C++ runners' own
//! JUnit-shaped files) becomes the per-test result file (`kit/SCHEMA.txt`) and reads its truth
//! through it; what it cannot read it leaves unread, never wrong.
//!
//! ```text
//!   TEST-*.xml ──▶ junit::to_report_files ──▶ session · test… · end ──▶ the reader
//!      │                   │
//!      │                   └─ older than run_started? ignored (a compile error leaves the
//!      │                      last run's XML behind: read blindly it is a wrong reading)
//!      └─ unreadable (empty, cut, not XML)? no session: the next reader decides
//! ```
//!
//! The recordings are the A0 spike's (`tests/fixtures/probe_runs/`): Maven 3.9.16 surefire 3.5.2,
//! Gradle 8.10.2, PHPUnit 13.4.2 `--log-junit`, each probe P1-P7 in a project of its own. Report
//! readers never give `missing_target_symbol` or `collection_error` on the JVM (a compile error
//! writes no report): those probes read can't-tell here and stay with the console tables.

mod probe_runs_support;

use familiar_ai::door::convert::junit;
use familiar_ai::door::convert::schema::{Context, Native};
use probe_runs_support::*;
use serde_json::Value;
use std::time::{Duration, SystemTime};

/// What each Java probe reads through the report files, the JVM's report readers' limit included.
const JVM: [(&str, Verdict); 8] = [
    ("P1", Verdict::Right),
    ("P2", Verdict::Right),
    ("P3", Verdict::CantTell),
    ("P3b", Verdict::Right),
    ("P4", Verdict::CantTell),
    ("P5", Verdict::Right),
    ("P6", Verdict::CantTell),
    ("P7", Verdict::Right),
];

/// PHPUnit's log is written for every run that gets as far as the logger: only P4 (a `require` of
/// a missing file, exit 255, a log file left empty) writes nothing readable.
const PHP: [(&str, Verdict); 8] = [
    ("P1", Verdict::Right),
    ("P2", Verdict::Right),
    ("P3", Verdict::Right),
    ("P3b", Verdict::Right),
    ("P4", Verdict::CantTell),
    ("P5", Verdict::Right),
    ("P6", Verdict::Right),
    ("P7", Verdict::Right),
];

fn lines(out: &str) -> Vec<Value> {
    out.lines().map(|l| serde_json::from_str(l).expect("every line is JSON")).collect()
}

fn natives(files: &[(String, String)], modified: SystemTime) -> Vec<Native<'_>> {
    files.iter().map(|(n, t)| Native { name: n, text: t, modified }).collect()
}

/// A recording's report files, converted as a run that began at `run_started()` and wrote them after.
fn convert(files: &[(String, String)], ctx: &Context<'_>, exit: i32) -> String {
    junit::to_report_files(&natives(files, after_start()), ctx, Some(run_started()), exit)
}

fn surefire(dir: &std::path::Path) -> Vec<(String, String)> {
    files_under(&dir.join("report/surefire-reports"), &|n| n.starts_with("TEST-") && n.ends_with(".xml"))
}

fn gradle(dir: &std::path::Path) -> Vec<(String, String)> {
    files_under(&dir.join("report/test-results"), &|n| n.ends_with(".xml"))
}

fn php(dir: &std::path::Path) -> Vec<(String, String)> {
    files_under(&dir.join("report"), &|n| n == "junit.xml")
}

#[test]
fn maven_surefire_reports_read_their_truths() {
    let ctx = Context::new("maven-surefire", "java");
    for setup in ["plain", "quiet"] {
        for (probe, expected) in JVM {
            let dir = recording("maven", setup, probe);
            let out = convert(&surefire(&dir), &ctx, exit_of(&dir));
            let reading = read_schema(&out, exit_of(&dir));
            assert_eq!(verdict(&reading, probe), expected, "maven {setup} {probe}: read {reading:?}");
        }
    }
}

#[test]
fn gradle_test_results_read_their_truths() {
    let ctx = Context::new("gradle", "java");
    for (probe, expected) in JVM {
        let dir = recording("gradle", "plain", probe);
        let out = convert(&gradle(&dir), &ctx, exit_of(&dir));
        let reading = read_schema(&out, exit_of(&dir));
        assert_eq!(verdict(&reading, probe), expected, "gradle {probe}: read {reading:?}");
    }
}

#[test]
fn phpunit_log_junit_reads_its_truths() {
    for setup in ["plain-junit", "compact-junit", "testdox-junit"] {
        for (probe, expected) in PHP {
            let dir = recording("phpunit", setup, probe);
            let files = php(&dir);
            let root = files.first().and_then(|(_, t)| php_root(t));
            let ctx = Context::new("phpunit", "php");
            let ctx = match &root {
                Some(r) => ctx.with_root(r),
                None => ctx,
            };
            let out = convert(&files, &ctx, exit_of(&dir));
            let reading = read_schema(&out, exit_of(&dir));
            assert_eq!(verdict(&reading, probe), expected, "phpunit {setup} {probe}: read {reading:?}");
        }
    }
}

/// What the recordings prove for every report reader together: no probe in any setup reads wrong.
#[test]
fn no_recording_reads_wrong() {
    let mut read = 0;
    for (runner, setups) in [("maven", vec!["plain", "quiet"]), ("gradle", vec!["plain"]), ("phpunit", vec!["plain-junit", "compact-junit", "testdox-junit"])] {
        for setup in setups {
            for probe in PROBES {
                let dir = recording(runner, setup, probe);
                let (files, ctx) = match runner {
                    "maven" => (surefire(&dir), Context::new("maven-surefire", "java")),
                    "gradle" => (gradle(&dir), Context::new("gradle", "java")),
                    _ => (php(&dir), Context::new("phpunit", "php")),
                };
                let out = convert(&files, &ctx, exit_of(&dir));
                let v = verdict(&read_schema(&out, exit_of(&dir)), probe);
                assert_ne!(v, Verdict::Wrong, "{runner} {setup} {probe} reads wrong");
                read += 1;
            }
        }
    }
    assert_eq!(read, 6 * 8, "every recording was read");
}

/// The Rust converter says what the spike's proven converter said (`converted.jsonl`), probe by
/// probe: ids, outcomes, kinds, types, the deepest frame, the counts.
#[test]
fn the_converter_says_what_the_spike_converter_said() {
    let mut compared = 0;
    for (runner, setup, runner_name, language) in [("maven", "plain", "maven-surefire", "java"), ("gradle", "plain", "gradle", "java"), ("phpunit", "plain-junit", "phpunit", "php")] {
        for probe in PROBES {
            let dir = recording(runner, setup, probe);
            let Ok(spike) = std::fs::read_to_string(dir.join("report/converted.jsonl")) else { continue };
            let files = match runner {
                "maven" => surefire(&dir),
                "gradle" => gradle(&dir),
                _ => php(&dir),
            };
            let ctx = Context::new(runner_name, language);
            let ours = convert(&files, &ctx, exit_of(&dir));
            if spike.lines().all(|l| l.contains("kit_error")) {
                assert!(lines(&ours).iter().all(|l| l["type"] != "session"), "{runner} {probe}: the spike read nothing, so do we");
            } else {
                assert_eq!(norm(&ours), norm(&spike), "{runner} {setup} {probe}");
            }
            compared += 1;
        }
    }
    // the spike converted the five Maven and five Gradle probes that wrote a report, and eight PHPUnit logs
    assert_eq!(compared, 18, "the spike's conversions were compared");
}

/// A compile error leaves the last run's XML in `target/surefire-reports` (the recorded
/// `stale-report/P3`): read blindly it is a failed assertion, which P3 is not; with the run's
/// start it is ignored and the reader asks.
#[test]
fn a_stale_report_is_ignored_and_read_blindly_it_reads_wrong() {
    let dir = recording("maven", "stale-report", "P3");
    let files = surefire(&dir);
    assert_eq!(files.len(), 1, "the earlier run's XML is there");
    let ctx = Context::new("maven-surefire", "java");
    let exit = exit_of(&dir);

    let blind = junit::to_report_files(&natives(&files, before_start()), &ctx, None, exit);
    assert_eq!(verdict(&read_schema(&blind, exit), "P3"), Verdict::Wrong, "read blindly the old file is a wrong reading");

    let stale = junit::to_report_files(&natives(&files, before_start()), &ctx, Some(run_started()), exit);
    assert!(lines(&stale).iter().all(|l| l["type"] != "session"), "a file older than the run has no session: {stale}");
    assert_eq!(verdict(&read_schema(&stale, exit), "P3"), Verdict::CantTell);
}

#[test]
fn a_report_written_at_the_runs_start_is_fresh_and_one_before_it_is_stale() {
    let dir = recording("maven", "plain", "P1");
    let files = surefire(&dir);
    let ctx = Context::new("maven-surefire", "java");
    let at = |m: SystemTime| junit::to_report_files(&natives(&files, m), &ctx, Some(run_started()), 0);
    assert!(at(run_started()).contains("\"session\""), "the start itself is fresh");
    assert!(!at(run_started() - Duration::from_secs(1)).contains("\"session\""), "a second before the start is stale");
    assert!(at(run_started() + Duration::from_secs(86_400)).contains("\"session\""), "later is fresh");
}

#[test]
fn only_the_fresh_files_of_a_run_are_read() {
    let old = "<testsuite name=\"old\" tests=\"1\"><testcase name=\"gone\" classname=\"o.T\"><failure type=\"X\" message=\"m\"/></testcase></testsuite>";
    let new = "<testsuite name=\"new\" tests=\"1\"><testcase name=\"now\" classname=\"n.T\"/></testsuite>";
    let files = [
        Native { name: "TEST-o.T.xml", text: old, modified: before_start() },
        Native { name: "TEST-n.T.xml", text: new, modified: after_start() },
    ];
    let out = junit::to_report_files(&files, &Context::new("maven-surefire", "java"), Some(run_started()), 0);
    let tests: Vec<Value> = lines(&out).into_iter().filter(|l| l["type"] == "test").collect();
    assert_eq!(tests.len(), 1, "{out}");
    assert_eq!(tests[0]["id"], "n.T::now");
    assert_eq!(read_schema(&out, 0), Reading::Read { label: "passed".into(), kind: None });
}

#[test]
fn the_kind_comes_from_the_exception_type_never_from_failure_versus_error() {
    // Gradle writes every exception as <failure>; surefire and PHPUnit split them, not by type.
    let xml = r#"<testsuite name="s" tests="6" failures="4" errors="2">
  <testcase name="parses()" classname="calc.T"><failure message="For input string" type="java.lang.NumberFormatException">java.lang.NumberFormatException: For input string
	at calc.Calc.parse(Calc.java:9)
	at calc.T.parses(T.java:10)
</failure></testcase>
  <testcase name="adds()" classname="calc.T"><failure message="expected: &lt;5&gt; but was: &lt;4&gt;" type="org.opentest4j.AssertionFailedError">org.opentest4j.AssertionFailedError: expected
	at calc.T.adds(T.java:12)
</failure></testcase>
  <testcase name="muls()" classname="calc.T"><failure message="Calc.mul" type="java.lang.NoSuchMethodError">java.lang.NoSuchMethodError: Calc.mul
	at calc.T.muls(T.java:14)
</failure></testcase>
  <testcase name="init()" classname="calc.T"><error message="Could not initialize class calc.Calc" type="java.lang.NoClassDefFoundError">java.lang.NoClassDefFoundError: Could not initialize class calc.Calc
	at calc.T.init(T.java:20)
</error></testcase>
  <testcase name="old()" classname="calc.T"><failure message="x" type="java.lang.AssertionError">java.lang.AssertionError: x
	at calc.T.old(T.java:16)
</failure></testcase>
  <testcase name="boom()" classname="calc.T"><error message="m" type="java.lang.AssertionError">java.lang.AssertionError: m
	at calc.T.boom(T.java:18)
</error></testcase>
</testsuite>"#;
    let out = junit::to_report(xml, 1);
    let kinds: Vec<(String, String, String)> = lines(&out)
        .iter()
        .filter(|l| l["type"] == "test")
        .map(|l| (l["id"].as_str().unwrap().to_string(), l["outcome"].as_str().unwrap().to_string(), l["error"]["kind"].as_str().unwrap().to_string()))
        .collect();
    let want = [
        ("calc.T::parses()", "failed", "exception"),
        ("calc.T::adds()", "failed", "assertion"),
        // a linkage error at run time is stale classes or a broken classpath, not the name the test
        // asks for (that is a compile error, which writes no report): it is an exception, never a
        // missing name that would seal a first red
        ("calc.T::muls()", "failed", "exception"),
        ("calc.T::init()", "error", "exception"),
        ("calc.T::old()", "failed", "assertion"),
        ("calc.T::boom()", "error", "assertion"),
    ];
    assert_eq!(kinds, want.iter().map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string())).collect::<Vec<_>>());
}

/// A failure that names no type and shows no place is no evidence of an assertion: it reads as an
/// exception, which at the planned test's own line asks instead of sealing.
#[test]
fn a_failure_with_no_type_and_no_place_is_not_called_an_assertion() {
    let out = junit::to_report("<testsuite name=\"s\"><testcase name=\"t\" classname=\"c\"><failure message=\"boom\"/></testcase></testsuite>", 1);
    let t = lines(&out).into_iter().find(|l| l["type"] == "test").unwrap();
    assert_eq!(t["error"]["kind"], "exception");
    assert_eq!(t["error"]["frames"], serde_json::json!([]));
}

#[test]
fn the_frames_are_the_projects_with_the_deepest_last() {
    let dir = recording("maven", "plain", "P7");
    let out = convert(&surefire(&dir), &Context::new("maven-surefire", "java"), 1);
    let t = lines(&out).into_iter().find(|l| l["type"] == "test").unwrap();
    let frames: Vec<(String, u64)> = t["error"]["frames"].as_array().unwrap().iter().map(|f| (f["file"].as_str().unwrap().to_string(), f["line"].as_u64().unwrap())).collect();
    // the JDK's, JUnit's and the build tool's frames are not the project's; the test first, the code it called last
    assert_eq!(frames, vec![("ProbeTest.java".to_string(), 10), ("Calc.java".to_string(), 9)]);
    assert_eq!(t["error"]["type"], "java.lang.NumberFormatException");
}

#[test]
fn php_paths_are_relative_to_the_projects_root() {
    let dir = recording("phpunit", "plain-junit", "P7");
    let files = php(&dir);
    let root = php_root(&files[0].1).expect("the recorded project's root");
    let out = convert(&files, &Context::new("phpunit", "php").with_root(&root), 2);
    let t = lines(&out).into_iter().find(|l| l["type"] == "test").unwrap();
    assert_eq!(t["file"], "tests/ProbeTest.php");
    let frames: Vec<&str> = t["error"]["frames"].as_array().unwrap().iter().map(|f| f["file"].as_str().unwrap()).collect();
    assert_eq!(frames, vec!["tests/ProbeTest.php", "src/Calc.php"]);
}

#[test]
fn a_php_message_the_type_attribute_lacks_is_found_in_the_text() {
    // PHPUnit's log has no message attribute: "Class::test", the message, a blank line, the trace
    let dir = recording("phpunit", "plain-junit", "P3");
    let out = convert(&php(&dir), &Context::new("phpunit", "php"), 2);
    let t = lines(&out).into_iter().find(|l| l["type"] == "test").unwrap();
    assert_eq!(t["error"]["message"], "Error: Call to undefined method App\\Calc::mul()");
    assert_eq!(t["error"]["kind"], "missing_name");
}

#[test]
fn skips_keep_their_reason_and_count_apart_from_passes() {
    let xml = "<testsuites><testsuite name=\"s\" tests=\"3\"><testcase name=\"a\" classname=\"c\"/><testcase name=\"b\" classname=\"c\"><skipped message=\"not yet\"/></testcase><testcase name=\"d\" classname=\"c\"><skipped/></testcase></testsuite></testsuites>";
    let out = lines(&junit::to_report(xml, 0));
    let skips: Vec<&Value> = out.iter().filter(|l| l["outcome"] == "skipped").collect();
    assert_eq!(skips.len(), 2);
    assert_eq!(skips[0]["skip_reason"], "not yet");
    assert_eq!(skips[1]["skip_reason"], Value::Null);
    let end = out.iter().find(|l| l["type"] == "end").unwrap();
    assert_eq!(end["counts"]["passed"], 1);
    assert_eq!(end["counts"]["skipped"], 2);
    assert_eq!(end["counts"]["failed"], 0);
    assert_eq!(end["counts"]["errors"], 0);
    assert_eq!(end["counts"]["deselected"], 0);
}

#[test]
fn the_end_line_carries_the_process_exit_and_the_session_names_the_runner() {
    let out = lines(&junit::to_report_files(&[Native { name: "r.xml", text: "<testsuite name=\"s\"><testcase name=\"a\" classname=\"c\"/></testsuite>", modified: after_start() }], &Context::new("gradle", "java"), Some(run_started()), 137));
    assert_eq!(out.first().unwrap()["type"], "session");
    assert_eq!(out.first().unwrap()["v"], 1);
    assert_eq!(out.first().unwrap()["runner"], "gradle");
    assert_eq!(out.first().unwrap()["language"], "java");
    assert_eq!(out.last().unwrap()["type"], "end");
    assert_eq!(out.last().unwrap()["exit"], 137);
}

#[test]
fn entities_cdata_and_namespaces_are_read_as_xml_does() {
    let xml = "<ns:testsuite xmlns:ns=\"urn:x\" name=\"s\"><ns:testcase name=\"a &lt; b &amp; c\" classname=\"c\"><ns:failure message=\"x &quot;y&quot;&#x0A;z\" type=\"AssertionError\"><![CDATA[AssertionError: <raw> & more\n\tat c.T.a(T.java:3)\n]]></ns:failure></ns:testcase></ns:testsuite>";
    let t = lines(&junit::to_report(xml, 1)).into_iter().find(|l| l["type"] == "test").unwrap();
    assert_eq!(t["id"], "c::a < b & c");
    assert_eq!(t["error"]["message"], "x \"y\"\nz");
    assert_eq!(t["error"]["frames"][0]["line"], 3);
}

/// An empty file, a cut one, text that is not XML, other XML: the run has no structured output.
/// A kit that cannot read says so in a `kit_error` line and writes no session and no end.
#[test]
fn what_cannot_be_read_has_no_session_and_no_end() {
    let good = "<testsuite name=\"s\"><testcase name=\"a\" classname=\"c\"/></testsuite>";
    let cut = &good[..good.len() - 30];
    let deep = "<a>".repeat(100_000);
    for (what, native) in [
        ("empty", ""),
        ("blank", "  \n"),
        ("cut mid-tag", cut),
        ("cut after a test", "<testsuite name=\"s\"><testcase name=\"a\" classname=\"c\"/>"),
        ("not xml", "Tests run: 1, Failures: 0"),
        ("control bytes", "\u{0}\u{1}<<<>>>&&&"),
        ("a bare <", "<"),
        ("nested too deep", deep.as_str()),
        ("another document", "<html><body><p>hello</p></body></html>"),
    ] {
        let out = junit::to_report(native, 1);
        assert!(lines(&out).iter().all(|l| l["type"] == "kit_error"), "{what}: only kit_error lines, got {out}");
        assert_eq!(verdict_or_ask(&out), "ask", "{what}");
    }
}

fn verdict_or_ask(out: &str) -> &'static str {
    match read_schema(out, 1) {
        Reading::NextReader(_) => "ask",
        _ => "read",
    }
}

#[test]
fn one_unreadable_file_among_good_ones_leaves_the_whole_run_unread() {
    let good = "<testsuite name=\"s\"><testcase name=\"a\" classname=\"c\"/></testsuite>";
    let files = [
        Native { name: "TEST-a.xml", text: good, modified: after_start() },
        Native { name: "TEST-b.xml", text: "<testsuite name=\"s\"><testcase", modified: after_start() },
    ];
    let out = junit::to_report_files(&files, &Context::new("maven-surefire", "java"), Some(run_started()), 0);
    assert!(lines(&out).iter().all(|l| l["type"] != "session" && l["type"] != "test" && l["type"] != "end"), "{out}");
    // a green read off the readable half would be a wrong reading
    assert_eq!(verdict_or_ask(&out), "ask");
}

#[test]
fn a_doctype_with_entities_is_never_expanded() {
    let xml = "<!DOCTYPE x [<!ENTITY e \"boom\"><!ENTITY f SYSTEM \"file:///etc/passwd\">]><testsuite name=\"s\"><testcase name=\"a&e;\" classname=\"c\"/></testsuite>";
    let out = junit::to_report(xml, 0);
    assert!(!out.contains("boom") && !out.contains("root:"), "{out}");
}

#[test]
fn a_suite_with_no_tests_is_a_run_where_nothing_ran() {
    // PHPUnit writes <testsuites/> for a class with no tests; the reader decides what nothing means
    let out = junit::to_report("<?xml version=\"1.0\"?><testsuites/>", 1);
    assert_eq!(read_schema(&out, 1), Reading::Read { label: "no_tests_collected".into(), kind: None });
}

#[test]
fn to_report_reads_one_document_with_no_staleness_to_judge() {
    let xml = "<testsuite name=\"s\"><testcase name=\"a\" classname=\"c\"/></testsuite>";
    let one = junit::to_report(xml, 0);
    let many = junit::to_report_files(&[Native { name: "r.xml", text: xml, modified: before_start() }], &Context::new("junit", ""), None, 0);
    assert_eq!(norm(&one), norm(&many));
    assert!(one.contains("\"passed\":1"));
}

#[test]
fn the_same_input_gives_the_same_bytes() {
    let dir = recording("maven", "plain", "P2");
    let files = surefire(&dir);
    let ctx = Context::new("maven-surefire", "java");
    assert_eq!(convert(&files, &ctx, 1), convert(&files, &ctx, 1));
}

/// A suite that declares failures or errors it does not show (a failed class setup or an engine
/// error with no test to carry it) is an incomplete picture: the green of its other tests would be
/// a wrong reading, so the report is unread.
#[test]
fn a_suite_that_declares_failures_it_does_not_show_is_unread() {
    for declared in ["failures=\"1\" errors=\"0\"", "failures=\"0\" errors=\"1\"", "failures=\"2\" errors=\"0\""] {
        let xml = format!("<testsuite name=\"s\" tests=\"1\" {declared}><testcase name=\"a\" classname=\"c\"/></testsuite>");
        let out = junit::to_report(&xml, 1);
        assert!(lines(&out).iter().all(|l| l["type"] == "kit_error"), "{declared}: {out}");
        assert!(matches!(read_schema(&out, 1), Reading::NextReader(_)), "{declared}");
    }
    // none to show at all
    let bare = junit::to_report("<testsuite name=\"s\" tests=\"0\" failures=\"0\" errors=\"1\"/>", 1);
    assert!(lines(&bare).iter().all(|l| l["type"] == "kit_error"), "{bare}");
}

/// What a suite declares is counted by the failure and error elements it shows (Catch2 writes one
/// per failed assertion), at the leaves (PHPUnit's parents sum their children), and a writer that
/// declares nothing is read as it shows.
#[test]
fn a_suite_that_declares_what_it_shows_is_read() {
    let none = "<testsuite name=\"s\"><testcase name=\"a\" classname=\"c\"><failure message=\"m\" type=\"AssertionError\"/></testcase></testsuite>";
    assert!(junit::to_report(none, 1).contains("\"session\""));

    let per_assertion = "<testsuite name=\"s\" failures=\"3\" errors=\"0\" tests=\"1\"><testcase name=\"a\" classname=\"c\"><failure message=\"1\" type=\"CHECK\"/><failure message=\"2\" type=\"CHECK\"/><failure message=\"3\" type=\"CHECK\"/></testcase></testsuite>";
    assert!(junit::to_report(per_assertion, 1).contains("\"session\""));

    let nested = "<testsuites><testsuite name=\"root\" tests=\"2\" failures=\"1\"><testsuite name=\"leaf\" tests=\"2\" failures=\"1\" errors=\"0\"><testcase name=\"a\" classname=\"c\"><failure type=\"AssertionError\" message=\"m\"/></testcase><testcase name=\"b\" classname=\"c\"/></testsuite></testsuite></testsuites>";
    assert!(junit::to_report(nested, 1).contains("\"session\""));

    // declaring fewer than it shows is the writer's rounding, not a hidden failure
    let fewer = "<testsuite name=\"s\" failures=\"0\" errors=\"0\"><testcase name=\"a\" classname=\"c\"><failure message=\"m\" type=\"AssertionError\"/></testcase></testsuite>";
    assert!(junit::to_report(fewer, 1).contains("\"session\""));
}
