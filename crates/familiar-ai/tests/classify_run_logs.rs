//! Every recorded run (plan S0: 388 runs of 19 runners, `tests/fixtures/run_logs`) reads as its
//! outcome through its own runner's table (plan §2, §3, S4), with the semantics of wave 0's
//! checker: rows in order, the first match wins, then the exit defaults, then the contradiction
//! rule. The owner's two decisions of 2026-10-14 are in: CS0246 is a missing name in all three
//! .NET tables, and exit 127 is the environment for a bare command, the command for a local path.

use std::path::{Path, PathBuf};

use familiar_ai::door::kept::{classify, classify_with, clean, compiles, cut, read_table, table_named, tables_for, Streams, TABLES};
use familiar_ai_run::End;

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

/// The recording whose text is the twin of another outcome's (plan S4): its table reads it as the
/// twin, and the place rule (§6.2) decides. (go's missing-module twin is a card since lane h4rustgo.)
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
}

impl Fixture {
    fn streams(&self) -> Streams<'_> {
        Streams { stdout: &self.stdout, stderr: &self.stderr, report: &self.report }
    }

    fn code(&self) -> i64 {
        match self.end {
            End::Exited(c) => i64::from(c),
            End::Signalled(n) => -i64::from(n),
            _ => unreachable!(),
        }
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
            });
        }
    }
    out
}

fn fixture(runner: &str, folder: &str) -> Fixture {
    fixtures().into_iter().find(|f| f.runner == runner && f.folder == folder).unwrap_or_else(|| panic!("no fixture {runner}/{folder}"))
}

#[test]
fn every_pattern_of_every_table_compiles() {
    assert_eq!(TABLES.len(), 20, "one table per recorded runner, sh's included (the owner, 2026-10-15)");
    for t in TABLES {
        for i in 0..t.rows.len() {
            assert_eq!(compiles(t.runner, i, false), Some(true), "{} row {}: {}", t.runner, i + 1, t.rows[i].regex);
        }
        for i in 0..t.locs.len() {
            assert_eq!(compiles(t.runner, i, true), Some(true), "{} location row {}: {}", t.runner, i + 1, t.locs[i].regex);
        }
    }
}

#[test]
fn every_table_is_selected_by_its_runner_row() {
    for t in TABLES {
        let row = familiar_ai_run::RUNNER_DETAILS.iter().find(|r| r.runner == t.row);
        assert!(row.is_some(), "{}: no RUNNER_DETAILS row {}", t.runner, t.row);
        assert!(tables_for(row).iter().any(|x| x.runner == t.runner));
    }
    let cpp: Vec<&str> = tables_for(familiar_ai_run::details_for("cmake -S . -B build && cmake --build build && ./build/calc_test")).iter().map(|t| t.runner).collect();
    assert_eq!(cpp, ["catch2", "ctest", "googletest"], "a C++ command selects its family's three tables");
    let dotnet: Vec<&str> = tables_for(familiar_ai_run::details_for("dotnet test")).iter().map(|t| t.runner).collect();
    assert_eq!(dotnet, ["dotnet-mstest", "dotnet-nunit", "dotnet-xunit"]);
    let pytest: Vec<&str> = tables_for(familiar_ai_run::details_for("uv run pytest -q")).iter().map(|t| t.runner).collect();
    assert_eq!(pytest, ["pytest"]);
    assert_eq!(tables_for(familiar_ai_run::details_for("make test")).len(), 19, "no runner named: every table but sh's");
    assert!(tables_for(None).iter().all(|t| t.runner != "sh"), "make is not sh: sh's exit-1 row never reads another runner's output");
    let sh: Vec<&str> = tables_for(familiar_ai_run::details_for("sh tests/test_c1.sh")).iter().map(|t| t.runner).collect();
    assert_eq!(sh, ["sh"]);
}

