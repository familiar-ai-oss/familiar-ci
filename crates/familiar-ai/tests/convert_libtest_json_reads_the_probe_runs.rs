//! The cargo-nextest converter (plan A, lane akgorust): `door::convert::libtest_json::to_report`
//! turns nextest's `libtest-json-plus` (read from the run's stdout) into the per-test result file
//! (JSON Lines v1, the spike's SCHEMA). The texts are the A0 spike's recordings
//! (`tests/fixtures/probe_runs/nextest/<setup>/P*`, cargo-nextest 0.9.143, rustc 1.94.0, the paths
//! shortened): every setup reads its probe's truth or cannot tell, never anything else. A build
//! failure and exit 4 (no tests to run) print no JSON, so they have no session line; an ignored test
//! is a test started with no end; no session line means "no structured output", and a run that did
//! not finish writes no `end` line.

#[path = "common/gorust_probe_runs.rs"]
mod gorust_probe_runs;

use familiar_ai::door::convert::libtest_json::to_report;
use gorust_probe_runs::{lines, of_type, read_through_schema, recorded, verdict, PROBES};
use serde_json::{json, Value};

const R: &str = "right";
const C: &str = "cant_tell";

/// Per probe in `PROBES` order (P1 P2 P3 P3b P4 P5 P6 P7): what a setup's recording reads as.
fn expected(setup: &str) -> [&'static str; 8] {
    match setup {
        // the env turns the JSON on; a build failure (P3, P4) and exit 4 (P6) print none
        "plain" | "status_none" | "no_fail_fast" => [R, R, C, R, C, R, C, R],
        // no env, the person's own --message-format human, or the junit tool config: no JSON on stdout
        "noinject" | "person_human" | "junit_toolcfg" => [C; 8],
        other => panic!("a setup with no expectation: {other}"),
    }
}

/// A nextest stream of one test binary: suite started, the events, suite ended (`ended` is the
/// suite's own end event, so a test can be left started with no end).
fn stream(events: &[Value], ended: Value) -> String {
    let suite = json!({"crate": "a", "test_binary": "t", "kind": "test"});
    let mut all = vec![json!({"type": "suite", "event": "started", "test_count": 1, "nextest": suite})];
    all.extend(events.iter().cloned());
    let mut end = json!({"type": "suite", "exec_time": 0.1, "measured": 0, "nextest": suite});
    end.as_object_mut().unwrap().extend(ended.as_object().unwrap().clone());
    all.push(end);
    all.iter().map(Value::to_string).collect::<Vec<_>>().join("\n")
}

fn one_failure(stdout: &str) -> Value {
    let all = stream(
        &[json!({"type": "test", "event": "started", "name": "a::t$one"}), json!({"type": "test", "event": "failed", "name": "a::t$one", "exec_time": 0.1, "stdout": stdout})],
        json!({"event": "failed", "passed": 0, "failed": 1, "ignored": 0, "filtered_out": 0}),
    );
    of_type(&lines(&to_report(&all, 100)), "test")[0].clone()
}

#[test]
fn every_setup_reads_its_truth_or_cannot_tell_and_never_wrong() {
    let setups = gorust_probe_runs::setups("nextest");
    assert_eq!(setups.len(), 6, "{setups:?}");
    for setup in &setups {
        for (probe, want) in PROBES.iter().zip(expected(setup)) {
            let run = recorded("nextest", setup, probe);
            let out = to_report(&run.stdout, run.exit);
            let got = read_through_schema(&out, run.exit);
            assert_eq!(verdict(&got, probe), want, "nextest/{setup}/{probe}: read {got:?}\n{out}");
        }
    }
}

#[test]
fn a_build_failure_and_exit_4_print_no_json_so_there_is_no_session_line() {
    for probe in ["P3", "P4", "P6"] {
        let run = recorded("nextest", "plain", probe);
        assert_eq!(run.stdout, "", "{probe}: nextest printed no JSON");
        assert_eq!(to_report(&run.stdout, run.exit), "", "{probe}");
    }
    assert_eq!(recorded("nextest", "plain", "P6").exit, 4);
}

