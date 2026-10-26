//! A recorded run (plan S0) reads as its outcome through the path the binary takes: the
//! runner its command names (`details_for`, plan §5) selects one table per `RUNNER_DETAILS` row,
//! and a command naming no runner reads every table (plan S4). Exit 127 splits by what the
//! shell could not find whatever the command names (plan §2, the owner, 2026-10-14).

use std::path::{Path, PathBuf};

use familiar_ai::door::kept::{classify, clean, Streams};
use familiar_ai_run::{details_for, End};

/// The outcome a folder's name says, by its prefix (wave 0's checker).
const PREFIX: &[(&str, &str)] = &[
    ("passed", "passed"),
    ("tests_failed", "tests_failed"),
    ("big_run", "tests_failed"),
    ("via_ctest_tests_failed", "tests_failed"),
    ("collection_error", "collection_error"),
    ("missing_target_symbol", "missing_target_symbol"),
    ("no_tests_collected", "no_tests_collected"),
    ("setup_error", "setup_error"),
    ("usage_error", "usage_error"),
    ("environment_error", "environment_error"),
    ("runner_crash", "runner_crash"),
    ("killed_sigkill", "killed"),
    ("killed_segv", "crashed"),
    ("killed_abort", "crashed"),
    ("killed_nil_deref", "crashed"),
    ("killed_failfast", "crashed"),
    // lane H5: a run only can't tell reads honestly (the A0 spike's `cd` probes)
    ("cant_tell", "unclassified"),
];

