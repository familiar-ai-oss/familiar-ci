//! H-2, lane H5 (plan of record 2026-10-19, revised by the A0 spike's findings §10): a test
//! that is a shell script is read through the shell's own trace in Familiar's own run, never by
//! rewriting the person's tests. A failing check (`test`, `[`, `[[`, `grep -q`, `diff`, `cmp`)
//! at a line of the script, or one just before `exit N`, is an assertion at that line; the
//! place's own script failing is an exception there (the place rule decides); a name the shell
//! cannot find is a missing name; anything else is can't tell. Exit 0 is the sealed script's
//! green. Each run here goes through `familiar_ai_run::run_test_command_traced` (the env, the
//! side file on its descriptor, the kit written once per version) and `kept::classify`, as the
//! binary's runs do.

use std::path::{Path, PathBuf};
use std::time::Duration;

use familiar_ai::door::kept::{classify, clean, Streams};
use familiar_ai_facts::LocationKind;
use familiar_ai_run::shell_trace::{sh_scripts, split_dash};
use familiar_ai_run::{details_for, run_test_command_traced, End};

const APP: &str = "#!/bin/sh\ngreet() { echo hi; }\ngreet\n";

fn project(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let d = std::env::temp_dir().join(format!("familiar-ai-h5-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    for (p, text) in files {
        let f = d.join(p);
        std::fs::create_dir_all(f.parent().expect("a folder")).expect("folder");
        std::fs::write(&f, text).expect("file");
    }
    d
}

/// Where a reading points: file, line, kind.
type At = Option<(String, u64, Option<LocationKind>)>;

/// The run's reading: label, location, the stderr the run kept.
fn read(dir: &Path, cmd: &str) -> (String, At, String) {
    let (out, trace) = run_test_command_traced(cmd, dir, Duration::from_secs(30), None, &[], details_for(cmd));
    let (stdout, stderr, report) = (clean(&out.stdout), clean(&out.stderr), clean(&trace));
    let t = classify(out.end, &Streams { stdout: &stdout, stderr: &stderr, report: &report }, details_for(cmd)).expect("a reading");
    // a can't tell prints the raw trace, shown when the test fails (hosted CI's dash, 2026-10-26)
    if t.label == "unclassified" {
        eprintln!("can't tell: {cmd}\n--- the run's trace, raw ---\n{trace}--- stderr ---\n{}", out.stderr);
    }
    let _ = std::fs::remove_dir_all(dir);
    (t.label, t.location.map(|l| (l.file_path, l.line, l.kind)), out.stderr)
}

fn assertion(file: &str, line: u64) -> At {
    Some((file.to_string(), line, Some(LocationKind::Assertion)))
}

#[test]
fn a_failing_check_at_the_scripts_last_line_is_an_assertion_there() {
    for (name, body, line) in [
        ("pipe", "sh src/app.sh | grep -qx hello\n", 1),
        ("bracket", "out=$(sh src/app.sh)\n[ \"$out\" = hello ]\n", 2),
        ("sourced", ". src/app.sh >/dev/null; greet | grep -qx hello\n", 1),
        ("grep", "echo checking\ngrep -q hello src/app.sh\n", 2),
    ] {
        let d = project(name, &[("src/app.sh", APP), ("tests/test_c1.sh", body)]);
        let (label, at, _) = read(&d, "sh tests/test_c1.sh");
        assert_eq!((label.as_str(), at), ("tests_failed", assertion("tests/test_c1.sh", line)), "{name}: {body}");
    }
}

#[test]
fn exit_n_just_after_a_check_is_the_checks_assertion() {
    for (name, body, line) in [
        ("or_exit", "grep -q hello src/app.sh || { echo 'no hello'; exit 1; }\necho done\n", 1),
        ("if_not", "check() {\n  if ! grep -q hello \"$1\"; then\n    echo FAIL\n    exit 1\n  fi\n}\ncheck src/app.sh\n", 2),
    ] {
        let d = project(name, &[("src/app.sh", APP), ("tests/test_c1.sh", body)]);
        let (label, at, _) = read(&d, "sh tests/test_c1.sh");
        assert_eq!((label.as_str(), at), ("tests_failed", assertion("tests/test_c1.sh", line)), "{name}: {body}");
    }
}

#[test]
fn a_bash_script_is_read_through_the_kit() {
    let body = "#!/bin/bash\nprintf 'alpha\\n' > out.txt\necho checking\ngrep -q gamma out.txt\n";
    let d = project("bash", &[("tests/test_c1.sh", body)]);
    let (label, at, _) = read(&d, "bash tests/test_c1.sh");
    assert_eq!((label.as_str(), at), ("tests_failed", assertion("tests/test_c1.sh", 4)));
    let body = "#!/bin/bash\nset -e\nassert_eq() { [ \"$1\" = \"$2\" ]; }\nassert_eq \"$(echo 2)\" 3\necho never\n";
    let d = project("bash-func", &[("tests/test_c1.sh", body)]);
    let (label, at, _) = read(&d, "bash tests/test_c1.sh");
    assert_eq!((label.as_str(), at), ("tests_failed", assertion("tests/test_c1.sh", 3)), "the check inside the helper");
}

#[test]
fn the_places_own_script_failing_is_an_exception_at_its_line() {
    let app = "#!/bin/sh\ngreet() { echo hi; }\nexit 3\n";
    let d = project("place", &[("src/app.sh", app), ("tests/test_c1.sh", "sh src/app.sh\n")]);
    let (label, at, _) = read(&d, "sh tests/test_c1.sh");
    assert_eq!((label.as_str(), at), ("tests_failed", Some(("src/app.sh".to_string(), 3, Some(LocationKind::Exception)))));
}

#[test]
fn anything_else_is_cant_tell() {
    for (name, body) in [("silent", "exit 1\n"), ("sleep", "sleep 0\nexit 1\n"), ("cd", "set -e\ncd /nonexistent/familiar-ai-h5\necho never\n"), ("cd_or_exit", "cd /nonexistent/familiar-ai-h5 || exit 3\n"), ("cat", "cat src/app.sh\nexit 1\n")] {
        let d = project(name, &[("src/app.sh", APP), ("tests/test_c1.sh", body)]);
        let (label, at, _) = read(&d, "sh tests/test_c1.sh");
        assert_eq!((label.as_str(), at), ("unclassified", None), "{name}: {body}");
    }
}

#[test]
fn a_missing_name_and_a_pass_read_as_before() {
    let d = project("missing", &[("src/app.sh", APP), ("tests/test_c1.sh", ". src/app.sh >/dev/null; farewell | grep -qx goodbye\n")]);
    let (label, at, _) = read(&d, "sh tests/test_c1.sh");
    assert_eq!((label.as_str(), at.map(|a| (a.0, a.1))), ("missing_target_symbol", Some(("tests/test_c1.sh".to_string(), 1))));
    let d = project("pass", &[("src/app.sh", APP), ("tests/test_c1.sh", "sh src/app.sh | grep -qx hi\n")]);
    assert_eq!(read(&d, "sh tests/test_c1.sh").0, "passed");
}

#[test]
fn the_persons_stderr_and_stdout_are_untouched() {
    let body = "echo out\necho err >&2\ngrep -q nope src/app.sh\n";
    let d = project("streams", &[("src/app.sh", APP), ("tests/test_c1.sh", body)]);
    let (out, trace) = run_test_command_traced("sh tests/test_c1.sh", &d, Duration::from_secs(30), None, &[], details_for("sh tests/test_c1.sh"));
    assert_eq!((out.stdout.as_str(), out.stderr.as_str(), out.end), ("out\n", "err\n", End::Exited(1)));
    assert!(trace.contains("tests/test_c1.sh\t3\tgrep -q nope src/app.sh"), "{trace}");
    // a runner that is not a shell script gets no trace
    let (_, none) = run_test_command_traced("true", &d, Duration::from_secs(30), None, &[], details_for("make test"));
    assert_eq!(none, "");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn the_kit_is_written_once_per_version_outside_the_copy() {
    let a = familiar_ai_run::shell_trace::kit_path().expect("the kit");
    let b = familiar_ai_run::shell_trace::kit_path().expect("the kit");
    assert_eq!(a, b);
    assert!(a.starts_with(std::env::temp_dir()), "{}", a.display());
    assert_eq!(std::fs::read_to_string(&a).expect("kit"), familiar_ai_run::shell_trace::KIT);
    assert!(a.to_string_lossy().contains(env!("CARGO_PKG_VERSION")), "{}", a.display());
}

/// bats 1.x's TAP lines for `bad.bats` (scratchpad bats-lab): one test failed on a check, one on a
/// command not found.
const BATS_MIXED: &str = "1..3\nok 1 adds\nnot ok 2 fails\n# (in test file bad.bats, line 2)\n#   `@test \"fails\" { [ $((2+2)) -eq 5 ]; }' failed\nnot ok 3 cmdnf\n# (in test file bad.bats, line 3)\n#   `@test \"cmdnf\" { nosuchcmd_xyz; }' failed with status 127\n# /code/bad.bats: line 3: nosuchcmd_xyz: command not found\n";

#[test]
fn bats_lines_are_read_when_a_script_runs_bats() {
    let sh = details_for("sh tests/run.sh");
    let read = |stdout: &str| classify(End::Exited(1), &Streams { stdout, ..Default::default() }, sh).expect("a reading");
    // a test that failed on an error beside one that failed on a check: can't tell
    assert_eq!(read(BATS_MIXED).label, "unclassified");
    // every failing test failed on a check: an assertion at its bats line
    let checks = "1..2\nok 1 adds\nnot ok 2 fails\n# (in test file test/bad.bats, line 2)\n#   `[ \"$result\" -eq 5 ]' failed\n";
    let t = read(checks);
    assert_eq!((t.label.as_str(), t.location.map(|l| (l.file_path, l.line, l.kind))), ("tests_failed", Some(("test/bad.bats".to_string(), 2, Some(LocationKind::Assertion)))));
    assert_eq!(t.evidence, "not ok 2 fails");
}

/// Ubuntu's dash writes a trace line in pieces (PS4, each word, the newline), so the commands of
/// a pipeline, traced at once, tear each other's lines: about 1 run in 100 here, often on GitHub's
/// small runners (hosted CI, 2026-10-26: run 40406964060's five stand-ins whose test is
/// `sh src/app.sh | grep -qx hello`, and this file's pipe and missing-name cases, read as can't
/// tell; the sourced case read as an exception at src/app.sh:2). A torn trace reads as the
/// untorn one. Each stderr is Ubuntu's dash's in Familiar's run of the case (`PS4_SH`, `-x`, the
/// `sh` wrapper), verbatim: every torn shape seen in 3,200 runs of each, the first untorn; then
/// shapes seen in 16,000 runs each under 32 at once (dash's message glued to a word, a stretch
/// before its sourcing was read, one from Familiar's own run), and an app that writes `oops` to
/// stderr: a line of the person's that lands inside the stretch. The person's stderr is the
/// untorn one's too.
#[test]
fn a_torn_dash_trace_reads_as_the_untorn_one() {
    let cmd = "sh tests/test_c1.sh";
    let oops = "#!/bin/sh\necho oops >&2\necho hi\n";
    for (name, app, body, code, runs, label, at) in [
        ("pipe", APP, "sh src/app.sh | grep -qx hello\n", 1, PIPE, "tests_failed", assertion("tests/test_c1.sh", 1)),
        ("sourced", APP, ". src/app.sh >/dev/null; greet | grep -qx hello\n", 1, SOURCED, "tests_failed", assertion("tests/test_c1.sh", 1)),
        ("missing", APP, ". src/app.sh >/dev/null; farewell | grep -qx goodbye\n", 1, MISSING, "missing_target_symbol", Some(("tests/test_c1.sh".to_string(), 1, None))),
        ("pass", APP, "sh src/app.sh | grep -qx hi\n", 0, PASS, "passed", None),
        ("oops", oops, "sh src/app.sh | grep -qx hello\n", 1, OOPS, "tests_failed", assertion("tests/test_c1.sh", 1)),
    ] {
        let d = project(&format!("torn-{name}"), &[("src/app.sh", app), ("tests/test_c1.sh", body)]);
        let scripts = sh_scripts(cmd, &d);
        let persons = split_dash(runs[0], &scripts, &d).stderr;
        for stderr in runs {
            let split = split_dash(stderr, &scripts, &d);
            let (kept, report) = (clean(&split.stderr), clean(&split.trace));
            let t = classify(End::Exited(code), &Streams { stdout: "", stderr: &kept, report: &report }, details_for(cmd)).expect("a reading");
            let got = (t.label.as_str(), t.location.map(|l| (l.file_path, l.line, l.kind.filter(|_| label != "missing_target_symbol"))), split.stderr.as_str());
            assert_eq!(got, (label, at.clone(), persons.as_str()), "{name}: {stderr:?}\n--- trace ---\n{report}--- stderr ---\n{kept}");
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}

const PIPE: &[&str] = &[
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tsh src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgrep -qx hello\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgrep -qx hello\nsh src/app.sh\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgrep -qx hellosh\n src/app.sh\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgrepsh -qx hello src/app.sh\n\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgrepsh -qx src/app.sh hello\n\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgrepsh src/app.sh -qx\n hello\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tsh src/app.sh\ngrep -qx hello\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tsh src/app.shgrep\n -qx hello\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tshgrep -qx hello\n src/app.sh\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tshgrep src/app.sh\n -qx hello\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tshgrep src/app.sh -qx\n hello\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tshgrep src/app.sh -qx hello\n\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgrep -qx+familiar-ai-sh\ttests/test_c1.sh\t\t hello\nsh src/app.sh\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tsh+familiar-ai-sh\ttests/test_c1.sh\t\t src/app.sh\ngrep -qx hello\n",
];

const SOURCED: &[&str] = &[
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\tgrep -qx hello\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgrep -qx hello\ngreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\ngrep -qx hello\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\ngrep -qx hello\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgreetgrep\n -qx hello\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet+familiar-ai-sh\ttests/test_c1.sh\t\t\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\ngrep -qx hello\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo higrep\n -qx hello\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\tgrepecho -qx hello hi\n\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\tgrepecho -qx hi hello\n\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\tgrepecho hi\n -qx hello\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\ngrep -qx hello\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\ngrep+familiar-ai-sh\ttests/test_c1.sh\t\techo -qx hi hello\n\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgrepgreet\n -qx hello+familiar-ai-sh\ttests/test_c1.sh\t\t\necho hi\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgrepgreet\n -qx+familiar-ai-sh\ttests/test_c1.sh\t\t helloecho\n hi\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo+familiar-ai-sh\ttests/test_c1.sh\t\t hi\ngrep -qx hello\n",
];

const MISSING: &[&str] = &[
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\tfarewell\ntests/test_c1.sh: 1: farewell: not found\n+familiar-ai-sh\ttests/test_c1.sh\t\tgrep -qx goodbye\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\tfarewell\n+familiar-ai-sh\ttests/test_c1.sh\t\ttests/test_c1.sh: 1: farewell: not found\ngrep -qx goodbye\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgrep -qx goodbye\nfarewell\ntests/test_c1.sh: 1: farewell: not found\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\tfarewell\n+familiar-ai-sh\ttests/test_c1.sh\t\tgrep -qx goodbye\ntests/test_c1.sh: 1: farewell: not found\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\tfarewell\ntests/test_c1.sh: 1: +familiar-ai-sh\ttests/test_c1.sh\t\tfarewell: not found\ngrep -qx goodbye\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tfarewell\ngrep -qx goodbye\ntests/test_c1.sh: 1: farewell: not found\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgrepfarewell\n -qx goodbye\ntests/test_c1.sh: 1: farewell: not found\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tfarewell\ntests/test_c1.sh: 1: farewell: not found\ngrep -qx goodbye\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tfarewellgrep -qx\n goodbye\ntests/test_c1.sh: 1: farewell: not found\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\tfarewell\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreptests/test_c1.sh: 1:  -qxfarewell: not found goodbye\n\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tfarewellgrep -qx goodbye\n\ntests/test_c1.sh: 1: farewell: not found\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\tfarewell\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreptests/test_c1.sh: 1:  -qx goodbyefarewell: not found\n\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\tfarewell\n+familiar-ai-sh\ttests/test_c1.sh\t\ttests/test_c1.sh: 1: grepfarewell: not found\n -qx goodbye\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\tfarewell+familiar-ai-sh\ttests/test_c1.sh\t\t\ngrep -qx goodbye\ntests/test_c1.sh: 1: farewell: not found\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tfarewell\ntests/test_c1.sh: 1: farewell: not foundgrep -qx\n goodbye\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\tfarewell+familiar-ai-sh\ttests/test_c1.sh\t\t\ntests/test_c1.sh: 1: farewell: not foundgrep\n -qx goodbye\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tfarewell\ntests/test_c1.sh: 1: farewell: not foundgrep\n -qx goodbye\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\tfarewell\ntests/test_c1.sh: 1: +familiar-ai-sh\ttests/test_c1.sh\t\tfarewell: not foundgrep -qx goodbye\n\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\tfarewell+familiar-ai-sh\ttests/test_c1.sh\t\t\ngreptests/test_c1.sh: 1: farewell: not found\n -qx goodbye\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\tfarewell\ntests/test_c1.sh: 1: +familiar-ai-sh\ttests/test_c1.sh\t\tfarewell: not foundgrep\n -qx goodbye\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\tfarewell+familiar-ai-sh\ttests/test_c1.sh\t\t\ngreptests/test_c1.sh: 1:  -qx goodbyefarewell: not found\n\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tfarewell\ntests/test_c1.sh: 1: farewell: not foundgrep -qx goodbye\n\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\tfarewell\n+familiar-ai-sh\ttests/test_c1.sh\t\ttests/test_c1.sh: 1: farewell: not foundgrep\n -qx goodbye\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\tfarewell+familiar-ai-sh\ttests/test_c1.sh\t\t\ntests/test_c1.sh: 1: farewell: not foundgrep -qx goodbye\n\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\tfarewell\n+familiar-ai-sh\ttests/test_c1.sh\t\ttests/test_c1.sh: 1: farewell: not foundgrep -qx goodbye\n\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\tfarewell+familiar-ai-sh\ttests/test_c1.sh\t\t\ngreptests/test_c1.sh: 1: farewell: not found -qx\n goodbye\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\tfarewell\ntests/test_c1.sh: 1: +familiar-ai-sh\ttests/test_c1.sh\t\tfarewell: not foundgrep -qx\n goodbye\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t. src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgreet\n+familiar-ai-sh\ttests/test_c1.sh\t\techo hi\n+familiar-ai-sh\ttests/test_c1.sh\t\tfarewell+familiar-ai-sh\ttests/test_c1.sh\t\t\ngreptests/test_c1.sh: 1:  -qxfarewell: not found goodbye\n\n",
];

const PASS: &[&str] = &[
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tsh src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgrep -qx hi\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tsh src/app.sh\ngrep -qx hi\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgrep -qx hi\nsh src/app.sh\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgrep -qx hish\n src/app.sh\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgrepsh src/app.sh\n -qx hi\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tshgrep src/app.sh\n -qx hi\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgrep -qxsh hi\n src/app.sh\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgrepsh -qx src/app.sh\n hi\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tgrepsh -qx src/app.sh hi\n\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tshgrep -qx hi\n src/app.sh\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgrep -qx+familiar-ai-sh\ttests/test_c1.sh\t\t hi\nsh src/app.sh\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tsh src/app.sh+familiar-ai-sh\ttests/test_c1.sh\t\t\ngrep -qx hi\n",
];

const OOPS: &[&str] = &[
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tsh src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgrep -qx hello\noops\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tsh src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgrepoops\n -qx hello\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tsh src/app.sh\noops\n+familiar-ai-sh\ttests/test_c1.sh\t\tgrep -qx hello\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tsh src/app.sh\ngrep -qxoops\n hello\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tsh+familiar-ai-sh\ttests/test_c1.sh\t\t src/app.sh\noops\ngrep -qx hello\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tsh src/app.sh\noops\ngrep -qx hello\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tsh src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgrep -qx hellooops\n\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tsh+familiar-ai-sh\ttests/test_c1.sh\t\t src/app.sh\ngrepoops\n -qx hello\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tsh src/app.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\toops\ngrep -qx hello\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\t+familiar-ai-sh\ttests/test_c1.sh\t\tsh src/app.sh\ngrepoops\n -qx hello\n",
    "+familiar-ai-sh\t/var/tmp/familiar-lanes/dashbin/sh\t\tsh -x tests/test_c1.sh\n+familiar-ai-sh\ttests/test_c1.sh\t\tgrep -qx+familiar-ai-sh\ttests/test_c1.sh\t\tsh src/app.sh\noops\n hello\n",
];