#[test]
fn a_persons_own_message_format_beats_the_env_so_nothing_is_read() {
    let run = recorded("nextest", "person_human", "P1");
    assert!(run.person_cmd.contains("--message-format human"), "{}", run.person_cmd);
    assert_eq!(to_report(&run.stdout, run.exit), "");
}

#[test]
fn nothing_and_junk_and_other_runners_output_are_no_structured_output() {
    for junk in ["", "\n\n", "hello\nworld\n", "{not json\n", "[1,2,3]\n", "{\"type\":\"nope\"}\n", "{\"type\":\"test\",\"event\":\"weird\",\"name\":\"a$b\"}\n", "{\"foo\":1}\n"] {
        assert_eq!(to_report(junk, 0), "", "{junk:?}");
        assert_eq!(to_report(junk, 100), "", "{junk:?}");
    }
    // go's test2json is not libtest's
    let go = recorded("go-test", "plain", "P1").stdout;
    assert_eq!(to_report(&go, 0), "");
    // cargo test's own console has no JSON in it
    for probe in PROBES {
        let run = recorded("cargo-test", "plain", probe);
        assert_eq!(to_report(&run.stdout, run.exit), "", "cargo-test/{probe}");
    }
}

#[test]
fn a_pass_reads_passed_with_its_counts() {
    let run = recorded("nextest", "plain", "P1");
    let all = lines(&to_report(&run.stdout, run.exit));
    assert_eq!(all[0]["type"], "session");
    assert_eq!((all[0]["v"].clone(), all[0]["runner"].clone(), all[0]["language"].clone()), (json!(1), json!("cargo-nextest"), json!("rust")));
    let tests = of_type(&all, "test");
    assert_eq!(tests.len(), 1);
    assert_eq!((tests[0]["id"].as_str(), tests[0]["outcome"].as_str()), (Some("probe::familiar_probe_p1$adds"), Some("passed")));
    assert_eq!(tests[0]["error"], Value::Null);
    let end = all.last().unwrap();
    assert_eq!((end["type"].as_str(), end["exit"].as_i64()), (Some("end"), Some(0)));
    assert_eq!(end["counts"], json!({"passed": 1, "failed": 0, "skipped": 0, "deselected": 0, "errors": 0}));
}

#[test]
fn an_assert_eq_is_an_assertion_at_the_panic_site_with_both_values() {
    let run = recorded("nextest", "plain", "P2");
    assert_eq!(run.exit, 100);
    let all = lines(&to_report(&run.stdout, run.exit));
    let t = of_type(&all, "test")[0].clone();
    assert_eq!(t["outcome"], "failed");
    assert_eq!(t["file"], "tests/familiar_probe_p2.rs", "the panic is in the test binary's own file");
    assert_eq!((t["error"]["type"].as_str(), t["error"]["kind"].as_str()), (Some("panic"), Some("assertion")));
    assert_eq!(t["error"]["message"], "assertion `left == right` failed\n  left: 4\n right: 5");
    assert_eq!(t["error"]["frames"], json!([{"file": "tests/familiar_probe_p2.rs", "line": 3, "function": null}]));
    let end = all.last().unwrap();
    assert_eq!((end["exit"].as_i64(), end["counts"]["failed"].as_u64()), (Some(100), Some(1)));
}

#[test]
fn a_panic_in_the_code_is_an_exception_at_its_site_not_in_the_test_file() {
    let run = recorded("nextest", "plain", "P7");
    let all = lines(&to_report(&run.stdout, run.exit));
    let t = of_type(&all, "test")[0].clone();
    assert_eq!(t["error"]["kind"], "exception");
    assert_eq!(t["error"]["message"], "not a number: x1");
    assert_eq!(t["error"]["frames"], json!([{"file": "src/lib.rs", "line": 8, "function": null}]));
    assert_eq!(t["file"], Value::Null, "the panic is not in the test's own file");
}