/// Where a folder's name is not its honest outcome (wave 0's `<runner>.expect`, and the owner's
/// two decisions): runner, folder, outcome, why.
const EXPECT: &[(&str, &str, &str, &str)] = &[
    ("pytest", "passed_skipped_pretty", "unclassified", "a pytest-pretty run with one test passed and one skipped (exit 0): plain pytest prints `1 passed, 1 skipped in` and no row reads a skip as green, so it is unclassified (a card); a skipped planned test must never read as passed, and pytest-pretty reads the same"),
    ("catch2", "setup_error", "unclassified", "a TEST_CASE_METHOD fixture's constructor threw ('db down'): Catch2 prints 'calc_test.cpp:11: FAILED:' then 'due to unexpected exception with message:' with no expression, the same text as the code under test throwing before the REQUIRE (int r = parse(\"x1\"); REQUIRE(r == 1)), which is an honest red; text cannot tell them apart, so the honest label is unclassified (a card), never setup_error"),
    ("catch2", "environment_error_cmd_not_found", "usage_error", "the command runs ./build/calc_testx, a typo for the project's own test binary that the build just made (calc_test): 'sh: line 1: ./build/calc_testx: No such file or directory', exit 127, is the agent's command (plan 6.3), not a tool missing from the machine"),
    ("dotnet-mstest", "killed_segv_sent_by_kill", "passed", "kill -SEGV at the testhost is absorbed by the .NET runtime: both tests ran and passed (Passed: 2), exit 0; the folder name records the provocation, not the outcome"),
    ("dotnet-nunit", "killed_segv_sent_by_kill", "passed", "the test sends kill -SEGV to its own testhost; the .NET runtime absorbs the signal, both tests run and pass (Passed: 2), exit 0: nothing was killed or crashed"),
    ("dotnet-nunit", "runner_crash_adapter", "no_tests_collected", "NUnit3TestAdapter reference removed: nothing crashed; dotnet test prints the same No-test-is-available line as no_tests_collected_file, exit 0 (indistinguishable; plan 6.1 adds the missing-adapter hint to the no_tests_collected line)"),
    ("dotnet-xunit", "killed_segv_sent_by_kill", "passed", "kill -SEGV <testhost pid> (an SI_USER signal) is absorbed by the .NET runtime's own SIGSEGV handler: the process survives, both tests run and pass, VSTest prints 'Passed!  - Failed: 0, Passed: 2', exit 0. Nothing crashed."),
    ("dotnet-xunit", "collection_error_import", "missing_target_symbol", "csc reports `using Nosuch;` as CS0246 'The type or namespace name 'Nosuch' could not be found', the same code and words as a test-first reference to a class not yet written; the table cannot tell them apart, so both read missing_target_symbol (name=Nosuch, the test file, line 1) and §6.2's place rule decides: 'Nosuch' is not the place's symbol, so this run is Test is broken, the folder's family."),
    ("go-test", "killed_nil_deref", "unclassified", "a nil dereference in the test body (calc_test.go:9) is a recovered runtime-error panic whose innermost project frame is the test file: go prints no assertion text for it and nothing separates a stub returning nil (an honest TDD red) from a broken test, so it is a card (D1; lane h4rustgo's row 11c). The location row still reads calc_test.go:9, but a card carries none"),
    ("go-test", "environment_error_missing_dep", "unclassified", "`calc_test.go:6:2: no required module provides package github.com/nosuch/helperpkg; to add it:` is word for word what go prints for a test-first import of a package of this module that does not exist yet (collection_error_import): the text cannot say which, so neither the environment nor a broken test (lane h4rustgo's row 9c)"),
    ("go-test", "collection_error_import", "unclassified", "the twin of environment_error_missing_dep: `no required module provides package example.com/calc/nosuchpkg; to add it:` reads the same as a third-party package that is not installed, so it is a card (lane h4rustgo's row 9c)"),
    ("googletest", "usage_error", "passed", "--nosuchflag is not a gtest flag: gtest ignores it without a word, runs every test and they pass (exit 0, '[  PASSED  ] 1 test.'); the folder names the command's mistake, the run's honest outcome is passed"),
    ("googletest", "usage_error_gtest_flag", "unclassified", "an unknown --gtest_ flag prints gtest's help text and runs nothing, exit 0; row 1 names it usage_error and the exit-0 contradiction rule turns that into unclassified (a card), never passed"),
    ("gradle", "killed_exit", "crashed", "System.exit(3) inside a test: the forked test JVM ended itself mid-run (Gradle: \"Process 'Gradle Test Executor 1' finished with non-zero exit value 3\"); not a signal and deterministic, so not killed (a retry repeats it); the folder prefix killed_exit has no mapping; crashed is the honest route, as for killed_failfast elsewhere"),
    ("jest", "runner_crash", "environment_error", "`● Validation Error: Test environment nosuch-environment cannot be found`: the testEnvironment package is not installed; deterministic, a retry cannot fix it (not Transient)"),
    ("maven", "killed_exit", "crashed", "System.exit(3) inside a test: the forked JVM ended itself mid-run and surefire prints the same \"The forked VM terminated without properly saying goodbye. VM crash or System.exit called?\" with \"Process Exit Code: 3\"; not a signal and deterministic (a retry repeats it), so not killed; the folder prefix killed_exit has no mapping; crashed (a card) is the honest route, as gradle and dotnet killed_failfast"),
    ("mocha", "usage_error", "passed", "mocha 12.0.3 silently ignores the unknown flag --nosuchflag: the one test ran and passed (stdout \"  1 passing (1ms)\", exit 0, nothing mentions the flag); the output is byte-identical to the passed fixture apart from the timing, so the honest outcome is passed"),
    ("node-test", "runner_crash", "usage_error", "a --test-reporter path that does not exist is the command's own error (exit 7, ERR_MODULE_NOT_FOUND from the harness): deterministic, a retry cannot fix it"),
    // lane H4js, 2026-10-19: the JS runners' reporters and skips; what a run that says nothing, or a skip beside a pass, reads as
    ("jest", "tests_failed_reporters_junit", "unclassified", "`--reporters=jest-junit` replaces the console reporter: nothing is printed (the result is junit.xml, a file no reader opens), exit 1; no row can read the run, so unclassified (a card), never a label"),
    ("jest", "passed_reporters_junit", "unclassified", "`--reporters=jest-junit` prints nothing: exit 0 with an empty console says nothing (H-1), so unclassified (a card), never passed"),
    ("jest", "tests_failed_reporters_silent", "unclassified", "jest-silent-reporter replaces the console reporter and prints only the failure blocks, no `Tests:` summary, exit 1: unclassified (a card) until a reader sees the counts"),
    ("jest", "passed_reporters_silent", "unclassified", "jest-silent-reporter prints nothing for a green run: exit 0 with an empty console says nothing (H-1), so unclassified (a card), never passed"),
    ("jest", "tests_failed_reporters_github_actions", "unclassified", "`--reporters=github-actions` alone prints nothing outside GitHub Actions (its `silent` option defaults true), exit 1: unclassified (a card)"),
    ("jest", "passed_reporters_github_actions", "unclassified", "`--reporters=github-actions` alone prints nothing: exit 0 with an empty console says nothing (H-1), so unclassified (a card), never passed"),
    ("node-test", "passed_skipped_spec", "unclassified", "node 24's spec reporter, one test passed and one skipped (exit 0): a skip beside a pass is not read as passed by a row (D4, H-3: the counts and the rules of H6 decide it), so unclassified (a card)"),
    // the owner, 2026-10-14: CS0246 is missing_target_symbol in all three .NET tables
    ("dotnet-nunit", "collection_error_import", "missing_target_symbol", "csc's CS0246 for `using Nosuch;`, as dotnet-xunit's: the place rule decides (the owner, 2026-10-14)"),
    // the owner, 2026-10-14: exit 127 with a local path (the missing word contains '/') is the command
    ("googletest", "environment_error_cmd_not_found", "usage_error", "'sh: line 1: ./build/calc_testx: No such file or directory': a local path (the owner, 2026-10-14)"),
    ("pest", "environment_error_cmd_not_found", "usage_error", "'sh: vendor/bin/pestx: not found': a local path (the owner, 2026-10-14)"),
    ("pest", "environment_error_missing_dep", "usage_error", "'sh: vendor/bin/pest: not found': a local path (the owner, 2026-10-14)"),
    ("phpunit", "environment_error_cmd_not_found", "usage_error", "'sh: vendor/bin/phpunitx: not found': a local path (the owner, 2026-10-14)"),
    ("phpunit", "environment_error_missing_dep", "usage_error", "'sh: vendor/bin/phpunit: not found': a local path (the owner, 2026-10-14)"),
    // lane h0, 2026-10-19, changed by lane h4py: the test calls calc.mull, a typo; the table first read
    // tests_failed (the truth harness pinned a fake seal), now it reads the missing attribute
    ("pytest", "tb_short_typo", "missing_target_symbol", "pytest --tb=short with a typo (calc.mull) reads the missing module attribute: missing_target_symbol, the name mull is not the place's and the place rule decides (lane h4py, a definition change)"),
    // lane h4py (2026-10-19): the Python family's honest outcomes that no prefix names, and the readings that stay a card
    ("pytest", "tb_line_typo", "missing_target_symbol", "pytest --tb=line with a typo (calc.mull) reads the missing module attribute: missing_target_symbol, the name mull is not the place's and the place rule decides (lane h4py, a definition change)"),
    ("pytest", "tb_native_typo", "missing_target_symbol", "pytest --tb=native with a typo (calc.mull) reads the missing module attribute: missing_target_symbol, the name mull is not the place's and the place rule decides (lane h4py, a definition change)"),
    ("pytest", "tb_no_typo", "missing_target_symbol", "pytest --tb=no prints only the FAILED line, `AttributeError: module 'calc' has no ...` (the terminal width cuts the name): the exception is read as a missing attribute with no place, so the run is a card (lane h4py, a definition change)"),
    ("pytest", "passed_skipped", "unclassified", "one test passed and one was skipped (exit 0): `1 passed, 1 skipped in` and no row reads a skip as green, so it is unclassified (a card): a skipped planned test must never read as passed"),
    ("pytest", "passed_qq_warning", "unclassified", "-qq prints no summary line: a green with a warnings block has no positive marker, so it is unclassified (a card)"),
    ("pytest", "no_tests_collected_qq", "unclassified", "-qq with nothing selected prints nothing at exit 5: the exit code alone is not read as no_tests_collected (H-1), so it is unclassified (a card)"),
    // lane h4jvmnet, 2026-10-19 (A0 spike): a German-locale `dotnet test` prints VSTest's words translated; the tables read English, so it reads unclassified (a card) until DOTNET_CLI_UI_LANGUAGE=en is pinned (A3)
    ("dotnet-xunit", "passed_locale_de", "unclassified", "German VSTest words (Bestanden, erfolgreich, gesamt): no English row matches, exit 0 reads unclassified"),
    ("dotnet-xunit", "tests_failed_locale_de", "unclassified", "German VSTest words (Fehler!, Fehlermeldung:, Stapelverfolgung:): no English row matches"),
    ("dotnet-xunit", "tests_failed_exception_locale_de", "unclassified", "German VSTest words: no English row matches"),
    ("dotnet-xunit", "no_tests_collected_locale_de", "unclassified", "German VSTest words (kein Test verfügbar): no English row matches"),
    // lane h4rustgo, 2026-10-19: an unclassified reading is a card, the honest one where the output cannot say
    ("go-test", "passed_pipe_go_junit_report", "unclassified", "a pipe into go-junit-report prints nothing and exits 0 (no pipefail): can't tell; the folder name says the tests passed, the output cannot"),
    ("go-test", "tests_failed_exception_json", "unclassified", "`go test -json` wraps every line in an event: the `--- FAIL:` and `panic:` rows read line starts, which never occur in JSON"),
    ("go-test", "tests_failed_pipe_go_junit_report", "unclassified", "a pipe into go-junit-report prints nothing and exits 0 (no pipefail) even though a test failed: can't tell"),
    // lane h4cppphp, 2026-10-19: honest outcomes whose folder name says another
    ("ctest", "passed_quiet", "unclassified", "ctest -Q prints nothing on either stream at any exit (cmCTest.cxx: every line sits under `if (!Quiet)`): an empty run says neither pass nor fail nor nothing ran, so unclassified"),
    ("ctest", "tests_failed_quiet", "unclassified", "ctest -Q prints nothing on either stream at any exit (cmCTest.cxx: every line sits under `if (!Quiet)`): an empty run says neither pass nor fail nor nothing ran, so unclassified"),
    ("ctest", "no_tests_collected_quiet", "unclassified", "ctest -Q prints nothing on either stream at any exit (cmCTest.cxx: every line sits under `if (!Quiet)`): an empty run says neither pass nor fail nor nothing ran, so unclassified"),
    ("googletest", "passed_skipped", "unclassified", "gtest default format with a GTEST_SKIP() beside a pass: `[  SKIPPED ] 1 test, listed below:` at exit 0; a skipped test beside a pass is not a pass (D4, H-3): unclassified, a card"),
    ("catch2", "passed_teamcity", "unclassified", "-r teamcity at exit 0: testStarted/testFinished lines only, no totals; a testFailed would be exit 42 but no row reads the stream as a pass: unclassified (a card)"),
    ("catch2", "passed_automake", "unclassified", "-r automake at exit 0: `:test-result: PASS name` only, no totals; no row reads it as a pass: unclassified (a card)"),
    ("phpunit", "passed_compact_skipped", "unclassified", "`OK (2 tests, 1 assertion, 1 skipped)` at exit 0 under --compact: a skipped test beside a pass is not a pass (D4, H-3): unclassified, a card"),
    // lane h4cppphp, the change commit: skips beside a pass, which no table reads as a pass any more (D4, H-3)
    ("googletest", "passed_brief_skipped", "unclassified", "--gtest_brief prints `[  SKIPPED ] 1 test.` (no `listed below:`) beside `[  PASSED  ] 1 test.` at exit 0: a skip beside a pass is not a pass (D4, H-3), unclassified, a card"),
    ("catch2", "passed_skipped", "unclassified", "`test cases: 2 | 1 passed | 1 skipped` at exit 0: a skip beside a pass is not a pass (D4, H-3), unclassified, a card"),
    ("phpunit", "passed_skipped", "unclassified", "`OK, but some tests were skipped!` at exit 0: a skip beside a pass is not a pass (D4, H-3), unclassified, a card (not no_tests_collected, which the rules send to TestBroken)"),
    ("pest", "passed_skipped", "unclassified", "`Tests:    1 skipped, 1 passed (1 assertions)` at exit 0: a skip beside a pass is not a pass (D4, H-3), unclassified, a card"),
    // lane H5 (H-2): a shell script is read from the shell's own trace; where a recording has none, can't tell
    ("sh", "tests_failed", "unclassified", "exit 1 with nothing on stderr and no trace: what failed is not shown, so can't tell (H-2 replaces the 2026-10-15 row 'exit 1 with empty stderr: the test said no')"),
    ("sh", "tests_failed_sh_is_bash_bashenv_func_test", "unclassified", "recorded with BASH_ENV only, which bash read as sh ignores: no trace, can't tell; Familiar's run sets SHELLOPTS=xtrace for it (its _shellopts twin reads tests_failed)"),
    ("sh", "tests_failed_sh_is_bash_bashenv_grep", "unclassified", "recorded with BASH_ENV only, which bash read as sh ignores: no trace, can't tell; Familiar's run sets SHELLOPTS=xtrace for it (its _shellopts twin reads tests_failed)"),
    ("sh", "tests_failed_sh_is_bash_bashenv_grep_or_exit", "unclassified", "recorded with BASH_ENV only, which bash read as sh ignores: no trace, can't tell; Familiar's run sets SHELLOPTS=xtrace for it (its _shellopts twin reads tests_failed)"),
    ("sh", "tests_failed_sh_is_bash_bashenv_grep_sete", "unclassified", "recorded with BASH_ENV only, which bash read as sh ignores: no trace, can't tell; Familiar's run sets SHELLOPTS=xtrace for it (its _shellopts twin reads tests_failed)"),
    ("sh", "tests_failed_sh_is_bash_bashenv_if_not", "unclassified", "recorded with BASH_ENV only, which bash read as sh ignores: no trace, can't tell; Familiar's run sets SHELLOPTS=xtrace for it (its _shellopts twin reads tests_failed)"),
    ("sh", "missing_target_symbol_bash_env_notfound", "environment_error", "recorded under the A0 spike's kit, whose command_not_found_handle printed `bash: frobnicate_widget: command not found` with no file or line: exit 127 with a bare word is the environment (the owner, 2026-10-14); Familiar's kit has no handler, so bash's own `<file>: line N:` line stays (its _shellopts twin reads missing_target_symbol)"),
    ("sh", "missing_target_symbol_bash_shebang_notfound", "environment_error", "recorded under the A0 spike's kit, whose command_not_found_handle printed `bash: frobnicate_widget: command not found` with no file or line: exit 127 with a bare word is the environment (the owner, 2026-10-14); Familiar's kit has no handler, so bash's own `<file>: line N:` line stays (its _shellopts twin reads missing_target_symbol)"),
];

