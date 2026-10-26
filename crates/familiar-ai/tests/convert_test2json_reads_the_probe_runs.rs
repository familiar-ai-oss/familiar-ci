//! The go test converter (plan A, lane akgorust): `door::convert::test2json::to_report` turns
//! `go test -json` (test2json, read from the run's stdout) into the per-test result file (JSON
//! Lines v1, the spike's SCHEMA). The texts are the A0 spike's recordings
//! (`tests/fixtures/probe_runs/go-test/<setup>/P*`, go 1.23.12 and 1.25.14, the paths shortened):
//! every injected setup reads its probe's truth or cannot tell, never anything else; no session
//! line means "no structured output" (go up to 1.23 prints a setup failure as plain text, and a
//! build failure's reason on stderr), and a run that did not finish writes no `end` line.

#[path = "common/gorust_probe_runs.rs"]
mod gorust_probe_runs;

use familiar_ai::door::convert::test2json::to_report;
use gorust_probe_runs::{lines, of_type, read_through_schema, recorded, verdict, PROBES};
use serde_json::{json, Value};

const R: &str = "right";
const C: &str = "cant_tell";

/// Per probe in `PROBES` order (P1 P2 P3 P3b P4 P5 P6 P7): what a setup's recording reads as.
fn expected(setup: &str) -> [&'static str; 8] {
    match setup {
        // go 1.24+ prints a build failure's words as build events inside the JSON
        "go125" | "go125_verbose" => [R; 8],
        // go 1.23: the compiler's words are on stderr (P3) and a setup failure is plain text
        // with no JSON at all (P4): stdout alone cannot tell why
        "plain" | "goflags_preset" | "person_json" | "verbose" => [R, R, C, R, C, R, R, R],
        // the console of a run with no injection is plain text, not test2json
        "noinject" => [C; 8],
        other => panic!("a setup with no expectation: {other}"),
    }
}

#[test]
fn every_setup_reads_its_truth_or_cannot_tell_and_never_wrong() {
    let setups = gorust_probe_runs::setups("go-test");
    assert_eq!(setups.len(), 7, "{setups:?}");
    for setup in &setups {
        for (probe, want) in PROBES.iter().zip(expected(setup)) {
            let run = recorded("go-test", setup, probe);
            let out = to_report(&run.stdout, run.exit);
            let got = read_through_schema(&out, run.exit);
            assert_eq!(verdict(&got, probe), want, "go-test/{setup}/{probe}: read {got:?}\n{out}");
        }
    }
}

#[test]
fn a_setup_failure_before_go_1_24_is_plain_text_so_there_is_no_session_line() {
    let run = recorded("go-test", "plain", "P4");
    assert!(run.stdout.starts_with("FAIL\texample.com/probe [setup failed]"), "{}", run.stdout);
    assert_eq!(to_report(&run.stdout, run.exit), "", "no JSON line seen, no structured output");
}

#[test]
fn a_build_failure_whose_reason_is_not_shown_is_not_read() {
    // go 1.23: only "FAIL pkg [build failed]" is JSON, the compiler's lines went to stderr
    let run = recorded("go-test", "plain", "P3");
    assert!(run.stdout.contains("[build failed]") && !run.stdout.contains("undefined"), "{}", run.stdout);
    assert_eq!(to_report(&run.stdout, run.exit), "", "without the reason it could be a missing name or a broken test");
}

#[test]
fn the_console_of_an_uninjected_run_is_not_structured_output() {
    for probe in PROBES {
        let run = recorded("go-test", "noinject", probe);
        assert_eq!(to_report(&run.stdout, run.exit), "", "noinject/{probe}");
    }
}

#[test]
fn nothing_and_junk_and_other_runners_output_are_no_structured_output() {
    for junk in ["", "\n\n", "hello\nworld\n", "{not json\n", "[1,2,3]\n", "{\"Action\":\"nope\"}\n", "{\"foo\":1}\n"] {
        assert_eq!(to_report(junk, 0), "", "{junk:?}");
        assert_eq!(to_report(junk, 1), "", "{junk:?}");
    }
    // go's own plain console (no injection), and nextest's JSON, are not test2json
    let cargo = recorded("go-test", "noinject", "P2").stdout;
    assert_eq!(to_report(&cargo, 1), "");
    let nextest = r#"{"type":"suite","event":"started","test_count":1}"#;
    assert_eq!(to_report(nextest, 0), "");
}

