//! Lane AKreports (plan of record 2026-10-19, A section; C/C++ last, A-7). The C/C++ report
//! readers (googletest's `GTEST_OUTPUT=xml:<folder>/`, Catch2 v3's `XML_OUTPUT_FILE` under
//! `BAZEL_TEST=1`, `ctest --output-junit`) write JUnit-shaped XML that `junit::to_report_files`
//! reads into the per-test result file.
//!
//! These samples are NOT recordings: the A0 spike did not run C/C++, and no probe kit calibrates
//! these readers (`kits::gtest::CALIBRATED` is false). They are hand-made from each format's
//! documented shape, to pin what the converter does with it: a place only where the format names
//! one, a skip never a pass, a failure that names nothing an exception (which asks at the planned
//! test's own line), and a report cut short unread. The first real recording replaces a sample.

mod probe_runs_support;

use familiar_ai::door::convert::junit;
use familiar_ai::door::convert::schema::{Context, Native};
use probe_runs_support::*;
use serde_json::Value;

const GTEST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites tests="5" failures="2" disabled="1" errors="0" time="0.003" timestamp="2026-10-19T12:00:00" name="AllTests">
  <testsuite name="CalcTest" tests="5" failures="2" disabled="1" skipped="1" errors="0" time="0.002" timestamp="2026-10-19T12:00:00">
    <testcase name="Adds" file="calc_test.cc" line="5" status="run" result="completed" time="0" timestamp="2026-10-19T12:00:00" classname="CalcTest" />
    <testcase name="AddsWrong" file="calc_test.cc" line="9" status="run" result="completed" time="0" timestamp="2026-10-19T12:00:00" classname="CalcTest">
      <failure message="calc_test.cc:10&#x0A;Expected equality of these values:&#x0A;  Add(2, 2)&#x0A;    Which is: 4&#x0A;  5" type=""><![CDATA[calc_test.cc:10
Expected equality of these values:
  Add(2, 2)
    Which is: 4
  5]]></failure>
    </testcase>
    <testcase name="Throws" file="calc_test.cc" line="13" status="run" result="completed" time="0" timestamp="2026-10-19T12:00:00" classname="CalcTest">
      <failure message="unknown file&#x0A;C++ exception with description &quot;bad input&quot; thrown in the test body." type=""><![CDATA[unknown file
C++ exception with description "bad input" thrown in the test body.]]></failure>
    </testcase>
    <testcase name="DISABLED_Later" file="calc_test.cc" line="17" status="notrun" result="suppressed" time="0" timestamp="2026-10-19T12:00:00" classname="CalcTest" />
    <testcase name="Skipped" file="calc_test.cc" line="21" status="run" result="skipped" time="0" timestamp="2026-10-19T12:00:00" classname="CalcTest">
      <skipped message="calc_test.cc:22&#x0A;Skipped&#x0A;" type=""><![CDATA[calc_test.cc:22
Skipped
]]></skipped>
    </testcase>
  </testsuite>
</testsuites>
"#;

const CATCH2: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites>
  <testsuite name="calc_tests" errors="1" failures="2" skipped="1" tests="5" hostname="tbd" time="0.003" timestamp="2026-10-19T12:00:00Z">
    <testcase classname="calc_tests.global" name="adds" time="0.001"/>
    <testcase classname="calc_tests.global" name="adds wrong" time="0.001">
      <failure message="4 == 5" type="REQUIRE">
FAILED:
  REQUIRE( add(2, 2) == 5 )
with expansion:
  4 == 5
at /src/tests/calc_test.cpp:12
      </failure>
    </testcase>
    <testcase classname="calc_tests.global" name="matcher" time="0.001">
      <failure message="&quot;abc&quot; contains &quot;x&quot;" type="CHECK_THAT">
FAILED:
  CHECK_THAT( s, ContainsSubstring("x") )
at /src/tests/calc_test.cpp:16
      </failure>
    </testcase>
    <testcase classname="calc_tests.global" name="throws" time="0.001">
      <error message="bad input" type="unexpected exception">
FAILED:
  {Unknown expression after the reported line}
due to unexpected exception with message:
  bad input
at /src/tests/calc_test.cpp:19
      </error>
    </testcase>
    <testcase classname="calc_tests.global" name="later" time="0.000">
      <skipped message="not yet"/>
    </testcase>
  </testsuite>