/// The two recordings whose text is the twin of another outcome's (plan S4): their table reads
/// them as the twin, and the place rule (§6.2) decides.
const TWINS: &[(&str, &str, &str)] = &[("jest", "setup_error", "tests_failed")];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/run_logs")
}

struct Fixture {
    runner: String,
    folder: String,
    end: End,
    stdout: String,
    stderr: String,
    report: String,
    cmd: String,
}

impl Fixture {
    fn streams(&self) -> Streams<'_> {
        Streams { stdout: &self.stdout, stderr: &self.stderr, report: &self.report }
    }

    fn expected(&self) -> &'static str {
        if let Some((_, _, o, _)) = EXPECT.iter().find(|(r, f, _, _)| *r == self.runner && *f == self.folder) {
            return o;
        }
        PREFIX.iter().find(|(p, _)| self.folder.starts_with(p)).map_or("?", |(_, o)| o)
    }
}

fn text(p: &Path) -> String {
    clean(&String::from_utf8_lossy(&std::fs::read(p).unwrap_or_default()))
}

/// The report files' text as the checker read it: each file under `report/`, in path order,
/// joined by a newline.
fn report(dir: &Path) -> String {
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
    files.iter().map(|p| text(p)).collect::<Vec<_>>().join("\n")
}

/// A shell recording's stderr and report as the binary's run keeps them (lane H5): where the
/// `sh` was not bash (dash, busybox; its version line says so), Familiar's run added `-x` and
/// moves the trace out of stderr into the report with the script's file and line
/// (`shell_trace::split_dash`, as `Trace::finish` does), the script read from `run_sources`.
fn as_kept(d: &Path, stderr: String, report: String) -> (String, String) {
    use familiar_ai_run::shell_trace::{sh_scripts, split_dash};
    let version = std::fs::read_to_string(d.join("version")).unwrap_or_default();
    if !stderr.lines().any(|l| l.starts_with("+familiar-ai\t")) || version.contains("GNU bash") {
        return (stderr, report);
    }
    let (runner, folder) = (d.parent().and_then(Path::file_name).unwrap_or_default(), d.file_name().unwrap_or_default());
    let sources = root().parent().map(|p| p.join("run_sources").join(runner).join(folder)).unwrap_or_default();
    let cmd = std::fs::read_to_string(d.join("cmd")).unwrap_or_default();
    let kept = split_dash(&stderr, &sh_scripts(cmd.trim(), &sources), &sources);
    (kept.stderr, format!("{report}{}", kept.trace))
}

