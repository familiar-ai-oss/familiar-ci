//! The kit data of go test, cargo-nextest and cargo test (plan A, lane akgorust): per runner, plain
//! data in `door::kits::<runner>` for the reader table: the reader specs (id, kind, env ops, added
//! flags, report globs), the converter and the stream it reads, the selection flags re-aim drops, the
//! watched setup files, and the probe kit P1..P7 (P3b is stage 2's "the function now exists") with
//! its truths, in the shape the other runners' modules have (a probe is (id, truth label, truth kind,
//! files), the first file the test the command names). The data is checked against what the A0 spike recorded: the injections that read right
//! and the truths the probes were judged by.

#[path = "common/gorust_probe_runs.rs"]
mod gorust_probe_runs;

use familiar_ai::door::kits::{cargo_nextest, cargo_test, go_test};
use gorust_probe_runs::{recorded, truth, PROBES};
use serde_json::{json, Value};

/// The spike's `injection.json` form of an env op list: `["append", "GOFLAGS", "-json"]`.
fn as_recorded(env: &[(&str, &str, &str, &str)]) -> Value {
    Value::Array(env.iter().map(|(var, op, value, _sep)| json!([op, var, value])).collect())
}

#[test]
fn go_test_has_the_json_reporter_in_two_routes_then_the_tables() {
    let ids: Vec<(&str, &str)> = go_test::READERS.iter().map(|r| (r.0, r.1)).collect();
    assert_eq!(ids, [("go-test:reporter", "reporter"), ("go-test:reporter-flag", "reporter"), ("go-test:tables", "tables")], "best first");
    assert_eq!(go_test::RUNNER, "go-test");
    assert_eq!((go_test::CONVERTER, go_test::NATIVE_STREAM), ("test2json", "stdout"));
    // the env route: GOFLAGS gets " -json" appended to the person's own, when go test is the only go call
    let env = go_test::READERS[0];
    assert_eq!(env.2, [("GOFLAGS", "append", "-json", " ")]);
    assert!(env.3.is_empty() && env.4.is_empty(), "no flag, no report file");
    // the flag route: -json goes into the go test part, the env untouched
    let flag = go_test::READERS[1];
    assert!(flag.2.is_empty());
    assert_eq!(flag.3, ["-json"]);
    let tables = go_test::READERS[2];
    assert!(tables.2.is_empty() && tables.3.is_empty() && tables.4.is_empty(), "the tables add nothing");
    // the note says why there are two routes (the spike's finding)
    for word in ["go vet", "go list", "only go call"] {
        assert!(go_test::NOTE.contains(word), "{word}: {}", go_test::NOTE);
    }
    assert!(go_test::NOTE.contains("1.23") && go_test::NOTE.contains("plain text"), "{}", go_test::NOTE);
}

#[test]
fn the_go_env_op_is_what_the_recorded_runs_injected() {
    for setup in ["plain", "goflags_preset", "verbose", "go125"] {
        let run = recorded("go-test", setup, "P1");
        assert_eq!(run.injection["env"], as_recorded(go_test::READERS[0].2), "{setup}");
    }
}

#[test]
fn cargo_nextest_has_the_two_env_vars_then_the_tables_and_skips_a_persons_message_format() {
    let ids: Vec<(&str, &str)> = cargo_nextest::READERS.iter().map(|r| (r.0, r.1)).collect();
    assert_eq!(ids, [("cargo-nextest:report", "report"), ("cargo-nextest:tables", "tables")], "best first");
    assert_eq!(cargo_nextest::RUNNER, "cargo-nextest");
    assert_eq!((cargo_nextest::CONVERTER, cargo_nextest::NATIVE_STREAM), ("libtest_json", "stdout"));
    let report = cargo_nextest::READERS[0];
    assert_eq!(report.2, [("NEXTEST_EXPERIMENTAL_LIBTEST_JSON", "set", "1", ""), ("NEXTEST_MESSAGE_FORMAT", "set", "libtest-json-plus", "")]);
    assert!(report.3.is_empty() && report.4.is_empty(), "no flag, and the report is the run's stdout");
    // what the recordings injected, both variables, the experimental one alone prints nothing
    for setup in ["plain", "status_none", "no_fail_fast"] {
        assert_eq!(recorded("nextest", setup, "P1").injection["env"], as_recorded(report.2), "{setup}");
    }
    // the person's own --message-format beats the env: the reader is skipped, the recording says so
    assert_eq!(cargo_nextest::SKIP_WHEN, [("cargo-nextest:report", &["--message-format"][..])]);
    assert!(recorded("nextest", "person_human", "P1").person_cmd.contains(cargo_nextest::SKIP_WHEN[0].1[0]));
    for word in ["NEXTEST_MESSAGE_FORMAT", "build failure", "exit 4", "no session line"] {
        assert!(cargo_nextest::NOTE.contains(word), "{word}: {}", cargo_nextest::NOTE);
    }
}

