//! Lane H4js (plan of record 2026-10-19, H3's location rules): a failed assertion of the JS
//! runners is kept with the line it failed at and the kind `assertion`, so the rules can say where
//! a red points (a location in the planned test file, an assertion: an honest red; no location:
//! can't tell). jest and node's TAP already did; vitest's and mocha's tables left an assertion with
//! no location (mocha's own comment: "an assertion yields none"), and the reporters mocha ships
//! beside spec (tap, json, json-stream, xunit) and node 24's default (spec) were not read at all.
//! Each case is a recording under `tests/fixtures/run_logs`, read the way the binary reads it.

use std::path::{Path, PathBuf};

use familiar_ai::door::kept::{classify, clean, in_project, test_log_translation, Streams};
use familiar_ai_run::{details_for, End};
use familiar_ai_facts::FactFields;
use serde_json::Value;

/// Where the recorder ran each fixture; each becomes the run copy.
const RECORDED_AT: &str = "/tmp/claude-1000/-home-user-projects-familiar-ai-oss/ce46f81f-ec40-4c7d-885d-547ef9d911b3/scratchpad/run_logs";
const COPY: &str = "/tmp/familiar-ai-run-4242-7";

/// runner, folder, the location's kind, file, line.
const KEPT: &[(&str, &str, &str, &str, u64)] = &[
    ("jest", "tests_failed", "assertion", "test/calc.test.js", 4),
    ("jest", "tests_failed_exception", "exception", "src/calc.js", 7),
    ("vitest", "tests_failed", "assertion", "test/calc.test.js", 5),
    ("vitest", "tests_failed_exception", "exception", "src/calc.js", 7),
    ("mocha", "tests_failed", "assertion", "test/calc.test.js", 6),
    ("mocha", "tests_failed_exception", "exception", "src/calc.js", 7),
    ("mocha", "tests_failed_reporter_tap", "assertion", "test/calc.test.js", 6),
    ("mocha", "tests_failed_reporter_json", "assertion", "test/calc.test.js", 6),
    ("mocha", "tests_failed_reporter_json_stream", "assertion", "test/calc.test.js", 6),
    ("mocha", "tests_failed_reporter_xunit", "assertion", "test/calc.test.js", 6),
    ("node-test", "tests_failed", "assertion", "test/calc.test.js", 6),
    ("node-test", "tests_failed_spec", "assertion", "test/calc.test.js", 6),
    ("node-test", "tests_failed_exception_spec", "exception", "src/calc.js", 7),
];

fn dir(runner: &str, folder: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/run_logs").join(runner).join(folder)
}

/// The text as a run copy prints it: the recording's own directory becomes the copy.
fn text(runner: &str, folder: &str, stream: &str) -> String {
    let t = clean(&String::from_utf8_lossy(&std::fs::read(dir(runner, folder).join(stream)).unwrap_or_default()));
    t.replace(&format!("{RECORDED_AT}/{runner}/{folder}/"), &format!("{COPY}/")).replace(&format!("{RECORDED_AT}/{runner}/{folder}"), COPY)
}

/// The location the binary keeps for the fixture's run, as the translation's fields hold it.
fn kept(runner: &str, folder: &str) -> Option<Value> {
    let code: i32 = std::fs::read_to_string(dir(runner, folder).join("exit")).expect("an exit code").trim().parse().expect("a number");
    let cmd = std::fs::read_to_string(dir(runner, folder).join("cmd")).expect("a command");
    let (stdout, stderr) = (text(runner, folder, "stdout"), text(runner, folder, "stderr"));
    let streams = Streams { stdout: &stdout, stderr: &stderr, report: "" };
    let t = classify(End::Exited(code), &streams, details_for(cmd.trim()))?;
    // the fixture's own sources, else the runner's tests_failed_exception (the truth harness's stand-in for git)
    let own = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/run_sources").join(runner).join(folder);
    let sources = if own.is_dir() { own } else { own.with_file_name("tests_failed_exception") };
    let read = move |p: &str| std::fs::read(sources.join(p)).ok();
    let t = test_log_translation(t, "r1", "c1", "b2", "full", |cands| in_project(cands, &read));
    Some(Value::Object(t.to_fields().expect("a translation's fields")).get("location").cloned().unwrap_or(Value::Null))
}

#[test]
fn a_failed_assertion_of_a_js_runner_is_kept_with_its_line_and_the_kind_assertion() {
    let mut wrong = Vec::new();
    for (runner, folder, kind, file, line) in KEPT {
        let got = kept(runner, folder);
        let at = |k: &str| got.as_ref().and_then(|l| l.get(k)).cloned();
        let now = (at("kind"), at("file_path"), at("line"));
        let want = (Some(Value::from(*kind)), Some(Value::from(*file)), Some(Value::from(*line)));
        if now != want {
            wrong.push(format!("{runner}/{folder}: kept {got:?}, want {kind} at {file}:{line}"));
        }
    }
    assert!(wrong.is_empty(), "{} of {} not kept:\n{}", wrong.len(), KEPT.len(), wrong.join("\n"));
}