#[test]
fn an_ignored_test_is_started_with_no_end_and_counts_as_skipped() {
    for setup in ["plain", "status_none", "no_fail_fast"] {
        let run = recorded("nextest", setup, "P5");
        let all = lines(&to_report(&run.stdout, run.exit));
        let tests = of_type(&all, "test");
        let by = |name: &str| tests.iter().find(|t| t["id"] == format!("probe::familiar_probe_p5${name}")).unwrap_or_else(|| panic!("{setup}: no {name}"));
        assert_eq!(by("adds")["outcome"], "passed");
        assert_eq!(by("later")["outcome"], "skipped");
        assert_eq!(by("later")["skip_reason"], Value::Null, "nextest does not carry #[ignore = \"..\"]'s reason");
        assert_eq!(all.last().unwrap()["counts"], json!({"passed": 1, "failed": 0, "skipped": 1, "deselected": 0, "errors": 0}), "{setup}");
    }
}

#[test]
fn a_filtered_out_test_is_a_deselected_one() {
    let all = stream(
        &[json!({"type": "test", "event": "started", "name": "a::t$one"}), json!({"type": "test", "event": "ok", "name": "a::t$one", "exec_time": 0.1})],
        json!({"event": "ok", "passed": 1, "failed": 0, "ignored": 0, "filtered_out": 3}),
    );
    let all = lines(&to_report(&all, 0));
    assert_eq!(all.last().unwrap()["counts"], json!({"passed": 1, "failed": 0, "skipped": 0, "deselected": 3, "errors": 0}));
}

#[test]
fn the_exit_code_is_carried_into_the_end_line() {
    for probe in ["P1", "P2", "P3b", "P5", "P7"] {
        let run = recorded("nextest", "plain", probe);
        let all = lines(&to_report(&run.stdout, run.exit));
        assert_eq!(all.last().unwrap()["exit"], run.exit, "{probe}");
    }
}

#[test]
fn a_result_that_contradicts_the_exit_code_reads_unclassified() {
    let pass = recorded("nextest", "plain", "P1");
    assert_eq!(read_through_schema(&to_report(&pass.stdout, 100), 100).label, "unclassified", "a pass with exit 100");
    let fail = recorded("nextest", "plain", "P2");
    assert_eq!(read_through_schema(&to_report(&fail.stdout, 0), 0).label, "unclassified", "a failure with exit 0");
}

#[test]
fn a_stream_that_was_cut_writes_no_end_line() {
    let run = recorded("nextest", "plain", "P1");
    let mut cut: Vec<&str> = run.stdout.lines().collect();
    cut.pop(); // the suite's own end: the run did not finish
    let out = to_report(&cut.join("\n"), run.exit);
    assert!(of_type(&lines(&out), "end").is_empty(), "{out}");
    assert_eq!(read_through_schema(&out, run.exit).label, "next_reader");

    // cut in the middle of the last line
    let half = &run.stdout[..run.stdout.len() - 40];
    let out = to_report(half, run.exit);
    assert!(of_type(&lines(&out), "end").is_empty(), "{out}");
    assert_eq!(read_through_schema(&out, run.exit).label, "next_reader");
}

#[test]
fn a_test_that_started_and_never_ended_without_being_ignored_is_not_a_skip() {
    // the suite says nothing was ignored, yet a test has no verdict: it died, the run is not read
    let all = stream(&[json!({"type": "test", "event": "started", "name": "a::t$one"})], json!({"event": "ok", "passed": 0, "failed": 0, "ignored": 0, "filtered_out": 0}));
    let out = to_report(&all, 0);
    assert!(of_type(&lines(&out), "end").is_empty(), "{out}");
    assert_eq!(read_through_schema(&out, 0).label, "next_reader");
}