</testsuites>
"#;

const CTEST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuite name="(empty)" tests="3" failures="1" disabled="1" skipped="1" hostname="host" time="0.10" timestamp="2026-10-19T12:00:00">
  <properties/>
  <testcase name="calc_pass" classname="calc_pass" time="0.01" status="run"/>
  <testcase name="calc_fail" classname="calc_fail" time="0.02" status="fail">
    <failure message="Failed"/>
    <system-out>1 test failed</system-out>
  </testcase>
  <testcase name="calc_off" classname="calc_off" time="0" status="notrun">
    <skipped message="Disabled"/>
  </testcase>
</testsuite>
"#;

fn lines(out: &str) -> Vec<Value> {
    out.lines().map(|l| serde_json::from_str(l).expect("every line is JSON")).collect()
}

fn test<'a>(all: &'a [Value], id: &str) -> &'a Value {
    all.iter().find(|l| l["type"] == "test" && l["id"] == id).unwrap_or_else(|| panic!("no test {id} in {all:?}"))
}

fn convert(xml: &str, runner: &str, root: Option<&str>, exit: i32) -> String {
    let ctx = Context::new(runner, "cpp");
    let ctx = match root {
        Some(r) => ctx.with_root(r),
        None => ctx,
    };
    junit::to_report_files(&[Native { name: "report.xml", text: xml, modified: after_start() }], &ctx, Some(run_started()), exit)
}

#[test]
fn googletest_names_a_place_for_an_assertion_and_none_for_an_exception_in_the_test_body() {
    let all = lines(&convert(GTEST, "gtest", None, 1));
    let wrong = test(&all, "CalcTest::AddsWrong");
    assert_eq!(wrong["outcome"], "failed");
    assert_eq!(wrong["error"]["kind"], "assertion", "the message opens with the assertion's own file:line");
    assert_eq!(wrong["error"]["frames"], serde_json::json!([{"file": "calc_test.cc", "line": 10, "function": null}]));
    assert!(wrong["error"]["message"].as_str().unwrap().contains("Expected equality"));

    let throws = test(&all, "CalcTest::Throws");
    assert_eq!(throws["error"]["kind"], "exception", "'unknown file' is no place");
    assert_eq!(throws["error"]["frames"], serde_json::json!([]));
}

#[test]
fn googletest_skips_and_disabled_tests_are_skips_never_passes() {
    let all = lines(&convert(GTEST, "gtest", None, 1));
    assert_eq!(test(&all, "CalcTest::DISABLED_Later")["outcome"], "skipped");
    let skipped = test(&all, "CalcTest::Skipped");
    assert_eq!(skipped["outcome"], "skipped");
    assert!(skipped["skip_reason"].as_str().unwrap().contains("Skipped"));
    assert_eq!(test(&all, "CalcTest::Adds")["outcome"], "passed");
    let end = all.iter().find(|l| l["type"] == "end").unwrap();
    assert_eq!((end["counts"]["passed"].as_u64(), end["counts"]["failed"].as_u64(), end["counts"]["skipped"].as_u64()), (Some(1), Some(2), Some(2)));
    assert_eq!(end["exit"], 1);
}

#[test]
fn googletest_reads_through_the_schema_as_a_failed_assertion() {
    let out = convert(GTEST, "gtest", None, 1);
    assert_eq!(read_schema(&out, 1), Reading::Read { label: "tests_failed".into(), kind: Some("assertion".into()) });
    // the same run said to exit 0 contradicts itself, and the reader says so
    assert_eq!(read_schema(&out, 0), Reading::Read { label: "unclassified".into(), kind: None });
}