fn fixtures() -> Vec<Fixture> {
    let mut out = Vec::new();
    let mut runners: Vec<PathBuf> = std::fs::read_dir(root()).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    runners.sort();
    for r in runners {
        let mut folders: Vec<PathBuf> = std::fs::read_dir(&r).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.join("exit").is_file()).collect();
        folders.sort();
        for d in folders {
            let code: i32 = std::fs::read_to_string(d.join("exit")).unwrap_or_default().trim().parse().unwrap_or_else(|_| panic!("{} has an exit code", d.display()));
            let end = if code < 0 { End::Signalled(-code) } else { End::Exited(code) };
            let (stderr, report) = as_kept(&d, text(&d.join("stderr")), report(&d));
            out.push(Fixture {
                runner: r.file_name().unwrap_or_default().to_string_lossy().into_owned(),
                folder: d.file_name().unwrap_or_default().to_string_lossy().into_owned(),
                end,
                stdout: text(&d.join("stdout")),
                stderr,
                report,
                cmd: std::fs::read_to_string(d.join("cmd")).unwrap_or_default().trim().to_string(),
            });
        }
    }
    out
}

/// The command as the recording ran it, without the recorder's docker wrapper
/// (`../../_bin/dock <image> '<command>'`).
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
fn every_recorded_run_reads_as_its_outcome_through_the_runner_its_command_names() {
    let all = fixtures();
    assert_eq!(all.len(), 604, "S0's recordings, sh's four, the --tb=short typo and lane h4py's 46");
    let mut wrong = Vec::new();
    for f in &all {
        let cmd = command(&f.cmd);
        let got = classify(f.end, &f.streams(), details_for(&cmd)).map(|t| t.label).unwrap_or_default();
        let want = TWINS.iter().find(|(r, d, _)| *r == f.runner && *d == f.folder).map_or(f.expected(), |(_, _, twin)| twin);
        if got != want {
            wrong.push(format!("{}/{}: want {want} got {got} [{cmd}]", f.runner, f.folder));
        }
    }
    assert!(wrong.is_empty(), "{} of {} misread:\n{}", wrong.len(), all.len(), wrong.join("\n"));
}