#[test]
fn every_fixture_reads_as_its_outcome_through_its_own_runners_table() {
    let all = fixtures();
    assert_eq!(all.len(), 604, "S0's recordings, sh's four, the --tb=short typo and lane h4py's 46");
    let mut wrong = Vec::new();
    for f in &all {
        let Some(t) = table_named(&f.runner) else {
            wrong.push(format!("{}: no table", f.runner));
            continue;
        };
        let want = f.expected();
        let twin = TWINS.iter().find(|(r, d, _)| *r == f.runner && *d == f.folder).map(|(_, _, o)| *o);
        let got = classify_with(&[t], false, f.end, &f.streams()).map(|x| x.label).unwrap_or_default();
        if got != twin.unwrap_or(want) {
            wrong.push(format!("{}/{}: want {want} got {got}", f.runner, f.folder));
        }
        // no other row matches first: the deciding row's own outcome is the fixture's
        let r = read_table(t, f.code(), &f.streams());
        if let Some(i) = r.row {
            if t.rows[i].outcome != twin.unwrap_or(want) && r.label == t.rows[i].outcome {
                wrong.push(format!("{}/{}: row {} ({}) matches first", f.runner, f.folder, i + 1, t.rows[i].outcome));
            }
        }
    }
    assert!(wrong.is_empty(), "{} of {}:\n{}", wrong.len(), all.len(), wrong.join("\n"));
}

#[test]
fn the_two_text_twins_read_as_their_twins_and_the_place_rule_decides() {
    for (runner, folder, twin) in TWINS {
        let f = fixture(runner, folder);
        let t = table_named(runner).expect("its table");
        let got = classify_with(&[t], false, f.end, &f.streams()).expect("a reading");
        assert_eq!(got.label, *twin, "{runner}/{folder} prints the same text as {twin}");
        assert_ne!(f.expected(), *twin, "the folder names the honest outcome");
    }
}

#[test]
fn the_evidence_is_one_line_verbatim_from_the_text_searched() {
    for f in fixtures() {
        let t = table_named(&f.runner).expect("its table");
        let got = classify_with(&[t], false, f.end, &f.streams()).expect("a reading");
        assert!(!got.evidence.contains('\n'), "{}/{}: {:?}", f.runner, f.folder, got.evidence);
        if !got.evidence.is_empty() {
            let all = format!("{}\n{}\n{}", f.stdout, f.stderr, f.report);
            assert!(all.lines().any(|l| l == got.evidence), "{}/{}: {:?} is a whole line of the output", f.runner, f.folder, got.evidence);
        }
    }
}

#[test]
fn exit_127_is_the_environment_for_a_bare_command_and_the_command_for_a_local_path() {
    for t in TABLES {
        let bare = Streams { stderr: "sh: 1: pytest: not found\n", ..Default::default() };
        let got = classify_with(&[t], false, End::Exited(127), &bare).expect("a reading");
        assert_eq!((got.label.as_str(), got.evidence.as_str()), ("environment_error", "sh: 1: pytest: not found"), "{}", t.runner);
        let path = Streams { stderr: "sh: line 1: ./build/calc_testx: No such file or directory\n", ..Default::default() };
        let got = classify_with(&[t], false, End::Exited(127), &path).expect("a reading");
        assert_eq!((got.label.as_str(), got.evidence.as_str()), ("usage_error", "sh: line 1: ./build/calc_testx: No such file or directory"), "{}", t.runner);
        let interpreter = Streams { stderr: "env: ‘php’: No such file or directory\n", ..Default::default() };
        assert_eq!(classify_with(&[t], false, End::Exited(127), &interpreter).map(|x| x.label).as_deref(), Some("environment_error"), "{}: php is bare", t.runner);
    }
}

