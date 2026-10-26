//! Lane H4 (jvm, .NET; plan of record 2026-10-19): a failed assertion of Maven, Gradle, xunit,
//! NUnit and MSTest points at the line of the planned test that asserted, with the kind
//! `assertion` (H3 keeps each candidate's kind; H6's family table reads an assertion in a planned
//! test file as an honest red and an exception there as can't tell, D1). Each recorded assertion
//! red is read the way the binary reads it (`kept::classify` on the command's runner, then
//! `test_log_translation` with the recorded sources standing in for git), and the location it
//! keeps must be the planned test's own assert line.

use std::path::{Path, PathBuf};

use familiar_ai::door::kept::{classify, clean, in_project, test_log_translation, Streams};
use familiar_ai_run::{details_for, End};

/// Per runner: the planned test file the assert line is in, and the recorded assertion reds (the
/// folders whose failure is a test's assertion and whose assert line the recorded sources hold).
const CASES: &[(&str, &str, &[&str])] = &[
    ("maven", "src/test/java/calc/CalcTest.java", &["tests_failed", "tests_failed_quiet"]),
    ("gradle", "src/test/java/calc/CalcTest.java", &["tests_failed"]),
    ("dotnet-xunit", "tests/CalcTests.cs", &["tests_failed"]),
    ("dotnet-nunit", "tests/CalcTests.cs", &["tests_failed"]),
    ("dotnet-mstest", "tests/CalcTests.cs", &["tests_failed"]),
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Where the recorder ran each fixture (`/r/<runner>/<folder>`); it becomes the run copy.
const COPY: &str = "/tmp/familiar-ai-run-4242-7";

fn text(p: &Path, runner: &str, folder: &str) -> String {
    clean(&String::from_utf8_lossy(&std::fs::read(p).unwrap_or_default())).replace(&format!("/r/{runner}/{folder}/"), &format!("{COPY}/")).replace(&format!("/r/{runner}/{folder}"), COPY)
}

fn report(dir: &Path, runner: &str, folder: &str) -> String {
    let mut files = Vec::new();
    let mut stack = vec![dir.join("report")];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                files.push(p);
            }
        }
    }
    files.sort();
    files.iter().map(|p| text(p, runner, folder)).collect::<Vec<_>>().join("\n")
}

/// The command as the recording ran it, without the recorder's docker wrapper.
fn command(cmd: &str) -> String {
    match cmd.strip_prefix("../../_bin/dock ") {
        Some(rest) => {
            let inner = rest.split_once(' ').map_or(rest, |(_, c)| c).trim();
            match inner.strip_prefix('\'') {
                Some(q) => q.split_once('\'').map_or(q, |(c, _)| c).to_string(),
                None => inner.to_string(),
            }
        }
        None => cmd.to_string(),
    }
}

#[test]
fn a_failed_assertion_keeps_the_line_of_the_planned_test_that_asserted() {
    let mut wrong = Vec::new();
    for (runner, planned, folders) in CASES {
        for folder in *folders {
            let dir = root().join("run_logs").join(runner).join(folder);
            let code: i32 = std::fs::read_to_string(dir.join("exit")).unwrap_or_default().trim().parse().unwrap_or_else(|_| panic!("{runner}/{folder}: an exit code"));
            let (stdout, stderr, rep) = (text(&dir.join("stdout"), runner, folder), text(&dir.join("stderr"), runner, folder), report(&dir, runner, folder));
            let cmd = command(std::fs::read_to_string(dir.join("cmd")).unwrap_or_default().trim());
            let s = Streams { stdout: &stdout, stderr: &stderr, report: &rep };
            let t = classify(End::Exited(code), &s, details_for(&cmd)).unwrap_or_else(|| panic!("{runner}/{folder}: a reading"));
            let sources = root().join("run_sources").join(runner).join("tests_failed_exception");
            let read = |p: &str| std::fs::read(sources.join(p)).ok();
            let t = test_log_translation(t, "r1", "c1", "b2", "full", |c| in_project(c, &read));
            match t.location {
                Some(l) if l.kind == Some(familiar_ai_facts::LocationKind::Assertion) && l.file_path == *planned => {}
                other => wrong.push(format!("{runner}/{folder}: read {} with location {other:?}, want an assertion in {planned}", t.label)),
            }
        }
    }
    assert!(wrong.is_empty(), "{} assertion reds without their line:\n{}", wrong.len(), wrong.join("\n"));
}
