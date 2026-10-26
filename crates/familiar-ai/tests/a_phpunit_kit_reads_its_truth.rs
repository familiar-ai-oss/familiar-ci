//! Lane AKreports (plan of record 2026-10-19, A section: PHPUnit/Pest's reporter). The spike's
//! extension kit (`kit/phpunit/{ext.php,prepend.php,ini/zz-familiar-ai.ini}`, loaded with
//! `PHP_INI_SCAN_DIR=":<kit>/phpunit/ini"` and the added flag `--extension "Familiar\PhpunitExtension"`)
//! writes the per-test result file itself, to the path in `FAMILIAR_AI_REPORT`. The recordings are
//! its own output on PHP 8.5.9 / PHPUnit 13.4.2, probes P1-P7 in three console setups (plain,
//! `PHPUNIT_COMPACT_OUTPUT=1`, `--testdox`); the kit is read through the schema and never wrong.
//!
//! ```text
//!   PHPUnit ──events──▶ Familiar\PhpunitExtension (a Tracer) ──▶ $FAMILIAR_AI_REPORT
//!      │                  session · test… · end after ExecutionFinished
//!      └─ P4: a missing `require` is caught by PHPUnit's Application and exits 255 with no
//!         event: the kit knows the phase, and at shutdown writes collect_error + end
//!         ("aborted_while_loading_tests"); a bare end read as "nothing ran" (a wrong reading)
//! ```
//!
//! PHPUnit never tells an extension its exit code, so the kit's end line has `exit: null` and the
//! reader uses the process's.

mod probe_runs_support;

use probe_runs_support::*;
use serde_json::Value;

const SETUPS: [&str; 3] = ["plain", "compact", "testdox"];

fn kit_output(setup: &str, probe: &str) -> (String, i32) {
    let dir = recording("phpunit", setup, probe);
    (std::fs::read_to_string(dir.join("report/results.jsonl")).expect("the kit's own output"), exit_of(&dir))
}

fn lines(out: &str) -> Vec<Value> {
    out.lines().map(|l| serde_json::from_str(l).expect("every line is JSON")).collect()
}

#[test]
fn the_kits_own_output_reads_every_probe_right_in_every_console_setup() {
    for setup in SETUPS {
        for probe in PROBES {
            let (out, exit) = kit_output(setup, probe);
            let reading = read_schema(&out, exit);
            assert_eq!(verdict(&reading, probe), Verdict::Right, "phpunit {setup} {probe}: read {reading:?}");
        }
    }
}

#[test]
fn the_report_is_the_same_in_every_console_setup() {
    for probe in PROBES {
        let plain = norm(&kit_output("plain", probe).0);
        for setup in ["compact", "testdox"] {
            assert_eq!(norm(&kit_output(setup, probe).0), plain, "{setup} {probe} differs from plain");
        }
    }
}

#[test]
fn the_kit_wrote_no_kit_error_and_names_the_runner_and_its_version() {
    for setup in SETUPS {
        for probe in PROBES {
            let (out, _) = kit_output(setup, probe);
            let all = lines(&out);
            assert!(all.iter().all(|l| l["type"] != "kit_error"), "{setup} {probe}: {out}");
            let session = all.iter().find(|l| l["type"] == "session").expect("a session line");
            assert_eq!(session["runner"], "phpunit");
            assert_eq!(session["language"], "php");
            assert_eq!(session["v"], 1);
            assert!(session["runner_version"].as_str().is_some_and(|v| !v.is_empty()), "{setup} {probe}: the runner's version is there for the drift check");
        }
    }
}

/// An end line is written only after the run finished or with the reason: a run that stopped
/// early writes no end, and an end saying nothing ran is written only when nothing ran (P6).
#[test]
fn an_end_line_never_says_nothing_ran_for_a_run_that_did_not_end_cleanly() {
    for setup in SETUPS {
        for probe in PROBES {
            let (out, _) = kit_output(setup, probe);
            let all = lines(&out);
            let ends = all.iter().filter(|l| l["type"] == "end").count();
            assert_eq!(ends, 1, "{setup} {probe}: exactly one end");
            let held = all.iter().any(|l| l["type"] == "test" || l["type"] == "collect_error");
            assert_eq!(held, probe != "P6", "{setup} {probe}: a test or a collect_error unless nothing was there to run");
        }
    }
}

#[test]
fn the_kit_cannot_know_the_exit_code_so_it_does_not_claim_one() {
    for setup in SETUPS {
        for probe in PROBES {
            let (out, _) = kit_output(setup, probe);
            let end = lines(&out).into_iter().find(|l| l["type"] == "end").unwrap();
            assert_eq!(end["exit"], Value::Null, "{setup} {probe}");
        }
    }
}

#[test]
fn a_load_failure_is_a_collect_error_with_its_reason_and_a_missing_name_is_a_test_error() {
    let (p4, _) = kit_output("plain", "P4");
    let ce = lines(&p4).into_iter().find(|l| l["type"] == "collect_error").expect("P4's collect_error");
    assert_eq!(ce["error"]["type"], "aborted_while_loading_tests");
    assert_eq!(ce["error"]["kind"], "exception", "a missing module is an exception, not a missing name");

    let (p3, _) = kit_output("plain", "P3");
    let t = lines(&p3).into_iter().find(|l| l["type"] == "test").unwrap();
    assert_eq!(t["outcome"], "error");
    assert_eq!(t["error"]["kind"], "missing_name");
    assert!(t["error"]["message"].as_str().unwrap().contains("Call to undefined method"));
}

/// What the kit's first version did (the spike's notes): an end line alone, read as "nothing
/// ran" at exit 255. The reading is wrong, which is why the kit above never writes one.
#[test]
fn a_bare_end_after_an_aborted_load_would_have_read_wrong() {
    let bare = "{\"type\":\"session\",\"v\":1,\"runner\":\"phpunit\"}\n{\"type\":\"end\",\"exit\":null,\"counts\":{\"passed\":0,\"failed\":0,\"skipped\":0,\"deselected\":0,\"errors\":0}}\n";
    assert_eq!(verdict(&read_schema(bare, 255), "P4"), Verdict::Wrong);
}