#[test]
fn cs0246_is_a_missing_name_in_all_three_dotnet_tables() {
    let f = fixture("dotnet-nunit", "collection_error_import");
    // the run as it would print in Familiar's copy of the commit
    let stdout = f.stdout.replace("/r/dotnet-nunit/collection_error_import/", "/tmp/familiar-ai-run-42-0/");
    let s = Streams { stdout: &stdout, stderr: &f.stderr, report: "" };
    for runner in ["dotnet-xunit", "dotnet-nunit", "dotnet-mstest"] {
        let t = table_named(runner).expect("its table");
        let got = classify_with(&[t], false, f.end, &s).expect("a reading");
        assert_eq!(got.label, "missing_target_symbol", "{runner}");
        assert_eq!(got.missing_name.as_deref(), Some("Nosuch"), "{runner}");
        let at = got.location.map(|l| (l.file_path, l.line));
        assert_eq!(at, Some(("tests/CalcTests.cs".to_string(), 1)), "{runner}: root-relative, the copy's prefix stripped");
    }
    let got = classify(f.end, &s, familiar_ai_run::details_for("dotnet test")).expect("a reading");
    assert_eq!((got.label.as_str(), got.missing_name.as_deref()), ("missing_target_symbol", Some("Nosuch")), "the three agree");
}

#[test]
fn a_failed_test_points_at_its_frame_root_relative() {
    let f = fixture("pytest", "tests_failed_exception");
    let t = table_named("pytest").expect("its table");
    let got = classify_with(&[t], false, f.end, &f.streams()).expect("a reading");
    assert_eq!(got.label, "tests_failed");
    let loc = got.location.expect("pytest's crash line");
    assert!(loc.file_path.ends_with(".py") && loc.line > 0, "{loc:?}");
}

#[test]
fn the_drain_and_cut_markers_match_no_pattern() {
    // each marker against a plain line of the same shape: a row that reads any text reads both
    let markers = [("[familiar: 123456 chars dropped]\n", "nothing to read here\n"), ("\n[familiar: 4194304 bytes dropped]\n", "\nnothing to read here\n")];
    for t in TABLES {
        for code in [0, 1, 2, 101, 139, -9] {
            for (m, plain) in markers {
                let at = |text: &'static str| [Streams { stdout: text, ..Default::default() }, Streams { stderr: text, ..Default::default() }, Streams { report: text, ..Default::default() }];
                for (s, p) in at(m).iter().zip(at(plain).iter()) {
                    let (got, want) = (read_table(t, code, s), read_table(t, code, p));
                    assert_eq!((got.row, got.label), (want.row, want.label), "{} at exit {code}: {m:?} decides nothing a plain line does not", t.runner);
                }
            }
        }
    }
}

#[test]
fn a_big_run_classified_on_full_finds_a_middle_line_the_cut_drops() {
    let mut found = Vec::new();
    for f in fixtures().into_iter().filter(|f| f.folder.starts_with("big_run")) {
        let t = table_named(&f.runner).expect("its table");
        let full = classify_with(&[t], false, f.end, &f.streams()).expect("a reading");
        let (so, se, re) = (cut(&f.stdout).0, cut(&f.stderr).0, cut(&f.report).0);
        let kept = Streams { stdout: &so, stderr: &se, report: &re };
        let short = classify_with(&[t], false, f.end, &kept).expect("a reading");
        let in_cut = format!("{so}\n{se}\n{re}").lines().any(|l| l == full.evidence);
        if full.label == "tests_failed" && short.label != full.label && !in_cut {
            found.push(format!("{}/{}: full {} ({:?}), cut {}", f.runner, f.folder, full.label, full.evidence, short.label));
        }
    }
    eprintln!("{}", found.join("\n"));
    assert!(found.iter().any(|f| f.starts_with("node-test/big_run: full tests_failed (\"not ok 301 - add 2-60\")")), "node's deciding line sits in the middle of the run: {found:?}");
}