#[test]
fn rust_before_1_73_prints_the_panic_message_in_quotes_before_the_place() {
    let t = one_failure("\nthread 'one' panicked at 'assertion failed: `(left == right)`\n  left: `4`,\n right: `5`', tests/t.rs:3:5\nnote: run with `RUST_BACKTRACE=1` environment variable to display a backtrace\n");
    assert_eq!(t["error"]["kind"], "assertion");
    assert_eq!(t["error"]["frames"], json!([{"file": "tests/t.rs", "line": 3, "function": null}]));
    assert_eq!(t["file"], "tests/t.rs");
}

#[test]
fn a_custom_message_or_an_unwrap_is_an_exception_and_a_failure_with_no_panic_has_no_frames() {
    let custom = one_failure("\nthread 'one' (1) panicked at tests/t.rs:4:5:\nthe sum is wrong\n");
    assert_eq!((custom["error"]["kind"].as_str(), custom["error"]["message"].as_str()), (Some("exception"), Some("the sum is wrong")), "assert!(x, \"custom\") panics with the custom text");
    let unwrap = one_failure("\nthread 'one' (1) panicked at tests/t.rs:4:5:\ncalled `Option::unwrap()` on a `None` value\n");
    assert_eq!(unwrap["error"]["kind"], "exception");
    let nothing = one_failure("the process was killed by signal 11\n");
    assert_eq!((nothing["error"]["kind"].as_str(), nothing["error"]["frames"].clone()), (Some("exception"), json!([])));
}

#[test]
fn the_recorded_injection_is_two_env_vars_and_the_persons_command_is_untouched() {
    let plain = recorded("nextest", "plain", "P1");
    assert_eq!(plain.injection["env"], json!([["set", "NEXTEST_EXPERIMENTAL_LIBTEST_JSON", "1"], ["set", "NEXTEST_MESSAGE_FORMAT", "libtest-json-plus"]]));
    assert_eq!(plain.person_cmd, "cargo nextest run");
    let none = recorded("nextest", "status_none", "P1");
    assert_eq!(none.person_cmd, "cargo nextest run --status-level none --final-status-level none");
    assert_eq!(none.injection["env"], plain.injection["env"]);
}

#[test]
fn a_run_killed_by_a_signal_writes_no_end_line() {
    // a complete-looking stream from a process that died by a signal (128 + n, or negative) did not finish
    let run = recorded("nextest", "plain", "P1");
    for exit in [137, 143, 130, 128, -9, -15] {
        let out = to_report(&run.stdout, exit);
        assert!(!out.is_empty() && of_type(&lines(&out), "end").is_empty(), "exit {exit}: {out}");
        assert_eq!(read_through_schema(&out, exit).label, "next_reader", "exit {exit}");
    }
    let normal = to_report(&run.stdout, 0);
    assert!(!of_type(&lines(&normal), "end").is_empty(), "a normal exit keeps its end line");
}

#[test]
fn a_stream_with_the_familiar_cut_marker_in_it_is_not_a_complete_run() {
    // a cut log (kept::cut) has its middle dropped, so the stream can look complete and miss tests
    let run = recorded("nextest", "plain", "P5");
    let mut all: Vec<&str> = run.stdout.lines().collect();
    all.insert(2, "[familiar: 1234 chars dropped]");
    let out = to_report(&all.join("\n"), run.exit);
    assert!(!out.is_empty() && of_type(&lines(&out), "end").is_empty(), "{out}");
    assert_eq!(read_through_schema(&out, run.exit).label, "next_reader");
}

#[test]
fn a_cut_anywhere_or_a_stream_started_anywhere_never_panics_and_is_well_formed() {
    for probe in PROBES {
        let run = recorded("nextest", "plain", probe);
        let text = &run.stdout;
        let at: Vec<usize> = text.char_indices().map(|(i, _)| i).filter(|&i| i % 11 == 0 || text[..i].ends_with('\n')).chain([text.len()]).collect();
        for &i in &at {
            for part in [&text[..i], &text[i..]] {
                let out = to_report(part, run.exit);
                if !out.is_empty() {
                    lines(&out); // every line is JSON with a type
                }
            }
        }
    }
}