#[test]
fn a_pass_reads_passed_with_its_counts() {
    let run = recorded("go-test", "plain", "P1");
    let out = to_report(&run.stdout, run.exit);
    let all = lines(&out);
    assert_eq!(all[0]["type"], "session");
    assert_eq!((all[0]["v"].clone(), all[0]["runner"].clone(), all[0]["language"].clone()), (json!(1), json!("go-test"), json!("go")));
    let tests = of_type(&all, "test");
    assert_eq!(tests.len(), 1);
    assert_eq!((tests[0]["id"].as_str(), tests[0]["outcome"].as_str()), (Some("example.com/probe.TestAdd"), Some("passed")));
    assert_eq!(tests[0]["error"], Value::Null);
    let end = all.last().unwrap();
    assert_eq!(end["type"], "end");
    assert_eq!(end["exit"], 0);
    assert_eq!(end["counts"], json!({"passed": 1, "failed": 0, "skipped": 0, "deselected": 0, "errors": 0}));
}

#[test]
fn an_assertion_is_the_t_error_line_in_the_test_file() {
    let run = recorded("go-test", "plain", "P2");
    let all = lines(&to_report(&run.stdout, run.exit));
    let t = of_type(&all, "test")[0].clone();
    assert_eq!(t["outcome"], "failed");
    assert_eq!(t["file"], "calc_test.go");
    assert_eq!(t["error"]["kind"], "assertion");
    assert_eq!(t["error"]["message"], "Add(2, 2) = 4, want 5");
    assert_eq!(t["error"]["frames"], json!([{"file": "calc_test.go", "line": 7, "function": "TestAdd"}]));
    assert_eq!(all.last().unwrap()["counts"]["failed"], 1);
    assert_eq!(all.last().unwrap()["exit"], 1);
}