#[test]
fn a_command_naming_no_runner_reads_every_table_and_needs_one_agreed_outcome() {
    // pytest's failed test: every table whose row matches says tests_failed, or the run is unclassified
    let f = fixture("pytest", "tests_failed");
    let agreed = classify(f.end, &f.streams(), None).expect("a reading");
    let pytest = classify(f.end, &f.streams(), familiar_ai_run::details_for("pytest")).expect("a reading");
    assert_eq!(pytest.label, "tests_failed");
    assert!(agreed.label == "tests_failed" || agreed.label == "unclassified", "{agreed:?}");
    // two tables that disagree: unclassified
    let mut disagree = None;
    for f in fixtures() {
        let readings: Vec<(&str, &str)> = TABLES.iter().map(|t| (t.runner, read_table(t, f.code(), &f.streams()))).filter(|(_, r)| r.row.is_some()).map(|(n, r)| (n, r.label)).collect();
        if readings.iter().any(|(_, l)| *l != readings[0].1) {
            disagree = Some((f, readings));
            break;
        }
    }
    let (f, readings) = disagree.expect("a run two tables read differently");
    assert_eq!(classify(f.end, &f.streams(), None).map(|x| x.label).as_deref(), Some("unclassified"), "{}/{}: {readings:?}", f.runner, f.folder);
    // no table's row matches: unclassified, never the exit default
    let quiet = Streams { stderr: "boom\n", ..Default::default() };
    assert!(TABLES.iter().all(|t| read_table(t, 139, &quiet).row.is_none()), "no row reads a bare line");
    assert_eq!(classify(End::Exited(139), &quiet, None).map(|x| x.label).as_deref(), Some("unclassified"));
    assert_eq!(classify(End::Exited(139), &quiet, familiar_ai_run::details_for("pytest")).map(|x| x.label).as_deref(), Some("crashed"), "a known runner keeps the default");
    assert_eq!(classify(End::Exited(139), &quiet, familiar_ai_run::details_for("dotnet test")).map(|x| x.label).as_deref(), Some("crashed"), "so does a family");
}

#[test]
fn familiars_own_ends_skip_the_patterns() {
    let s = Streams { stdout: "test_a ... ok\ntest_b ...\n", ..Default::default() };
    let pytest = familiar_ai_run::details_for("pytest");
    let timed = classify(End::TimedOut, &s, pytest).expect("a reading");
    assert_eq!((timed.by, timed.label.as_str(), timed.evidence.as_str()), ("binary", "timed_out", "test_b ..."), "the last line names the last test");
    for end in [End::EmptyCommand, End::NoDirectory] {
        let got = classify(end, &s, pytest).expect("a reading");
        assert_eq!((got.by, got.label.as_str()), ("binary", "usage_error"), "{end:?}");
    }
    assert_eq!(classify(End::Cancelled, &s, pytest), None);
}

#[test]
fn a_shell_test_reads_through_the_sh_table() {
    // the owner, 2026-10-15: a test that is a shell script has a runner of its own
    let t = table_named("sh").expect("sh's table");
    let read = |folder: &str| {
        let f = fixture("sh", folder);
        classify_with(&[t], false, f.end, &f.streams()).expect("a reading")
    };
    assert_eq!(read("passed").label, "passed");
    // H-2 (lane H5): a silent exit 1 does not show what failed; the shell's own trace does
    assert_eq!(read("tests_failed").label, "unclassified", "exit 1, nothing on stderr, no trace: can't tell");
    let traced = read("tests_failed_sh_is_bash_shellopts_grep");
    assert_eq!(traced.label, "tests_failed");
    assert_eq!(traced.location.map(|l| (l.file_path, l.line, l.kind)), Some(("posix/s_grep.sh".to_string(), 4, Some(familiar_ai_facts::LocationKind::Assertion))));
    let missing = read("missing_target_symbol");
    assert_eq!(missing.label, "missing_target_symbol");
    assert_eq!(missing.missing_name.as_deref(), Some("farewell"));
    assert_eq!(missing.location.map(|l| (l.file_path, l.line)), Some(("tests/test_app.sh".to_string(), 2)));
    assert_eq!(read("usage_error").label, "usage_error", "a syntax error at exit 2 is the command's");
    // exit 1 with words on stderr is not a silent no
    let loud = Streams { stderr: "grep: src/app.sh: No such file or directory\n", ..Default::default() };
    assert_eq!(classify_with(&[t], false, End::Exited(1), &loud).map(|x| x.label).as_deref(), Some("unclassified"));
    // the shell's own line for a command it could not start is not the script's
    let shell = Streams { stderr: "/bin/sh: line 1: jestx: command not found\n", ..Default::default() };
    assert_eq!(classify_with(&[t], false, End::Exited(127), &shell).map(|x| x.label).as_deref(), Some("environment_error"));
}