#[test]
fn exit_127_splits_by_the_missing_word_whatever_the_command_names() {
    for (cmd, line, want) in [
        ("pytestx", "sh: 1: pytestx: not found", "environment_error"),
        ("jestx", "/bin/sh: line 1: jestx: command not found", "environment_error"),
        ("./run_tests.sh", "sh: 1: ./run_tests.sh: not found", "usage_error"),
        ("vendor/bin/phpunitx", "sh: vendor/bin/phpunitx: not found", "usage_error"),
        ("pytest", "sh: 1: pytest: not found", "environment_error"),
        ("make test", "sh: 1: make: not found", "environment_error"),
    ] {
        let s = Streams { stderr: &format!("{line}\n"), ..Default::default() };
        let got = classify(End::Exited(127), &s, details_for(cmd)).expect("a reading");
        assert_eq!((got.label.as_str(), got.evidence.as_str()), (want, line), "{cmd}");
    }
}

/// Assumption 2 (lane l1): the example command is `python -m pytest`, so every pytest
/// recording, made with the bare `pytest` word, reads the same under `python -m` and
/// `python3 -m`: the same row, the same outcome.
#[test]
fn every_pytest_recording_reads_the_same_under_python_dash_m() {
    let pytest: Vec<Fixture> = fixtures().into_iter().filter(|f| f.runner == "pytest").collect();
    assert_eq!(pytest.len(), 65, "S0's pytest recordings, the --tb=short typo and lane h4py's 39");
    for f in &pytest {
        let cmd = command(&f.cmd);
        let bare = classify(f.end, &f.streams(), details_for(&cmd)).map(|t| t.label).unwrap_or_default();
        for python in ["python", "python3"] {
            let module = format!("{python} -m {cmd}");
            assert_eq!(details_for(&module).map(|d| d.words), details_for(&cmd).map(|d| d.words), "{}: {module}", f.folder);
            let got = classify(f.end, &f.streams(), details_for(&module)).map(|t| t.label).unwrap_or_default();
            assert_eq!(got, bare, "{}: {module}", f.folder);
        }
    }
}

#[test]
fn a_shell_script_names_the_sh_runner_and_make_and_bats_do_not() {
    // the owner, 2026-10-15
    let runner = |cmd: &str| details_for(cmd).map(|r| r.runner);
    for cmd in ["sh tests/test_c1.sh", "bash tests/run.sh", "dash t.sh", "zsh t.sh", "./tests/run.sh", "tests/check.sh", "for f in tests/*.sh; do sh \"$f\"; done", "set -e; for f in tests/*.sh; do if true; then bash \"$f\"; fi; done"] {
        assert_eq!(runner(cmd), Some("sh"), "{cmd}");
    }
    for cmd in ["make test", "bats tests/", "./run_tests", "pytest -q"] {
        assert_ne!(runner(cmd), Some("sh"), "{cmd}");
    }
    let silent = Streams::default();
    // H-2 (lane H5): a silent exit 1 with no trace does not show what failed
    assert_eq!(classify(End::Exited(1), &silent, details_for("sh tests/test_c1.sh")).map(|x| x.label).as_deref(), Some("unclassified"));
    assert_eq!(classify(End::Exited(1), &silent, details_for("make test")).map(|x| x.label).as_deref(), Some("unclassified"), "make is not sh");
}