#[test]
fn cargo_test_is_tables_only_and_says_so() {
    assert_eq!(cargo_test::RUNNER, "cargo-test");
    let ids: Vec<(&str, &str)> = cargo_test::READERS.iter().map(|r| (r.0, r.1)).collect();
    assert_eq!(ids, [("cargo-test:tables", "tables")]);
    assert_eq!((cargo_test::CONVERTER, cargo_test::NATIVE_STREAM), ("", ""), "no converter, nothing to read but the console");
    assert!(cargo_test::NOTE.contains("tables only"), "{}", cargo_test::NOTE);
    // the recording: no reporter, nothing injected
    assert_eq!(recorded("cargo-test", "plain", "P1").injection["env"], json!([]));
}

fn names<'a>(list: &[(&'a str, bool)]) -> Vec<&'a str> {
    list.iter().map(|f| f.0).collect()
}

#[test]
fn the_selection_flags_re_aim_drops_are_named_with_whether_they_take_a_value() {
    assert_eq!(go_test::SELECTION_FLAGS, [("-run", true), ("--run", true), ("-skip", true), ("--skip", true)]);
    // cargo's own target selection, then what the person's filters are (positional names are dropped by re-aim itself)
    for (flag, takes) in [("--test", true), ("--lib", false), ("--bin", true), ("--bins", false), ("--example", true), ("--examples", false), ("--tests", false), ("--bench", true), ("--benches", false), ("--all-targets", false), ("--doc", false)] {
        assert!(cargo_test::SELECTION_FLAGS.contains(&(flag, takes)), "cargo test: {flag}");
    }
    // libtest's own, after `--`
    for (flag, takes) in [("--skip", true), ("--exact", false), ("--ignored", false), ("--include-ignored", false)] {
        assert!(cargo_test::AFTER_DOUBLE_DASH.contains(&(flag, takes)), "libtest: {flag}");
    }
    for (flag, takes) in [("-E", true), ("--filter-expr", true), ("--run-ignored", true), ("--partition", true), ("--test", true), ("--lib", false), ("--all-targets", false)] {
        assert!(cargo_nextest::SELECTION_FLAGS.contains(&(flag, takes)), "nextest: {flag}");
    }
    for list in [names(go_test::SELECTION_FLAGS), names(cargo_test::SELECTION_FLAGS), names(cargo_nextest::SELECTION_FLAGS), names(cargo_test::AFTER_DOUBLE_DASH)] {
        let mut sorted = list.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), list.len(), "a flag listed twice: {list:?}");
        assert!(list.iter().all(|f| f.starts_with('-')), "{list:?}");
    }
}

#[test]
fn the_watched_setup_files_are_the_plans_in_the_other_runners_style() {
    assert_eq!(go_test::WATCHED, ["**/go.mod"]);
    assert_eq!(cargo_test::WATCHED, ["**/Cargo.toml", "**/Cargo.lock"]);
    assert_eq!(cargo_nextest::WATCHED, ["**/Cargo.toml", "**/Cargo.lock", ".config/nextest.toml"]);
}

#[test]
fn there_is_no_kit_file_for_any_of_the_three() {
    // go's and nextest's own JSON is the report and the converters are Rust; cargo test has no reader
    assert!(go_test::KIT_FILES.is_empty() && cargo_nextest::KIT_FILES.is_empty() && cargo_test::KIT_FILES.is_empty());
}

/// The probes in `PROBES` order, whichever runner's kit.
fn check_probes(runner: &str, probes: &[go_test::Probe]) {
    let ids: Vec<&str> = probes.iter().map(|p| p.0).collect();
    assert_eq!(ids, PROBES, "{runner}");
    for (id, label, kind, files) in probes {
        let t = truth(id);
        assert_eq!((*label, if kind.is_empty() { None } else { Some(*kind) }), (t.label.as_str(), t.kind.as_deref()), "{runner}/{id}: the spike's truth");
        assert!(!files.is_empty(), "{runner}/{id}");
        let lower = id.to_lowercase();
        for (path, text) in *files {
            assert!(path.contains(&format!("familiar_probe_{lower}")), "{runner}/{id}: a probe's file is its own: {path}");
            assert!(!path.starts_with('/') && !path.contains(".."), "{path}");
            assert!(text.ends_with('\n') && text.len() > 20, "{runner}/{id}: {path}");
        }
    }
}