#[test]
fn a_panic_is_an_exception_with_the_project_frames_outermost_first() {
    let run = recorded("go-test", "plain", "P7");
    let all = lines(&to_report(&run.stdout, run.exit));
    let t = of_type(&all, "test")[0].clone();
    assert_eq!(t["outcome"], "failed");
    assert_eq!(t["error"]["kind"], "exception");
    assert_eq!(t["error"]["type"], "panic");
    assert!(t["error"]["message"].as_str().unwrap().starts_with("not a number: x1"), "{t}");
    let frames = t["error"]["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 2, "the testing and runtime frames are not the project's: {frames:?}");
    let at = |i: usize| (frames[i]["file"].as_str().unwrap().rsplit('/').next().unwrap().to_string(), frames[i]["line"].as_u64().unwrap(), frames[i]["function"].as_str().unwrap().to_string());
    assert_eq!(at(0), ("calc_test.go".into(), 6, "example.com/probe.TestParse".into()), "outermost first");
    assert_eq!(at(1), ("calc.go".into(), 10, "example.com/probe.Parse".into()), "the deepest project frame last");
    assert!(t["file"].as_str().unwrap().ends_with("calc_test.go"), "the test's own file: {t}");
}

#[test]
fn a_skip_beside_a_pass_keeps_its_reason_and_its_count() {
    for setup in ["plain", "verbose", "go125"] {
        let run = recorded("go-test", setup, "P5");
        let all = lines(&to_report(&run.stdout, run.exit));
        let tests = of_type(&all, "test");
        let by = |name: &str| tests.iter().find(|t| t["id"] == format!("example.com/probe.{name}")).unwrap_or_else(|| panic!("{setup}: no {name}"));
        assert_eq!(by("TestAdd")["outcome"], "passed");
        assert_eq!(by("TestLater")["outcome"], "skipped");
        assert_eq!(by("TestLater")["skip_reason"], "not yet");
        assert_eq!(all.last().unwrap()["counts"], json!({"passed": 1, "failed": 0, "skipped": 1, "deselected": 0, "errors": 0}), "{setup}");
    }
}

#[test]
fn a_run_that_selected_no_test_has_a_session_and_an_end_with_zero_counts() {
    let run = recorded("go-test", "plain", "P6");
    let all = lines(&to_report(&run.stdout, run.exit));
    assert!(of_type(&all, "test").is_empty());
    assert_eq!(all.last().unwrap()["counts"], json!({"passed": 0, "failed": 0, "skipped": 0, "deselected": 0, "errors": 0}));
}

#[test]
fn go_1_25_build_events_give_a_missing_name_and_a_broken_import() {
    let run = recorded("go-test", "go125", "P3");
    let all = lines(&to_report(&run.stdout, run.exit));
    let ces = of_type(&all, "collect_error");
    assert_eq!(ces.len(), 1, "{all:?}");
    assert_eq!(ces[0]["file"], "calc_test.go");
    assert_eq!(ces[0]["error"]["kind"], "missing_name");
    assert!(ces[0]["error"]["message"].as_str().unwrap().contains("undefined: Mul"), "{}", ces[0]);
    assert_eq!(ces[0]["error"]["frames"], json!([{"file": "calc_test.go", "line": 6, "function": null}]));
    assert_eq!(all.last().unwrap()["counts"]["errors"], 1);

    let run = recorded("go-test", "go125", "P4");
    let all = lines(&to_report(&run.stdout, run.exit));
    let ces = of_type(&all, "collect_error");
    assert_eq!(ces.len(), 1, "{all:?}");
    assert_eq!(ces[0]["error"]["kind"], "exception", "a missing module is not a missing name");
    assert!(ces[0]["error"]["message"].as_str().unwrap().contains("no required module provides package example.com/nosuch"), "{}", ces[0]);
    assert_eq!(ces[0]["error"]["frames"][0]["file"], "calc_test.go");
}

#[test]
fn the_exit_code_is_carried_into_the_end_line() {
    for setup in ["plain", "go125"] {
        for probe in PROBES {
            let run = recorded("go-test", setup, probe);
            let out = to_report(&run.stdout, run.exit);
            if out.is_empty() {
                continue;
            }
            let all = lines(&out);
            assert_eq!(all.last().unwrap()["exit"], run.exit, "{setup}/{probe}");
        }
    }
}

#[test]
fn a_result_that_contradicts_the_exit_code_reads_unclassified() {
    let pass = recorded("go-test", "plain", "P1");
    assert_eq!(read_through_schema(&to_report(&pass.stdout, 1), 1).label, "unclassified", "a pass with exit 1");
    let fail = recorded("go-test", "plain", "P2");
    assert_eq!(read_through_schema(&to_report(&fail.stdout, 0), 0).label, "unclassified", "a failure with exit 0");
}

#[test]
fn a_stream_that_was_cut_writes_no_end_line() {
    let run = recorded("go-test", "plain", "P1");
    let mut cut: Vec<&str> = run.stdout.lines().collect();
    cut.pop(); // the package's own pass event: the run did not finish
    let out = to_report(&cut.join("\n"), run.exit);
    assert!(of_type(&lines(&out), "end").is_empty(), "{out}");
    assert_eq!(read_through_schema(&out, run.exit).label, "next_reader");

    // cut in the middle of a line: the half line is not an event, the package never ended
    let half = &run.stdout[..run.stdout.len() - 25];
    let out = to_report(half, run.exit);
    assert!(of_type(&lines(&out), "end").is_empty(), "{out}");
    assert_eq!(read_through_schema(&out, run.exit).label, "next_reader");
}

#[test]
fn a_package_that_failed_with_no_failing_test_is_not_read() {
    // a TestMain that exits 1, an init panic, a timeout: no test says why, and it is not a build failure
    let stream = [
        json!({"Action": "start", "Package": "p"}),
        json!({"Action": "output", "Package": "p", "Output": "panic: boom\n"}),
        json!({"Action": "output", "Package": "p", "Output": "FAIL\tp\t0.01s\n"}),
        json!({"Action": "fail", "Package": "p", "Elapsed": 0.01}),
    ]
    .iter()
    .map(Value::to_string)
    .collect::<Vec<_>>()
    .join("\n");
    assert_eq!(to_report(&stream, 1), "");
}

#[test]
fn a_test_with_no_verdict_in_a_failed_package_died_with_the_binary() {
    let ev = |v: Value| v.to_string();
    let stream = [
        ev(json!({"Action": "start", "Package": "p"})),
        ev(json!({"Action": "run", "Package": "p", "Test": "TestA"})),
        ev(json!({"Action": "output", "Package": "p", "Test": "TestA", "Output": "panic: runtime error: index out of range [3] with length 3\n"})),
        ev(json!({"Action": "output", "Package": "p", "Test": "TestA", "Output": "\n"})),
        ev(json!({"Action": "output", "Package": "p", "Test": "TestA", "Output": "goroutine 7 [running]:\n"})),
        ev(json!({"Action": "output", "Package": "p", "Test": "TestA", "Output": "p.TestA(0xc000)\n"})),
        ev(json!({"Action": "output", "Package": "p", "Test": "TestA", "Output": "\t/w/a_test.go:9 +0x1d\n"})),
        ev(json!({"Action": "output", "Package": "p", "Output": "FAIL\tp\t0.01s\n"})),
        ev(json!({"Action": "fail", "Package": "p", "Elapsed": 0.01})),
    ]
    .join("\n");
    let all = lines(&to_report(&stream, 2));
    let t = of_type(&all, "test")[0].clone();
    assert_eq!((t["outcome"].as_str(), t["error"]["kind"].as_str(), t["error"]["type"].as_str()), (Some("failed"), Some("exception"), Some("panic")));
    assert_eq!(t["error"]["frames"], json!([{"file": "/w/a_test.go", "line": 9, "function": "p.TestA"}]));
    assert_eq!(all.last().unwrap()["counts"]["failed"], 1);
}

#[test]
fn a_failing_test_with_no_location_and_no_panic_is_an_exception_with_no_frames() {
    // os.Exit(1) from a helper, a t.FailNow with no message: nothing says where
    let ev = |v: Value| v.to_string();
    let stream = [
        ev(json!({"Action": "run", "Package": "p", "Test": "TestA"})),
        ev(json!({"Action": "output", "Package": "p", "Test": "TestA", "Output": "--- FAIL: TestA (0.00s)\n"})),
        ev(json!({"Action": "fail", "Package": "p", "Test": "TestA", "Elapsed": 0})),
        ev(json!({"Action": "fail", "Package": "p", "Elapsed": 0})),
    ]
    .join("\n");
    let all = lines(&to_report(&stream, 1));
    let t = of_type(&all, "test")[0].clone();
    assert_eq!(t["error"]["kind"], "exception");
    assert_eq!(t["error"]["frames"], json!([]));
}

#[test]
fn a_message_that_says_panic_inside_a_t_error_is_not_a_panic() {
    let ev = |v: Value| v.to_string();
    let stream = [
        ev(json!({"Action": "run", "Package": "p", "Test": "TestA"})),
        ev(json!({"Action": "output", "Package": "p", "Test": "TestA", "Output": "    a_test.go:5: got panic: x, want ok\n"})),
        ev(json!({"Action": "output", "Package": "p", "Test": "TestA", "Output": "--- FAIL: TestA (0.00s)\n"})),
        ev(json!({"Action": "fail", "Package": "p", "Test": "TestA", "Elapsed": 0})),
        ev(json!({"Action": "fail", "Package": "p", "Elapsed": 0})),
    ]
    .join("\n");
    let all = lines(&to_report(&stream, 1));
    let t = of_type(&all, "test")[0].clone();
    assert_eq!(t["error"]["kind"], "assertion");
    assert_eq!(t["error"]["frames"], json!([{"file": "a_test.go", "line": 5, "function": "TestA"}]));
}

#[test]
fn the_recorded_injection_is_goflags_json_appended_to_the_persons_own() {
    // what the kit's reader spec has to say (kits::go_test): GOFLAGS gets " -json" appended
    let plain = recorded("go-test", "plain", "P1");
    assert_eq!(plain.injection["env"], json!([["append", "GOFLAGS", "-json"]]));
    assert_eq!(plain.person_cmd, "go test ./...", "the person's command is untouched");
    let preset = recorded("go-test", "goflags_preset", "P1");
    assert_eq!(preset.injection["env"], json!([["append", "GOFLAGS", "-json"]]));
}

#[test]
fn a_run_killed_by_a_signal_writes_no_end_line() {
    // a complete-looking stream from a process that died by a signal (128 + n, or negative) did not finish
    let run = recorded("go-test", "plain", "P1");
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
    let run = recorded("go-test", "plain", "P5");
    let mut all: Vec<&str> = run.stdout.lines().collect();
    all.insert(2, "[familiar: 1234 chars dropped]");
    let out = to_report(&all.join("\n"), run.exit);
    assert!(!out.is_empty() && of_type(&lines(&out), "end").is_empty(), "{out}");
    assert_eq!(read_through_schema(&out, run.exit).label, "next_reader");
}

#[test]
fn a_cut_anywhere_or_a_stream_started_anywhere_never_panics_and_is_well_formed() {
    for probe in PROBES {
        let run = recorded("go-test", "plain", probe);
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

/// A failing test's output as events, then its own and its package's fail.
fn failing(output: &[&str]) -> String {
    let mut all = vec![json!({"Action": "run", "Package": "p", "Test": "TestA"}).to_string()];
    all.extend(output.iter().map(|s| json!({"Action": "output", "Package": "p", "Test": "TestA", "Output": s}).to_string()));
    all.push(json!({"Action": "fail", "Package": "p", "Test": "TestA", "Elapsed": 0}).to_string());
    all.push(json!({"Action": "fail", "Package": "p", "Elapsed": 0}).to_string());
    all.join("\n")
}

#[test]
fn only_the_panicking_goroutines_frames_are_the_panics() {
    // a panic's dump lists the goroutine that panicked first, then every other: theirs are not its own
    let stream = failing(&[
        "panic: boom\n",
        "\n",
        "goroutine 7 [running]:\n",
        "p.TestA(0xc000)\n",
        "\t/w/a_test.go:9 +0x1d\n",
        "\n",
        "goroutine 1 [chan receive]:\n",
        "p.worker(0xc001)\n",
        "\t/w/worker.go:30 +0x44\n",
    ]);
    let all = lines(&to_report(&stream, 2));
    let t = of_type(&all, "test")[0].clone();
    assert_eq!(t["error"]["frames"], json!([{"file": "/w/a_test.go", "line": 9, "function": "p.TestA"}]), "not worker.go");
}

#[test]
fn a_test_timeout_has_no_project_frame_for_the_test() {
    // the first goroutine dumped is the testing package's alarm, not the test that was running
    let stream = failing(&[
        "panic: test timed out after 1s\n",
        "\trunning tests:\n",
        "\t\tTestA (1s)\n",
        "\n",
        "goroutine 7 [running]:\n",
        "testing.(*M).startAlarm.func1()\n",
        "\t/usr/local/go/src/testing/testing.go:2373 +0x385\n",
        "created by time.goFunc\n",
        "\t/usr/local/go/src/time/sleep.go:215 +0x2d\n",
        "\n",
        "goroutine 6 [sleep]:\n",
        "p.TestA(0xc000)\n",
        "\t/w/a_test.go:9 +0x1d\n",
    ]);
    let all = lines(&to_report(&stream, 2));
    let t = of_type(&all, "test")[0].clone();
    assert_eq!((t["error"]["kind"].as_str(), t["error"]["message"].as_str()), (Some("exception"), Some("test timed out after 1s")));
    assert_eq!(t["error"]["frames"], json!([]), "nothing says where it hung");
}