#[test]
fn dashs_own_lines_read_as_bashs_through_the_sh_table() {
    // hosted CI (2026-10-26): on Ubuntu `sh` is dash, whose own line for a syntax error or a missing
    // command is not bash's; each reads as its bash twin does: the same label, name and place, the
    // shell's own line the evidence (the lines are Ubuntu's dash's and bash 5's, verbatim)
    let t = table_named("sh").expect("sh's table");
    let read = |code: i32, line: &str| {
        let stderr = format!("{line}\n");
        let got = classify_with(&[t], false, End::Exited(code), &Streams { stderr: &stderr, ..Default::default() }).expect("a reading");
        (got.label, got.evidence, got.missing_name, got.location.map(|l| (l.file_path, l.line, l.kind)))
    };
    let twins = |code: i32, label: &str, bash: &str, dash: &str| {
        let (b, d) = (read(code, bash), read(code, dash));
        assert_eq!((b.0.as_str(), b.1.as_str()), (label, bash), "bash's line");
        assert_eq!((d.0.as_str(), d.1.as_str()), (label, dash), "dash's line {dash:?} reads as bash's {bash:?}");
        assert_eq!((&d.2, &d.3), (&b.2, &b.3), "dash's line {dash:?} names what bash's {bash:?} names, at its place");
        d
    };
    // a syntax error is the command's (exit 2): in a script the test sources, in the test script, in its eval, in sh -c
    twins(2, "usage_error", "src/app.sh: line 2: syntax error: unexpected end of file from `{' command on line 1", "tests/test_c1.sh: 2: src/app.sh: Syntax error: end of file unexpected (expecting \"}\")");
    twins(2, "usage_error", "tests/s.sh: line 3: syntax error near unexpected token `fi'", "tests/s.sh: 3: Syntax error: \"fi\" unexpected");
    twins(2, "usage_error", "tests/t4.sh: eval: line 1: syntax error near unexpected token `then'", "tests/t4.sh: 1: eval: Syntax error: \"then\" unexpected");
    twins(2, "usage_error", "sh: -c: line 1: syntax error near unexpected token `then'", "sh: 1: Syntax error: \"then\" unexpected");
    // a missing command in the test script is its missing name, at its line
    let missing = twins(127, "missing_target_symbol", "tests/t3.sh: line 3: farewell: command not found", "tests/t3.sh: 3: farewell: not found");
    assert_eq!((missing.2.as_deref(), missing.3), (Some("farewell"), Some(("tests/t3.sh".to_string(), 3, Some(familiar_ai_facts::LocationKind::Compile)))));
    // at the top of a script the test sources, dash names that script after the line's number
    let sourced = twins(127, "missing_target_symbol", "src/top.sh: line 1: nosuch_top: command not found", "tests/t2.sh: 1: src/top.sh: nosuch_top: not found");
    assert_eq!((sourced.2.as_deref(), sourced.3), (Some("nosuch_top"), Some(("src/top.sh".to_string(), 1, Some(familiar_ai_facts::LocationKind::Compile)))));
    // the shell's own -c: a bare command it could not start is the environment's
    twins(127, "environment_error", "sh: line 1: foo: command not found", "sh: 1: foo: not found");
}