#[test]
fn catch2_names_the_macro_and_the_place_it_failed_at() {
    let all = lines(&convert(CATCH2, "catch2", Some("/src"), 1));
    let wrong = test(&all, "calc_tests.global::adds wrong");
    assert_eq!(wrong["error"]["kind"], "assertion");
    assert_eq!(wrong["error"]["type"], "REQUIRE");
    assert_eq!(wrong["error"]["frames"], serde_json::json!([{"file": "tests/calc_test.cpp", "line": 12, "function": null}]), "relative to the root");
    let matcher = test(&all, "calc_tests.global::matcher");
    assert_eq!(matcher["error"]["kind"], "assertion", "CHECK_THAT is a macro too");
    let throws = test(&all, "calc_tests.global::throws");
    assert_eq!(throws["outcome"], "error");
    assert_eq!(throws["error"]["kind"], "exception");
    assert_eq!(throws["error"]["frames"][0]["line"], 19);
    assert_eq!(test(&all, "calc_tests.global::later")["outcome"], "skipped");
    assert_eq!(test(&all, "calc_tests.global::later")["skip_reason"], "not yet");
}

#[test]
fn ctest_knows_a_program_failed_and_nothing_more() {
    let all = lines(&convert(CTEST, "ctest", None, 8));
    let fail = test(&all, "calc_fail::calc_fail");
    assert_eq!(fail["outcome"], "failed");
    assert_eq!(fail["error"]["kind"], "exception", "no type and no place: it is no assertion");
    assert_eq!(fail["error"]["frames"], serde_json::json!([]));
    assert_eq!(fail["error"]["message"], "Failed");
    assert_eq!(test(&all, "calc_off::calc_off")["outcome"], "skipped");
    assert_eq!(test(&all, "calc_off::calc_off")["skip_reason"], "Disabled");
    assert_eq!(test(&all, "calc_pass::calc_pass")["outcome"], "passed");
}

#[test]
fn a_green_run_of_each_format_reads_passed() {
    let green_gtest = r#"<testsuites tests="1" failures="0"><testsuite name="S"><testcase name="A" status="run" result="completed" classname="S"/></testsuite></testsuites>"#;
    let green_catch2 = r#"<testsuites><testsuite name="t"><testcase classname="t.global" name="a" time="0.1"/></testsuite></testsuites>"#;
    let green_ctest = r#"<testsuite name="(empty)" tests="1"><testcase name="a" classname="a" status="run"/></testsuite>"#;
    for (runner, xml) in [("gtest", green_gtest), ("catch2", green_catch2), ("ctest", green_ctest)] {
        assert_eq!(read_schema(&convert(xml, runner, None, 0), 0), Reading::Read { label: "passed".into(), kind: None }, "{runner}");
    }
}

#[test]
fn a_run_where_every_test_is_skipped_or_disabled_does_not_read_passed() {
    let all_off = r#"<testsuites><testsuite name="S"><testcase name="DISABLED_A" status="notrun" result="suppressed" classname="S"/><testcase name="B" status="run" result="skipped" classname="S"><skipped/></testcase></testsuite></testsuites>"#;
    let reading = read_schema(&convert(all_off, "gtest", None, 0), 0);
    assert_ne!(reading, Reading::Read { label: "passed".into(), kind: None }, "{reading:?}");
}

#[test]
fn a_report_cut_short_is_unread_for_every_format() {
    for (runner, xml) in [("gtest", GTEST), ("catch2", CATCH2), ("ctest", CTEST)] {
        let cut = &xml[..xml.len() - 60];
        let out = convert(cut, runner, None, 1);
        assert!(lines(&out).iter().all(|l| l["type"] == "kit_error"), "{runner}: {out}");
        assert!(matches!(read_schema(&out, 1), Reading::NextReader(_)), "{runner}");
    }
}

#[test]
fn googletests_xml_is_one_file_per_executable_and_all_of_them_are_read() {
    let a = r#"<testsuites><testsuite name="A"><testcase name="x" status="run" result="completed" classname="A"/></testsuite></testsuites>"#;
    let b = r#"<testsuites><testsuite name="B"><testcase name="y" status="run" result="completed" classname="B"><failure message="b.cc:3&#x0A;boom" type=""/></testcase></testsuite></testsuites>"#;
    let files = [Native { name: "unit_a.xml", text: a, modified: after_start() }, Native { name: "unit_b.xml", text: b, modified: after_start() }];
    let out = junit::to_report_files(&files, &Context::new("gtest", "cpp"), Some(run_started()), 1);
    let all = lines(&out);
    assert_eq!(all.iter().filter(|l| l["type"] == "test").count(), 2);
    assert_eq!(read_schema(&out, 1), Reading::Read { label: "tests_failed".into(), kind: Some("assertion".into()) });
}