#[test]
fn the_go_probe_kit_is_a_package_per_probe_with_the_spikes_truths() {
    check_probes("go", go_test::PROBES);
    let file = |id: &str, name: &str| go_test::PROBES.iter().find(|p| p.0 == id).unwrap().3.iter().find(|f| f.0.ends_with(name)).unwrap_or_else(|| panic!("{id}: no {name}")).1;
    for p in go_test::PROBES {
        let k = p.0.to_lowercase();
        let paths: Vec<&str> = p.3.iter().map(|f| f.0).collect();
        assert_eq!(paths, [format!("familiar_probe_{k}/probe_test.go"), format!("familiar_probe_{k}/probe.go")], "{}: a package per probe, the test first (the one the command names), then the code", p.0);
        assert!(p.3.iter().all(|f| f.1.starts_with("package probe\n")), "{}", p.0);
    }
    assert!(file("P2", "probe_test.go").contains("got != 5"), "an assertion that fails");
    assert!(file("P3", "probe_test.go").contains("Mul(2, 3)") && !file("P3", "probe.go").contains("func Mul"), "the function does not exist");
    assert!(file("P3b", "probe.go").contains("func Mul"), "now it does");
    assert_eq!(file("P3", "probe_test.go"), file("P3b", "probe_test.go"), "the same test");
    assert!(file("P4", "probe_test.go").contains("example.com/nosuch"), "a package that is not there");
    assert!(file("P5", "probe_test.go").contains("t.Skip(\"not yet\")"));
    assert!(!file("P6", "probe_test.go").contains("func Test"), "nothing to run");
    assert!(file("P7", "probe_test.go").contains("Parse(\"x1\")") && file("P7", "probe.go").contains("panic("), "an exception inside the code's function");
    assert_eq!(go_test::SKIPPED, ["TestLater"], "P5's test that must read skipped");
}

#[test]
fn the_rust_probe_kit_is_a_test_target_per_probe_that_needs_nothing_of_the_project() {
    check_probes("cargo", cargo_test::PROBES);
    // one kit for cargo test and for nextest
    assert_eq!(cargo_nextest::PROBES.len(), cargo_test::PROBES.len());
    for (a, b) in cargo_nextest::PROBES.iter().zip(cargo_test::PROBES) {
        assert_eq!((a.0, a.3), (b.0, b.3));
    }
    assert_eq!(cargo_nextest::SKIPPED, cargo_test::SKIPPED);
    let file = |id: &str| cargo_test::PROBES.iter().find(|p| p.0 == id).unwrap().3[0].1;
    for p in cargo_test::PROBES {
        let k = p.0.to_lowercase();
        let paths: Vec<&str> = p.3.iter().map(|f| f.0).collect();
        assert_eq!(paths, [format!("tests/familiar_probe_{k}.rs")], "{}: a test target per probe, one file", p.0);
        let text = p.3[0].1;
        assert!(!text.contains("probe::") && !text.contains("extern crate"), "{}: the code under test is in the file, not the project's crate", p.0);
    }
    assert!(file("P2").contains("assert_eq!(add(2, 2), 5)"), "an assertion that fails");
    assert!(file("P3").contains("mul(2, 3)") && !file("P3").contains("fn mul("), "the function does not exist");
    assert!(file("P3b").contains("fn mul("), "now it does");
    assert!(file("P4").contains("use nosuch::FOUR;"), "a crate that is not there");
    assert!(file("P5").contains("#[ignore = \"not yet\"]"));
    assert!(!file("P6").contains("#[test]"), "nothing to run");
    assert!(file("P7").contains("parse(\"x1\")") && file("P7").contains("panic!("), "a panic inside the code's function");
    assert_eq!(cargo_test::SKIPPED, ["later"], "P5's test that must read skipped");
    // warnings are not errors in the person's setup (RUSTFLAGS=-D warnings): no probe has an unused item
    for id in ["P1", "P2", "P3b", "P5", "P7"] {
        assert!(!file(id).contains("dead_code"), "{id}");
    }
}
