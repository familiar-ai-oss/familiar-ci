//! H-1, lane H0 (plan of record 2026-10-19): a reading is the truth or "can't tell". The truth
//! harness [draft name]: every recorded run (`tests/fixtures/run_logs`) and output never seen
//! (generated per runner) is read the way the binary reads it — `details_for` on its command →
//! `kept::classify` → the translation through `kept::test_log_translation` (the helper
//! `runner.rs` uses; the place kept is the first candidate the run's sources hold, stood in for by
//! `run_sources`) → a `familiar_ai_rules` view holding `place_chosen`, `test_planned`, the run and
//! its translation → the family the rules give it:
//!
//! - stage 1, the tests-first commit: `outcome::family_of`, at the right place and at a wrong one;
//! - stage 2, a fix commit after a seal on the runner's seal fixture: `outcome::result_family`, the
//!   reading `lifecycle::passed_there` asks Green of.
//!
//! A reading passes when it is the truth's family or `Unknown` (a card). Anything else is a
//! violation of one of the owner's four wrong verdicts (2026-10-19), or `other`:
//!
//! ```text
//!  stage 1 (wants an honest red)        stage 2 (wants green)
//!  (1) fake seal                        (2) false green
//!  (3) honest red refused               (4) pass read as a failure
//! ```
//!
//! `KNOWN` is the ratchet: the violations of today, exactly, each with why it is accepted. A fix
//! forces its rows out; a new violation fails. Since lane H6 it holds no fake seal and no false
//! green. At stage 2 only the green verdict is judged (a red read as another red family
//! is still an attempt, D3).
//!
//! Lane H3 adds what a reading stores beside its family: every translation carries `counts`
//! (D7), a kept location has the kind its truth says (`kind_of`), and `COUNTED` is what each
//! recorded run's own summary counted.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use familiar_ai::door::kept::{classify, clean, cut, in_project, test_log_translation, Streams};
use familiar_ai_facts::{append_raw, FactFields, Log};
use familiar_ai_rules::outcome::{family_of, result_family, route, Family, Route, Stage};
use familiar_ai_rules::View;
use familiar_ai_run::{details_for, End};
use serde_json::{json, Value};

/// What really happened in a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Truth {
    /// Every test ran and passed.
    Pass,
    /// An assertion of the planned test failed.
    Assertion,
    /// The code under test raised inside the place's function.
    Raised,
    /// The place's name does not exist yet.
    Missing,
    /// The test or its setup is broken (a collection error, a typo, nothing collected).
    Broken,
    /// The command is wrong.
    Command,
    /// The machine lacks something.
    Env,
    /// The process crashed.
    Crashed,
    /// Killed from outside, or the runner fell over: a retry may pass.
    Transient,
    /// The output cannot say: only "can't tell" is honest.
    Unknowable,
}
use Truth::{Assertion, Broken, Command, Crashed, Env, Missing, Pass, Raised, Transient, Unknowable};

/// Per runner: its planned test files, the place (file, the function its exception fixtures raise
/// in, the name its missing-name fixtures miss), the fixture the stage-2 seal is made on, and the
/// fixture whose sources stand in for git where a fixture has none of its own.
struct Layout {
    runner: &'static str,
    tests: &'static [&'static str],
    file: &'static str,
    symbol: &'static str,
    missing: &'static str,
    seal: &'static str,
    sources: &'static str,
}

const fn lay(runner: &'static str, tests: &'static [&'static str], file: &'static str, symbol: &'static str, missing: &'static str) -> Layout {
    Layout { runner, tests, file, symbol, missing, seal: "tests_failed", sources: "tests_failed_exception" }
}

const LAYOUT: &[Layout] = &[
    lay("cargo-nextest", &["tests/calc_test.rs"], "src/calc.rs", "parse", "mul"),
    lay("cargo-test", &["tests/calc_test.rs"], "src/calc.rs", "parse", "mul"),
    lay("catch2", &["tests/calc_test.cpp"], "src/calc.cpp", "parse", "mul"),
    lay("ctest", &["tests/calc_test.cpp"], "src/calc.cpp", "parse", "mul"),
    lay("dotnet-mstest", &["tests/CalcTests.cs"], "src/Calc.cs", "Parse", "Mul"),
    lay("dotnet-nunit", &["tests/CalcTests.cs"], "src/Calc.cs", "Parse", "Mul"),
    lay("dotnet-xunit", &["tests/CalcTests.cs"], "src/Calc.cs", "Parse", "Mul"),
    lay("go-test", &["calc_test.go"], "calc.go", "Parse", "Mul"),
    lay("googletest", &["tests/calc_test.cpp"], "src/calc.cpp", "parse", "mul"),
    lay("gradle", &["src/test/java/calc/CalcTest.java"], "src/main/java/calc/Calc.java", "parse", "mul"),
    lay("jest", &["test/calc.test.js", "test/calc.test.ts"], "src/calc.js", "parse", "mul"),
    lay("maven", &["src/test/java/calc/CalcTest.java"], "src/main/java/calc/Calc.java", "parse", "mul"),
    lay("mocha", &["test/calc.test.js", "test/calc.test.mjs"], "src/calc.js", "parse", "mul"),
    lay("node-test", &["test/calc.test.js", "test/calc.test.mjs"], "src/calc.js", "parse", "mul"),
    lay("pest", &["tests/CalcTest.php"], "src/Calc.php", "parse", "mul"),
    lay("phpunit", &["tests/CalcTest.php"], "src/Calc.php", "parse", "mul"),
    lay("pytest", &["tests/test_calc.py"], "src/calc.py", "parse", "mul"),
    // lane H5: the A0 spike's probes (tests/ run as bash, posix/ as sh) are planned tests too; the seal is a traced run
    Layout { runner: "sh", tests: SH_TESTS, file: "src/app.sh", symbol: "greet", missing: "farewell", seal: "tests_failed_sh_is_bash_shellopts_grep", sources: "missing_target_symbol" },
    lay("unittest", &["tests/test_calc.py"], "calc.py", "parse", "mul"),
    lay("vitest", &["test/calc.test.js", "test/calc.test.ts"], "src/calc.js", "parse", "mul"),
];

/// sh's planned test files: the recorded project's, and the A0 spike's probes (lane H5).
const SH_TESTS: &[&str] = &[
    "tests/test_app.sh",
    "tests/s_pass.sh",
    "tests/s_grep.sh",
    "tests/s_grep_or_exit.sh",
    "tests/s_grep_sete.sh",
    "tests/s_if_not.sh",
    "tests/s_func_test.sh",
    "tests/s_notfound.sh",
    "tests/s_cd.sh",
    "tests/s_cd_noe.sh",
    "posix/s_pass.sh",
    "posix/s_grep.sh",
    "posix/s_grep_or_exit.sh",
    "posix/s_grep_sete.sh",
    "posix/s_if_not.sh",
    "posix/s_func_test.sh",
    "posix/s_notfound.sh",
    "posix/s_cd.sh",
    "posix/s_cd_noe.sh",
];

/// A wrong place: the place's file, another function.
const ELSEWHERE: &str = "elsewhere";

/// The truth a folder's name says, by its prefix (as `classify_run_logs.rs`), longest first.
const PREFIX: &[(&str, Truth)] = &[
    ("passed", Pass),
    ("tests_failed_exception", Raised),
    ("tests_failed", Assertion),
    ("big_run", Assertion),
    ("via_ctest_tests_failed", Assertion),
    ("collection_error", Broken),
    ("missing_target_symbol", Missing),
    ("no_tests_collected", Broken),
    ("setup_error", Broken),
    ("usage_error", Command),
    ("environment_error", Env),
    ("runner_crash", Transient),
    ("killed_sigkill", Transient),
    ("killed_segv", Crashed),
    ("killed_abort", Crashed),
    ("killed_nil_deref", Crashed),
    ("killed_failfast", Crashed),
    // lane H5: a run only can't tell reads honestly (the A0 spike's `cd` probes)
    ("cant_tell", Unknowable),
];

/// Where a case's truth is not its prefix's: runner, case, truth, why.
const TRUTH: &[(&str, &str, Truth, &str)] = &[
    // classify_run_logs.rs EXPECT: the folder records the provocation, not the outcome
    ("dotnet-mstest", "killed_segv_sent_by_kill", Pass, "the .NET runtime absorbed the signal; both tests ran and passed, exit 0"),
    ("dotnet-nunit", "killed_segv_sent_by_kill", Pass, "the .NET runtime absorbed the signal; both tests ran and passed, exit 0"),
    ("dotnet-xunit", "killed_segv_sent_by_kill", Pass, "the .NET runtime absorbed the signal; both tests ran and passed, exit 0"),
    ("googletest", "usage_error", Pass, "gtest ignores --nosuchflag without a word and every test passes"),
    ("mocha", "usage_error", Pass, "mocha 12 ignores --nosuchflag; the one test ran and passed"),
    ("googletest", "usage_error_gtest_flag", Command, "an unknown --gtest_ flag prints the help and runs nothing: the command's mistake"),
    ("catch2", "setup_error", Unknowable, "a fixture's constructor throwing prints the same text as the code under test throwing before the REQUIRE"),
    ("dotnet-nunit", "runner_crash_adapter", Broken, "the test project lost its NUnit adapter: nothing crashed, no test is found"),
    ("gradle", "killed_exit", Crashed, "System.exit(3) inside a test: the forked JVM ended itself"),
    ("maven", "killed_exit", Crashed, "System.exit(3) inside a test: the forked JVM ended itself"),
    ("go-test", "killed_nil_deref", Broken, "the test body dereferences nil (calc_test.go:9): the test is broken"),
    ("jest", "runner_crash", Env, "the testEnvironment package is not installed"),
    ("node-test", "runner_crash", Command, "a --test-reporter path that does not exist: the command's own error"),
    ("catch2", "environment_error_cmd_not_found", Command, "./build/calc_testx, a typo of the project's own binary: a local path (the owner, 2026-10-14)"),
    ("googletest", "environment_error_cmd_not_found", Command, "a local path (the owner, 2026-10-14)"),
    ("pest", "environment_error_cmd_not_found", Command, "a local path (the owner, 2026-10-14)"),
    ("pest", "environment_error_missing_dep", Command, "a local path (the owner, 2026-10-14)"),
    ("phpunit", "environment_error_cmd_not_found", Command, "a local path (the owner, 2026-10-14)"),
    ("phpunit", "environment_error_missing_dep", Command, "a local path (the owner, 2026-10-14)"),
    // H-3, D4: a skipped test beside a pass is not a pass
    ("pytest", "passed_skipped_pretty", Unknowable, "one test passed and one was skipped: at stage 1 a skip beside a pass can't tell (D4); at stage 2 the seal run skipped none (H-3)"),
    // lane h4jvmnet, 2026-10-19 (A0 spike recordings): a German-locale `dotnet test` prints VSTest's words translated
    // ("Fehler!", "Fehlermeldung:", "Stapelverfolgung:"); DOTNET_CLI_UI_LANGUAGE is not pinned yet (A3), so only can't tell is honest
    ("dotnet-xunit", "passed_locale_de", Unknowable, "German VSTest words (Bestanden, erfolgreich): the tables read English; only can't tell is honest until DOTNET_CLI_UI_LANGUAGE=en is pinned"),
    ("dotnet-xunit", "tests_failed_locale_de", Unknowable, "German VSTest words (Fehler, Fehlermeldung, Stapelverfolgung): the tables read English; only can't tell is honest until DOTNET_CLI_UI_LANGUAGE=en is pinned"),
    ("dotnet-xunit", "tests_failed_exception_locale_de", Unknowable, "German VSTest words: the tables read English; only can't tell is honest until DOTNET_CLI_UI_LANGUAGE=en is pinned"),
    ("dotnet-xunit", "no_tests_collected_locale_de", Unknowable, "German VSTest words (kein Test verfügbar): the tables read English; only can't tell is honest until DOTNET_CLI_UI_LANGUAGE=en is pinned"),
    // lane H4js (2026-10-19): the JS runners' reporters and skips. A run whose reporter prints nothing
    // says nothing, red or green: only "can't tell" is honest (the machine report is a file nobody reads)
    ("jest", "tests_failed_reporters_junit", Unknowable, "--reporters=jest-junit prints nothing to the console (junit.xml is a file): a red says nothing"),
    ("jest", "passed_reporters_junit", Unknowable, "--reporters=jest-junit prints nothing: exit 0 with an empty console says nothing (H-1)"),
    ("jest", "passed_reporters_silent", Unknowable, "jest-silent-reporter prints only failure blocks and no summary: a green prints nothing"),
    ("jest", "tests_failed_reporters_github_actions", Unknowable, "--reporters=github-actions alone prints nothing outside Actions (its `silent` option defaults true): a red says nothing"),
    ("jest", "passed_reporters_github_actions", Unknowable, "--reporters=github-actions alone prints nothing: exit 0 with an empty console says nothing (H-1)"),
    ("jest", "passed_skipped", Unknowable, "one test passed and one was skipped: at stage 1 a skip beside a pass can't tell (D4); at stage 2 the seal run skipped none (H-3)"),
    ("vitest", "passed_skipped", Unknowable, "one test passed and one was skipped: at stage 1 a skip beside a pass can't tell (D4); at stage 2 the seal run skipped none (H-3)"),
    ("mocha", "passed_skipped", Unknowable, "one test passed and one was pending: at stage 1 a skip beside a pass can't tell (D4); at stage 2 the seal run skipped none (H-3)"),
    ("node-test", "passed_skipped", Unknowable, "one test passed and one was skipped: at stage 1 a skip beside a pass can't tell (D4); at stage 2 the seal run skipped none (H-3)"),
    ("node-test", "passed_skipped_spec", Unknowable, "one test passed and one was skipped: at stage 1 a skip beside a pass can't tell (D4); at stage 2 the seal run skipped none (H-3)"),
    // the owner, 2026-10-19: the reproduced fake seal
    ("pytest", "tb_short_typo", Broken, "the test calls calc.mull, a typo of add and not the place's name: the test is broken, whatever --tb prints"),
    // lane h4py (2026-10-19): the Python family's research cases
    ("pytest", "tb_line_typo", Broken, "the test calls calc.mull, a typo of add and not the place's name: the test is broken, whatever --tb prints"),
    ("pytest", "tb_native_typo", Broken, "the test calls calc.mull, a typo of add and not the place's name: the test is broken, whatever --tb prints"),
    ("pytest", "tb_no_typo", Broken, "the test calls calc.mull, a typo of add and not the place's name: the test is broken, whatever --tb prints"),
    ("pytest", "missing_target_symbol_wrong_module", Broken, "the test calls os.mul: the missing name is the place's but the module is not the place's file, so the test is broken"),
    ("unittest", "missing_target_symbol_wrong_module", Broken, "the test calls os.mul: the missing name is the place's but the module is not the place's file, so the test is broken"),
    ("unittest", "missing_target_symbol_typo", Broken, "the test calls calc.mull, a typo of add and not the place's name: the test is broken"),
    ("pytest", "missing_target_symbol_mixed_typo", Broken, "one test fails an assertion and another is a typo (calc.mull): a test of the file is broken, whatever the other one does"),
    ("unittest", "missing_target_symbol_mixed_typo", Broken, "one test fails an assertion and another is a typo (calc.mull): a test of the file is broken, whatever the other one does"),
    ("pytest", "passed_skipped", Unknowable, "one test passed and one was skipped: at stage 1 a skip beside a pass can't tell (D4); at stage 2 the seal run skipped none (H-3)"),
    ("unittest", "passed_skipped", Unknowable, "one test passed and one was skipped: at stage 1 a skip beside a pass can't tell (D4); at stage 2 the seal run skipped none (H-3)"),
    // lane h4rustgo (2026-10-19), D1: a panic with no assertion text, at the planned test's own line, is an exception there
    ("cargo-test", "tests_failed_exception_unwrap_in_test", Unknowable, "the test unwraps the Err of a stub (`parse_checked` returns Err(\"todo\")) at tests/calc_test.rs:5, an honest TDD red; the panic has no assertion text and sits at the test line: only can't tell is honest (D1, H-5)"),
    ("cargo-test", "tests_failed_exception_assert_message", Unknowable, "assert!(cond, \"add is wrong\") panics with the custom text alone at tests/calc_test.rs:5, the shape of any panic!(\"msg\"): can't tell (D1, H-5)"),
    ("cargo-nextest", "tests_failed_exception_unwrap_in_test", Unknowable, "the test unwraps the Err of a stub at tests/calc_test.rs:5, an honest TDD red; no assertion text at the test line: can't tell (D1, H-5)"),
    ("cargo-nextest", "tests_failed_exception_assert_message", Unknowable, "assert!(cond, \"add is wrong\") panics with the custom text alone at tests/calc_test.rs:5: can't tell (D1, H-5)"),
    // lane h4rustgo: a pipe's exit is the last command's
    ("go-test", "passed_pipe_go_junit_report", Unknowable, "`go test -v ./... | go-junit-report -out report.xml` prints nothing and exits 0 whether the tests passed or failed (no pipefail): only can't tell is honest"),
    ("go-test", "tests_failed_pipe_go_junit_report", Unknowable, "the same pipe with a failing test: nothing printed, exit 0, the failure is not visible"),
    // lane h4cppphp, 2026-10-19: ctest, googletest, catch2, phpunit, pest
    ("ctest", "passed_quiet", Unknowable, "ctest -Q prints nothing at any outcome (cmCTest.cxx: every line sits under `if (!Quiet)`): an empty exit 0 says neither that a test passed nor that none ran"),
    ("ctest", "tests_failed_quiet", Unknowable, "ctest -Q prints nothing at any outcome: an empty exit 8 says nothing of why"),
    ("ctest", "no_tests_collected_quiet", Unknowable, "ctest -Q with no test registered prints nothing, as it does for a pass: the empty exit 0 cannot tell them apart"),
    ("googletest", "passed_brief_skipped", Unknowable, "a skip beside a pass: at stage 1 can't tell (D4), at stage 2 the seal run skipped none (H-3)"),
    ("googletest", "passed_skipped", Unknowable, "a skip beside a pass: at stage 1 can't tell (D4), at stage 2 the seal run skipped none (H-3)"),
    ("ctest", "G4 tests_failed_exception cut", Crashed, "cut as the backstop reads it there is no LastTest.log: ctest says only `Subprocess aborted***Exception`, which an uncaught throw and an abort() print alike"),
    ("catch2", "passed_skipped", Unknowable, "a SKIP() beside a pass: at stage 1 can't tell (D4), at stage 2 the seal run skipped none (H-3)"),
    ("phpunit", "passed_compact_skipped", Unknowable, "a skipped test beside a pass: at stage 1 can't tell (D4), at stage 2 the seal run skipped none (H-3)"),
    ("phpunit", "passed_skipped", Unknowable, "a skipped test beside a pass: at stage 1 can't tell (D4), at stage 2 the seal run skipped none (H-3)"),
    ("pest", "passed_skipped", Unknowable, "a skipped test beside a pass: at stage 1 can't tell (D4), at stage 2 the seal run skipped none (H-3)"),
    // lane H5: the A0 spike's not-found probe calls frobnicate_widget, a tool, not the place's name
    ("sh", "missing_target_symbol_bash_env_notfound", Broken, "the probe calls frobnicate_widget, which is not the place's name: the test is broken"),
    ("sh", "missing_target_symbol_bash_shebang_notfound", Broken, "the probe calls frobnicate_widget, which is not the place's name: the test is broken"),
    ("sh", "missing_target_symbol_busybox_x_notfound", Broken, "the probe calls frobnicate_widget, which is not the place's name: the test is broken"),
    ("sh", "missing_target_symbol_dash_x_notfound", Broken, "the probe calls frobnicate_widget, which is not the place's name: the test is broken"),
    ("sh", "missing_target_symbol_sh_is_bash_bashenv_notfound", Broken, "the probe calls frobnicate_widget, which is not the place's name: the test is broken"),
    ("sh", "missing_target_symbol_sh_is_bash_shellopts_notfound", Broken, "the probe calls frobnicate_widget, which is not the place's name: the test is broken"),
];

/// The violations of today: runner, case, each stage that is violated with its class, and why it
/// is accepted. Lane H6 took out every fake seal (1) and false green (2); a row left is a run sent
/// to a card or refused under another family's name, never a seal or a green.
type Known = (&'static str, &'static str, &'static [(&'static str, &'static str)], &'static str);
const KNOWN: &[Known] = &[
    // lane H4js: a skip beside a pass reads passed today; the rules (H6: D4 at stage 1, H-3 at stage 2, from `counts`) decide it, not a row
    // lane H4js: jest prints a beforeAll throw byte-for-byte like a test-body throw (only the code frame differs), so no row can
    // separate the twin; D1 (H6: an exception located in a planned test file can't tell) is what takes this row out
    ("sh", "G4 missing_target_symbol_bash_env_notfound cut", &[("1 right", "other"), ("1 wrong", "other")], "a bash `command not found` (exit 127) from a sh test script reads as the environment's missing tool (the owner's 2026-10-14 127 split) before H-2's missing-name rule; a card or a refusal, never a seal or a green; H5 and the 127 rule meet later"),
    ("sh", "G4 missing_target_symbol_bash_shebang_notfound cut", &[("1 right", "other"), ("1 wrong", "other")], "a bash `command not found` (exit 127) from a sh test script reads as the environment's missing tool (the owner's 2026-10-14 127 split) before H-2's missing-name rule; a card or a refusal, never a seal or a green; H5 and the 127 rule meet later"),
    ("sh", "missing_target_symbol_bash_env_notfound", &[("1 right", "other"), ("1 wrong", "other")], "a bash `command not found` (exit 127) from a sh test script reads as the environment's missing tool (the owner's 2026-10-14 127 split) before H-2's missing-name rule; a card or a refusal, never a seal or a green; H5 and the 127 rule meet later"),
    ("sh", "missing_target_symbol_bash_shebang_notfound", &[("1 right", "other"), ("1 wrong", "other")], "a bash `command not found` (exit 127) from a sh test script reads as the environment's missing tool (the owner's 2026-10-14 127 split) before H-2's missing-name rule; a card or a refusal, never a seal or a green; H5 and the 127 rule meet later"),
];

const FAKE_SEAL: &str = "(1) fake seal";
const FALSE_GREEN: &str = "(2) false green";
const RED_REFUSED: &str = "(3) honest red refused";
const PASS_FAILED: &str = "(4) pass read as a failure";
const OTHER: &str = "other";

const RIGHT: &str = "1 right";
const WRONG: &str = "1 wrong";
const FIX: &str = "2";

/// Where the recorder ran each fixture (`location_per_runner.rs`); each becomes the run copy.
const RECORDED_AT: &[&str] = &["/tmp/claude-1000/-home-user-projects-familiar-ai-oss/ce46f81f-ec40-4c7d-885d-547ef9d911b3/scratchpad/run_logs", "/r"];
const COPY: &str = "/tmp/familiar-ai-run-4242-7";

/// The exits a generated case is read at, and their truth when the output says nothing (H-1: the
/// exit code alone decides only a signal's).
const EXITS: &[(i32, Truth)] = &[
    (0, Unknowable),
    (1, Unknowable),
    (2, Unknowable),
    (3, Unknowable),
    (4, Unknowable),
    (5, Unknowable),
    (8, Unknowable),
    (42, Unknowable),
    (100, Unknowable),
    (101, Unknowable),
    (134, Crashed),
    (139, Crashed),
    (137, Transient),
    (143, Transient),
];

const JUNK_OUT: &str = "starting the suite\nsomething happened here\nall done\n";
const JUNK_ERR: &str = "warning: a line nobody wrote a row for\n";
const CUT_MARKER: &str = "[familiar: 123456 chars dropped]\n";
/// How many of a stream's last lines that are not blank G3 keeps as the runner's summary.
const SUMMARY_LINES: usize = 5;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

struct Case {
    runner: String,
    id: String,
    end: End,
    stdout: String,
    stderr: String,
    report: String,
    cmd: String,
    /// The `run_sources` folder that stands in for git at the run's commit.
    sources: PathBuf,
    truth: Truth,
    generated: bool,
}

fn layout(runner: &str) -> &'static Layout {
    LAYOUT.iter().find(|l| l.runner == runner).unwrap_or_else(|| panic!("{runner} has a LAYOUT row"))
}

/// The text as a run copy prints it: each recording root of this fixture becomes the copy.
fn text(p: &Path, runner: &str, folder: &str) -> String {
    let mut t = clean(&String::from_utf8_lossy(&std::fs::read(p).unwrap_or_default()));
    for at in RECORDED_AT {
        t = t.replace(&format!("{at}/{runner}/{folder}/"), &format!("{COPY}/")).replace(&format!("{at}/{runner}/{folder}"), COPY);
    }
    t
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

fn override_of(runner: &str, case: &str) -> Option<Truth> {
    TRUTH.iter().find(|(r, c, _, _)| *r == runner && *c == case).map(|(_, _, t, _)| *t)
}

fn sources_of(runner: &str, folder: &str) -> PathBuf {
    let own = root().join("run_sources").join(runner).join(folder);
    if own.is_dir() {
        own
    } else {
        root().join("run_sources").join(runner).join(layout(runner).sources)
    }
}

fn fixtures() -> Vec<Case> {
    let mut out = Vec::new();
    let mut runners: Vec<PathBuf> = std::fs::read_dir(root().join("run_logs")).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    runners.sort();
    for r in runners {
        let mut folders: Vec<PathBuf> = std::fs::read_dir(&r).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.join("exit").is_file()).collect();
        folders.sort();
        for d in folders {
            let runner = r.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let folder = d.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let code: i32 = std::fs::read_to_string(d.join("exit")).unwrap_or_default().trim().parse().unwrap_or_else(|_| panic!("{} has an exit code", d.display()));
            let truth = override_of(&runner, &folder)
                .or_else(|| PREFIX.iter().find(|(p, _)| folder.starts_with(p)).map(|(_, t)| *t))
                .unwrap_or_else(|| panic!("{runner}/{folder}: no TRUTH row and no prefix"));
            let cmd = command(std::fs::read_to_string(d.join("cmd")).unwrap_or_default().trim());
            let sources = sources_of(&runner, &folder);
            let (stderr, report) = as_kept(&d, &cmd, &sources, text(&d.join("stderr"), &runner, &folder), report(&d, &runner, &folder));
            out.push(Case {
                end: if code < 0 { End::Signalled(-code) } else { End::Exited(code) },
                stdout: text(&d.join("stdout"), &runner, &folder),
                stderr,
                report,
                cmd,
                sources,
                truth,
                generated: false,
                id: folder,
                runner,
            });
        }
    }
    out
}

/// A shell recording's stderr and report as the binary's run keeps them (lane H5): where the
/// `sh` was not bash (dash, busybox; its version line says so), Familiar's run added `-x` and
/// moves the trace out of stderr into the report with the script's file and line
/// (`shell_trace::split_dash`, as `Trace::finish` does).
fn as_kept(dir: &Path, cmd: &str, sources: &Path, stderr: String, report: String) -> (String, String) {
    use familiar_ai_run::shell_trace::{sh_scripts, split_dash};
    let version = std::fs::read_to_string(dir.join("version")).unwrap_or_default();
    if !stderr.lines().any(|l| l.starts_with("+familiar-ai\t")) || version.contains("GNU bash") {
        return (stderr, report);
    }
    let kept = split_dash(&stderr, &sh_scripts(cmd, sources), sources);
    (kept.stderr, format!("{report}{}", kept.trace))
}

/// The last `n` lines of `s` that are not blank, each with its newline.
fn last_lines(s: &str, n: usize) -> String {
    let mut lines: Vec<&str> = s.lines().rev().filter(|l| !l.trim().is_empty()).take(n).collect();
    lines.reverse();
    lines.iter().map(|l| format!("{l}\n")).collect()
}

/// H-2: a shell script's exit 0 is its green.
fn generated_truth(runner: &str, code: i32, said: Truth) -> Truth {
    if runner == "sh" && code == 0 {
        return Pass;
    }
    said
}

/// G1 empty, G2 junk and the cut marker alone, G3 the runner's own summary then junk, G4 every
/// fixture cut as the backstop reads it.
fn generated(all: &[Case]) -> Vec<Case> {
    let mut out = Vec::new();
    for l in LAYOUT {
        let Some(seal) = all.iter().find(|c| c.runner == l.runner && c.id == l.seal) else { continue };
        let at = |id: String, code: i32, stdout: String, stderr: String, truth: Truth| {
            let truth = override_of(l.runner, &id).unwrap_or(generated_truth(l.runner, code, truth));
            Case { runner: l.runner.to_string(), id, end: End::Exited(code), stdout, stderr, report: String::new(), cmd: seal.cmd.clone(), sources: sources_of(l.runner, l.sources), truth, generated: true }
        };
        for (code, truth) in EXITS {
            out.push(at(format!("G1 empty exit {code}"), *code, String::new(), String::new(), *truth));
            out.push(at(format!("G2 junk exit {code}"), *code, JUNK_OUT.to_string(), JUNK_ERR.to_string(), *truth));
            out.push(at(format!("G2 cut marker exit {code}"), *code, CUT_MARKER.to_string(), String::new(), *truth));
        }
        for folder in ["passed", "tests_failed"] {
            let Some(f) = all.iter().find(|c| c.runner == l.runner && c.id == folder) else { continue };
            let End::Exited(own) = f.end else { continue };
            let mut codes: Vec<i32> = EXITS.iter().filter(|(_, t)| *t == Unknowable).map(|(c, _)| *c).collect();
            if !codes.contains(&own) {
                codes.push(own);
            }
            for code in codes {
                let truth = if code == own { f.truth } else { Unknowable };
                let stdout = format!("{}{JUNK_OUT}", last_lines(&f.stdout, SUMMARY_LINES));
                out.push(at(format!("G3 {folder} summary then junk exit {code}"), code, stdout, last_lines(&f.stderr, SUMMARY_LINES), truth));
            }
        }
    }
    for f in all {
        let (so, se) = (cut(&f.stdout).0, cut(&f.stderr).0);
        if so == f.stdout && se == f.stderr && f.report.is_empty() {
            continue;
        }
        let id = format!("G4 {} cut", f.id);
        let truth = override_of(&f.runner, &id).unwrap_or(f.truth);
        out.push(Case { runner: f.runner.clone(), id, end: f.end, stdout: so, stderr: se, report: String::new(), cmd: f.cmd.clone(), sources: f.sources.clone(), truth, generated: true });
    }
    out
}

fn raw(log: &Log, kind: &str, fields: Value) -> Log {
    let Value::Object(m) = fields else { unreachable!() };
    append_raw(log, kind, m, log.len() as f64 + 1.0, "h0").unwrap_or_else(|e| panic!("{kind}: {e:?}"))
}

/// The translation's fields as the binary writes them for `c` at `commit`.
fn translation(c: &Case, run_id: &str, commit: &str) -> Option<Value> {
    let streams = Streams { stdout: &c.stdout, stderr: &c.stderr, report: &c.report };
    let t = classify(c.end, &streams, details_for(&c.cmd))?;
    let dir = c.sources.clone();
    let read = move |p: &str| std::fs::read(dir.join(p)).ok();
    let t = test_log_translation(t, run_id, "c1", commit, "full", |cands| in_project(cands, &read));
    Some(Value::Object(t.to_fields().expect("a translation's fields")))
}

/// The plan: c1's place and its planned tests.
fn planned(l: &Layout, symbol: &str) -> Log {
    let mut log = raw(&Log::new(), "place_chosen", json!({"criterion_id": "c1", "places": [{"file_path": l.file, "symbol": symbol, "line_start": 1, "line_end": 1000}]}));
    for t in l.tests {
        log = raw(&log, "test_planned", json!({"criterion_id": "c1", "file_path": t, "will": "create"}));
    }
    log
}

/// The binary's run of c1's command at `commit`: reserved, translated, its results.
fn ran(log: &Log, l: &Layout, run_id: &str, commit: &str, cmd: &str, t: Value) -> Log {
    let passed = t.get("label").and_then(Value::as_str) == Some("passed");
    let mut log = raw(log, "run_reserved", json!({"run_id": run_id, "criterion_id": "c1", "criterion_rev": "R1", "commit": commit, "command": cmd, "resources": ["tests"], "wait": "60", "by": "binary"}));
    log = raw(&log, "test_log_translation", t);
    for f in l.tests {
        log = raw(&log, "pass_please_pass", json!({"criterion_id": "c1", "file_path": f, "commit": commit, "outcome": if passed { "passed" } else { "failed" }, "ran_in": "laptop", "with_anchor_reverted": false, "run_id": run_id}));
    }
    log
}

fn want(truth: Truth, right: bool) -> Family {
    match truth {
        Pass => Family::Green,
        Assertion => Family::ValidRed,
        Raised | Missing if right => Family::ValidRed,
        Raised | Missing | Broken => Family::TestBroken,
        Command => Family::CommandBroken,
        Env => Family::NotTheAgents,
        Crashed => Family::Crashed,
        Transient => Family::Transient,
        Unknowable => Family::Unknown,
    }
}

/// Stage 1's verdict: `None` when the reading is the truth or can't tell. (3) is an honest red
/// the rules refuse (back to writing the test); one sent to a card is `other`.
fn stage_one(got: Family, want: Family) -> Option<&'static str> {
    if got == want || got == Family::Unknown {
        None
    } else if got == Family::ValidRed {
        Some(FAKE_SEAL)
    } else if want == Family::ValidRed && route(got, Stage::BeforeSeal) == Route::Refuse {
        Some(RED_REFUSED)
    } else {
        Some(OTHER)
    }
}

/// Stage 2's verdict on green alone.
fn stage_two(got: Family, want: Family) -> Option<&'static str> {
    match (got == Family::Green, want == Family::Green) {
        _ if got == Family::Unknown => None,
        (true, false) => Some(FALSE_GREEN),
        (false, true) => Some(PASS_FAILED),
        _ => None,
    }
}

#[derive(Default)]
struct Tally {
    truth: usize,
    unknown: usize,
    wrong: usize,
}

impl Tally {
    fn add(&mut self, got: Family, verdict: Option<&str>) {
        match (verdict, got) {
            (Some(_), _) => self.wrong += 1,
            (None, Family::Unknown) => self.unknown += 1,
            (None, _) => self.truth += 1,
        }
    }
}

#[test]
fn every_reading_is_the_truth_or_cant_tell_and_todays_violations_are_known() {
    let all = fixtures();
    assert_eq!(all.len(), 604, "S0's recordings, sh's four, the pretty three, the --tb=short typo and lane h4py's 46 (pytest 39, unittest 7)");
    for (r, c, _, why) in TRUTH {
        assert!(!why.is_empty(), "{r}/{c}: a TRUTH override says why");
    }
    let gen = generated(&all);
    // (runner, case, stage, class) → the family read
    let mut found: BTreeMap<(String, String, &str, &str), Family> = BTreeMap::new();
    let mut tallies: BTreeMap<(String, &str, bool), Tally> = BTreeMap::new();
    for c in all.iter().chain(gen.iter()) {
        let l = layout(&c.runner);
        let seal = all.iter().find(|s| s.runner == c.runner && s.id == l.seal).unwrap_or_else(|| panic!("{}: the seal fixture {}", c.runner, l.seal));
        let Some(t) = translation(c, "r1", "b2") else {
            panic!("{}/{}: every case ends with a reading", c.runner, c.id);
        };
        // stage 1, right and wrong place
        for (stage, symbol, right) in [(RIGHT, if c.id.starts_with("missing_target_symbol") || c.truth == Missing { l.missing } else { l.symbol }, true), (WRONG, ELSEWHERE, false)] {
            let log = ran(&planned(l, symbol), l, "r1", "b2", &c.cmd, t.clone());
            let v = View::new(&log);
            let fact = log.iter().rev().find(|f| f.kind == "test_log_translation").expect("the translation");
            let got = family_of(&v, fact, "c1");
            let verdict = stage_one(got, want(c.truth, right));
            tallies.entry((c.runner.clone(), stage, c.generated)).or_default().add(got, verdict);
            if let Some(class) = verdict {
                found.insert((c.runner.clone(), c.id.clone(), stage, class), got);
            }
        }
        // stage 2: sealed on the seal fixture's run at a1, this run at b2
        let sealed = translation(seal, "r0", "a1").expect("the seal's reading");
        let log = ran(&planned(l, l.symbol), l, "r0", "a1", &seal.cmd, sealed);
        let log = ran(&log, l, "r1", "b2", &c.cmd, t);
        let v = View::new(&log);
        let r = log.iter().rev().find(|f| f.kind == "pass_please_pass").expect("the result");
        let got = result_family(&v, "c1", r);
        let verdict = stage_two(got, want(c.truth, true));
        tallies.entry((c.runner.clone(), FIX, c.generated)).or_default().add(got, verdict);
        if let Some(class) = verdict {
            found.insert((c.runner.clone(), c.id.clone(), FIX, class), got);
        }
    }
    for l in LAYOUT {
        for stage in [RIGHT, WRONG, FIX] {
            let f = tallies.get(&(l.runner.to_string(), stage, false));
            let g = tallies.get(&(l.runner.to_string(), stage, true));
            let n = |t: Option<&Tally>| t.map_or((0, 0, 0), |t| (t.truth, t.unknown, t.wrong));
            let (ft, fu, fw) = n(f);
            let (gt, gu, gw) = n(g);
            eprintln!("h0 {:<13} stage {stage:<7}: fixtures {ft} truth / {fu} unknown / {fw} wrong; generated {gt} truth / {gu} unknown / {gw} wrong", l.runner);
        }
    }
    for (r, c, ss, why) in KNOWN {
        assert!(!why.is_empty(), "{r}/{c}: a KNOWN row says why it is accepted");
        assert!(ss.iter().all(|(_, k)| *k != FAKE_SEAL && *k != FALSE_GREEN), "{r}/{c}: no fake seal or false green is accepted (lane H6)");
    }
    let known: BTreeSet<(String, String, &str, &str)> = KNOWN.iter().flat_map(|(r, c, ss, _)| ss.iter().map(move |(s, k)| (r.to_string(), c.to_string(), *s, *k))).collect();
    let keys: BTreeSet<(String, String, &str, &str)> = found.keys().cloned().collect();
    let row = |k: &(String, String, &str, &str)| format!("{} / {} / stage {}: {} (read {:?})", k.0, k.1, k.2, k.3, found.get(k));
    let new: Vec<String> = keys.difference(&known).map(row).collect();
    let gone: Vec<String> = known.difference(&keys).map(row).collect();
    if !new.is_empty() || !gone.is_empty() {
        // today's set, as KNOWN reads it
        let mut rows: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
        for (r, c, s, k) in &keys {
            rows.entry((r.clone(), c.clone())).or_default().push(format!("(\"{s}\", \"{k}\")"));
        }
        let block: Vec<String> = rows.iter().map(|((r, c), v)| format!("    (\"{r}\", \"{c}\", &[{}], \"why\"),", v.join(", "))).collect();
        eprintln!("KNOWN as today reads:\n{}", block.join("\n"));
    }
    assert!(new.is_empty() && gone.is_empty(), "{} violations not in KNOWN:\n{}\n{} KNOWN rows no longer violated (take them out):\n{}", new.len(), new.join("\n"), gone.len(), gone.join("\n"));
}

/// The kind a truth's location has (lane H3): where a reading keeps a location for one of these
/// truths, it is of this kind.
fn kind_of(truth: Truth) -> Option<&'static str> {
    match truth {
        Assertion => Some("assertion"),
        Raised => Some("exception"),
        Missing => Some("compile"),
        _ => None,
    }
}

/// Where a recorded run's location kind is not its truth's (lane h4rustgo; a definition added to
/// H3's, announced): runner, case, the kind its output says, why.
const KINDS: &[(&str, &str, &str, &str)] = &[
    ("cargo-test", "tests_failed_exception_assert_in_code", "assertion", "std's `assertion failed: s.len() > 1` is an assertion wherever it fires; the run is an exception of the code under test by the place rule"),
    ("cargo-nextest", "tests_failed_exception_assert_in_code", "assertion", "std's `assertion failed: s.len() > 1` is an assertion wherever it fires; the run is an exception of the code under test by the place rule"),
    ("cargo-test", "tests_failed_exception_unwrap_in_test", "exception", "the unwrap's panic has no assertion text: an exception, at the test's own line (D1)"),
    ("cargo-nextest", "tests_failed_exception_unwrap_in_test", "exception", "the unwrap's panic has no assertion text: an exception, at the test's own line (D1)"),
    ("cargo-test", "tests_failed_exception_assert_message", "exception", "assert!(c, \"msg\") panics with the message alone: an exception, at the test's own line (D1)"),
    ("cargo-nextest", "tests_failed_exception_assert_message", "exception", "assert!(c, \"msg\") panics with the message alone: an exception, at the test's own line (D1)"),
];

/// What each recorded run's own summary says it counted (lane H3, D7; a definition added to H0's
/// tables, announced): runner, case, the `counts` its translation stores. A field the output
/// does not print is left out (pytest prints no `0 passed`; go prints a pass only under -v).
const COUNTED: &[(&str, &str, &str)] = &[
    ("cargo-nextest", "passed", r#"{"passed":1,"skipped":0}"#),
    ("cargo-nextest", "tests_failed", r#"{"passed":0,"failed":1,"skipped":0}"#),
    ("cargo-nextest", "big_run", r#"{"passed":334,"failed":1,"skipped":0}"#),
    ("cargo-nextest", "passed_status_level_none", r#"{"passed":1,"skipped":0}"#),
    ("cargo-nextest", "tests_failed_status_level_none", r#"{"passed":0,"failed":1,"skipped":0}"#),
    ("cargo-nextest", "passed_stress_count", r#"{"passed":2,"skipped":0}"#),
    ("cargo-nextest", "tests_failed_stress_count", r#"{"passed":0,"failed":1,"skipped":0}"#),
    ("cargo-nextest", "tests_failed_assert", r#"{"passed":0,"failed":1,"skipped":0}"#),
    ("cargo-test", "passed", r#"{"passed":1,"failed":0,"skipped":0,"deselected":0}"#),
    ("cargo-test", "tests_failed", r#"{"passed":0,"failed":1,"skipped":0,"deselected":0}"#),
    ("cargo-test", "big_run", r#"{"passed":599,"failed":1,"skipped":0,"deselected":0}"#),
    ("cargo-test", "passed_format_json", r#"{"passed":1,"failed":0,"skipped":0,"deselected":0}"#),
    ("cargo-test", "tests_failed_format_json", r#"{"passed":0,"failed":1,"skipped":0,"deselected":0}"#),
    ("cargo-test", "passed_format_junit", r#"{"passed":1,"failed":0,"skipped":0}"#),
    ("cargo-test", "tests_failed_format_junit", r#"{"passed":0,"failed":1,"skipped":0}"#),
    ("cargo-test", "tests_failed_assert", r#"{"passed":0,"failed":1,"skipped":0,"deselected":0}"#),
    ("catch2", "passed", r#"{"passed":1}"#),
    ("catch2", "tests_failed", r#"{"passed":0,"failed":1}"#),
    ("catch2", "big_run", r#"{"passed":599,"failed":1}"#),
    ("catch2", "passed_skipped", r#"{"passed":1,"skipped":1}"#),
    ("catch2", "passed_v2", r#"{"passed":1}"#),
    ("catch2", "tests_failed_v2", r#"{"passed":0,"failed":1}"#),
    ("catch2", "tests_failed_v2_two", r#"{"passed":0,"failed":2}"#),
    ("catch2", "passed_compact", r#"{"passed":1}"#),
    ("catch2", "tests_failed_compact", r#"{"passed":0,"failed":1}"#),
    ("catch2", "passed_tap", r#"{}"#),
    ("catch2", "tests_failed_tap", r#"{}"#),
    ("catch2", "passed_xml", r#"{"passed":1,"failed":0,"skipped":0}"#),
    ("catch2", "tests_failed_xml", r#"{"passed":0,"failed":1,"skipped":0}"#),
    ("catch2", "passed_junit", r#"{"passed":1,"failed":0,"skipped":0}"#),
    ("catch2", "tests_failed_junit", r#"{"passed":0,"failed":1,"skipped":0}"#),
    ("catch2", "passed_teamcity", r#"{"passed":1}"#),
    ("catch2", "tests_failed_teamcity", r#"{"passed":0,"failed":1}"#),
    ("catch2", "passed_automake", r#"{"passed":1}"#),
    ("catch2", "tests_failed_automake", r#"{"failed":1}"#),
    ("ctest", "passed", r#"{"passed":1,"failed":0}"#),
    ("ctest", "tests_failed", r#"{"passed":0,"failed":1}"#),
    ("ctest", "big_run", r#"{"passed":599,"failed":1}"#),
    ("ctest", "passed_quiet", r#"{}"#),
    ("ctest", "tests_failed_quiet", r#"{}"#),
    ("ctest", "no_tests_collected_quiet", r#"{}"#),
    ("dotnet-mstest", "passed", r#"{"passed":1,"failed":0,"skipped":0}"#),
    ("dotnet-mstest", "tests_failed", r#"{"passed":0,"failed":1,"skipped":0}"#),
    ("dotnet-mstest", "big_run", r#"{"passed":599,"failed":1,"skipped":0}"#),
    ("dotnet-mstest", "big_run_verbose", r#"{"passed":599,"failed":1}"#),
    ("dotnet-nunit", "passed", r#"{"passed":1,"failed":0,"skipped":0}"#),
    ("dotnet-nunit", "tests_failed", r#"{"passed":0,"failed":1,"skipped":0}"#),
    ("dotnet-nunit", "big_run", r#"{"passed":599,"failed":1,"skipped":0}"#),
    ("dotnet-nunit", "big_run_verbose", r#"{"passed":599,"failed":1}"#),
    ("dotnet-xunit", "passed", r#"{"passed":1,"failed":0,"skipped":0}"#),
    ("dotnet-xunit", "tests_failed", r#"{"passed":0,"failed":1,"skipped":0}"#),
    ("dotnet-xunit", "big_run", r#"{"passed":599,"failed":1,"skipped":0}"#),
    ("dotnet-xunit", "big_run_verbose", r#"{"passed":599,"failed":1}"#),
    ("go-test", "passed", r#"{}"#),
    ("go-test", "tests_failed", r#"{"failed":1}"#),
    ("go-test", "big_run", r#"{"failed":1}"#),
    ("go-test", "big_run_verbose", r#"{"passed":599,"failed":1}"#),
    ("go-test", "passed_json", r#"{"passed":1}"#),
    ("go-test", "tests_failed_json", r#"{"failed":1}"#),
    ("go-test", "tests_failed_exception_json", r#"{"failed":1}"#),
    ("go-test", "tests_failed_verbose", r#"{"failed":1}"#),
    ("go-test", "passed_pipe_go_junit_report", r#"{}"#),
    ("googletest", "passed", r#"{"passed":1}"#),
    ("googletest", "tests_failed", r#"{"passed":0,"failed":1}"#),
    ("googletest", "big_run", r#"{"passed":599,"failed":1}"#),
    ("googletest", "via_ctest_tests_failed", r#"{"passed":0,"failed":2}"#),
    ("googletest", "passed_brief", r#"{"passed":1}"#),
    ("googletest", "passed_skipped", r#"{"passed":1,"skipped":1}"#),
    ("googletest", "tests_failed_brief", r#"{"passed":0}"#),
    ("googletest", "tests_failed_brief_no_time", r#"{"passed":0}"#),
    ("googletest", "passed_brief_skipped", r#"{"passed":1,"skipped":1}"#),
    ("gradle", "passed", r#"{}"#),
    ("gradle", "tests_failed", r#"{"passed":0,"failed":1,"skipped":0}"#),
    ("gradle", "big_run", r#"{"passed":599,"failed":1,"skipped":0}"#),
    ("jest", "passed", r#"{"passed":1}"#),
    ("jest", "tests_failed", r#"{"passed":0,"failed":1}"#),
    ("jest", "big_run", r#"{"passed":599,"failed":1}"#),
    ("maven", "passed", r#"{"passed":1,"failed":0,"skipped":0}"#),
    ("maven", "tests_failed", r#"{"passed":0,"failed":1,"skipped":0}"#),
    ("maven", "big_run", r#"{"passed":599,"failed":1,"skipped":0}"#),
    // lane h4jvmnet: `mvn -q` prints no aggregate line; the surefire report says what ran
    ("maven", "passed_quiet", r#"{"passed":1,"failed":0,"skipped":0}"#),
    ("maven", "tests_failed_quiet", r#"{"passed":0,"failed":1,"skipped":0}"#),
    ("maven", "tests_failed_exception_quiet", r#"{"passed":0,"failed":1,"skipped":0}"#),
    ("maven", "no_tests_collected_no_test_methods", r#"{"passed":0,"failed":0,"skipped":0}"#),
    ("mocha", "passed", r#"{"passed":1}"#),
    ("mocha", "tests_failed", r#"{"passed":0,"failed":1}"#),
    ("mocha", "big_run", r#"{"passed":599,"failed":1}"#),
    ("node-test", "passed", r#"{"passed":1,"failed":0,"skipped":0}"#),
    ("node-test", "tests_failed", r#"{"passed":0,"failed":1,"skipped":0}"#),
    ("node-test", "big_run", r#"{"passed":599,"failed":1,"skipped":0}"#),
    ("pest", "passed", r#"{"passed":1}"#),
    ("pest", "tests_failed", r#"{"failed":1}"#),
    ("pest", "big_run", r#"{"passed":599,"failed":1}"#),
    ("pest", "passed_compact", r#"{"passed":1}"#),
    ("pest", "passed_ci", r#"{"passed":1}"#),
    ("pest", "passed_skipped", r#"{"passed":1,"skipped":1}"#),
    ("pest", "tests_failed_compact", r#"{"failed":1}"#),
    ("pest", "tests_failed_ci", r#"{"failed":1}"#),
    ("pest", "tests_failed_exception_compact", r#"{"failed":1}"#),
    ("pest", "missing_target_symbol_compact", r#"{"failed":1}"#),
    ("phpunit", "passed", r#"{"passed":1}"#),
    ("phpunit", "tests_failed", r#"{"passed":0,"failed":1}"#),
    ("phpunit", "big_run", r#"{"passed":599,"failed":1}"#),
    ("phpunit", "passed_compact", r#"{"passed":1}"#),
    ("phpunit", "passed_skipped", r#"{"passed":1,"skipped":1}"#),
    ("phpunit", "passed_compact_skipped", r#"{"passed":1,"skipped":1}"#),
    ("phpunit", "tests_failed_compact", r#"{"passed":0,"failed":1}"#),
    ("phpunit", "tests_failed_exception_compact", r#"{"passed":0,"failed":1}"#),
    ("phpunit", "missing_target_symbol_compact", r#"{"passed":0,"failed":1}"#),
    ("phpunit", "setup_error_compact", r#"{"passed":0,"failed":1}"#),
    ("phpunit", "environment_error_missing_class_compact", r#"{"passed":0,"failed":1}"#),
    ("pytest", "passed", r#"{"passed":1}"#),
    ("pytest", "tests_failed", r#"{"failed":1}"#),
    ("pytest", "big_run", r#"{"passed":599,"failed":1}"#),
    ("pytest", "passed_pretty", r#"{"passed":1}"#),
    ("pytest", "passed_skipped_pretty", r#"{"passed":1,"skipped":1}"#),
    ("pytest", "tests_failed_pretty", r#"{"failed":1}"#),
    ("pytest", "passed_pretty_deselected", r#"{"passed":1,"deselected":2}"#),
    ("pytest", "no_tests_collected_pretty_skipped", r#"{"skipped":1}"#),
    ("pytest", "no_tests_collected_pretty_filter", r#"{"deselected":1}"#),
    ("pytest", "tests_failed_two", r#"{"failed":2}"#),
    ("pytest", "tests_failed_two_pretty", r#"{"failed":2}"#),
    ("pytest", "tests_failed_exception_two", r#"{"failed":2}"#),
    ("pytest", "missing_target_symbol_mixed_typo", r#"{"failed":2}"#),
    ("pytest", "passed_skipped", r#"{"passed":1,"skipped":1}"#),
    ("pytest", "passed_xdist", r#"{"passed":1}"#),
    ("pytest", "tests_failed_xdist", r#"{"failed":1}"#),
    ("pytest", "tests_failed_tb_no", r#"{"failed":1}"#),
    ("pytest", "tests_failed_tb_short", r#"{"failed":1}"#),
    ("jest", "passed_skipped", r#"{"passed":1,"skipped":1}"#),
    ("jest", "tests_failed_reporters_summary", r#"{"passed":0,"failed":1}"#),
    ("jest", "passed_reporters_summary", r#"{"passed":1}"#),
    ("vitest", "passed_skipped", r#"{"passed":1,"skipped":1}"#),
    ("vitest", "no_tests_collected_related", r#"{"passed":0,"skipped":1}"#),
    ("mocha", "passed_skipped", r#"{"passed":1,"skipped":1}"#),
    ("mocha", "tests_failed_reporter_tap", r#"{"passed":0,"failed":1}"#),
    ("mocha", "passed_reporter_tap", r#"{"passed":1,"failed":0}"#),
    ("mocha", "tests_failed_reporter_json", r#"{"passed":0,"failed":1,"skipped":0}"#),
    ("mocha", "passed_reporter_json", r#"{"passed":1,"failed":0,"skipped":0}"#),
    ("mocha", "tests_failed_reporter_json_stream", r#"{"passed":0,"failed":1,"skipped":0}"#),
    ("mocha", "passed_reporter_json_stream", r#"{"passed":1,"failed":0,"skipped":0}"#),
    ("mocha", "tests_failed_reporter_xunit", r#"{"passed":0,"failed":1,"skipped":0}"#),
    ("mocha", "passed_reporter_xunit", r#"{"passed":1,"failed":0,"skipped":0}"#),
    ("node-test", "passed_skipped", r#"{"passed":1,"failed":0,"skipped":1}"#),
    ("node-test", "tests_failed_spec", r#"{"passed":0,"failed":1,"skipped":0}"#),
    ("node-test", "passed_spec", r#"{"passed":1,"failed":0,"skipped":0}"#),
    ("node-test", "passed_skipped_spec", r#"{"passed":1,"failed":0,"skipped":1}"#),
    ("node-test", "tests_failed_exception_spec", r#"{"passed":0,"failed":1,"skipped":0}"#),
    ("sh", "passed", r#"{}"#),
    ("sh", "tests_failed", r#"{}"#),
    ("unittest", "passed", r#"{"passed":1}"#),
    ("unittest", "tests_failed", r#"{"passed":0,"failed":1}"#),
    ("unittest", "big_run", r#"{"passed":599,"failed":1}"#),
    ("unittest", "passed_skipped", r#"{"passed":1,"skipped":1}"#),
    ("unittest", "tests_failed_two", r#"{"passed":0,"failed":2}"#),
    ("unittest", "tests_failed_exception_two", r#"{"passed":0,"failed":2}"#),
    ("unittest", "missing_target_symbol_mixed_typo", r#"{"passed":0,"failed":2}"#),
    ("unittest", "missing_target_symbol_attr", r#"{"passed":0,"failed":1}"#),
    ("vitest", "passed", r#"{"passed":1}"#),
    ("vitest", "tests_failed", r#"{"passed":0,"failed":1}"#),
    ("vitest", "big_run", r#"{"passed":599,"failed":1}"#),
];

/// Lane H3: every reading stores `counts` (D7's marker) and, for every recorded run, a kept
/// location has its truth's kind and `COUNTED` is what the summary said.
#[test]
fn every_kept_kind_is_the_truths_and_every_count_the_runners_own() {
    let all = fixtures();
    let gen = generated(&all);
    let mut wrong = Vec::new();
    for c in all.iter().chain(gen.iter()) {
        let t = translation(c, "r1", "b2").expect("a reading");
        if !t.get("counts").is_some_and(Value::is_object) {
            wrong.push(format!("{}/{}: no counts", c.runner, c.id));
        }
        if c.generated {
            continue;
        }
        let kind = t.get("location").and_then(|l| l.get("kind")).and_then(Value::as_str);
        let said = KINDS.iter().find(|(r, f, _, _)| *r == c.runner && *f == c.id).map(|(_, _, k, _)| *k);
        if let (Some(want), Some(got)) = (said.or(kind_of(c.truth)), kind) {
            if want != got {
                wrong.push(format!("{}/{}: a {:?} kept as {got}", c.runner, c.id, c.truth));
            }
        }
        if let Some((_, _, want)) = COUNTED.iter().find(|(r, f, _)| *r == c.runner && *f == c.id) {
            let want: Value = serde_json::from_str(want).expect("COUNTED is JSON");
            if t["counts"] != want {
                wrong.push(format!("{}/{}: counted {} where the summary says {want}", c.runner, c.id, t["counts"]));
            }
        }
    }
    for (r, f, _) in COUNTED {
        assert!(all.iter().any(|c| c.runner == *r && c.id == *f), "{r}/{f} is recorded");
    }
    for l in LAYOUT {
        assert!(COUNTED.iter().any(|(r, f, _)| *r == l.runner && *f == "tests_failed"), "{}: its tests_failed run's counts", l.runner);
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Lane H4 (plan H3, H6): the runners whose recorded assertion reds are held to keep an assertion
/// location in a planned test file. H6 reads a `tests_failed` with no location as can't tell, so a
/// runner with no assertion row would ask a card for every honest red.
const LOCATED: &[&str] = &[
    // lane h4cppphp: the C++ and PHP runners
    "catch2",
    "ctest",
    "googletest",
    "pest",
    "phpunit",
];

/// A recorded assertion red read as `tests_failed` whose output names no place, and why: runner,
/// case, why. Each is a card under H6; a row that finds the place takes its entry out. (The place
/// is the first candidate of the translation: whether git tracks its file is `location_per_runner.rs`'s.)
const UNLOCATED: &[(&str, &str, &str)] = &[
    ("catch2", "tests_failed_automake", "automake prints `:test-result: FAIL <name>` and nothing else"),
    ("catch2", "tests_failed_tap", "TAP prints `not ok N - <expression> for: <expansion>` with no file or line"),
    ("ctest", "big_run", "as tests_failed: ctest's default prints `***Failed` rows and no output of the test"),
    ("ctest", "tests_failed", "ctest shows a failed test's own output only under --output-on-failure or -V; this run prints `***Failed` and nothing of why (its LastTest.log is no report in production)"),
    ("googletest", "via_ctest_tests_failed", "two failing tests, one an assertion and one a caught exception (`unknown file: Failure`): rule (3) of lane H3, an exception with no throw site, leaves the run no place at all"),
];

/// Lane H4, rule (3) of lane H3: an exception the framework caught and printed without its throw
/// site (googletest's `unknown file`, Catch2's `due to unexpected exception`, ctest's terminate
/// text) keeps no location at all, whatever other rows match: the tables let rule (3) apply.
const NO_PLACE: &[(&str, &str)] = &[
    ("catch2", "tests_failed_exception"),
    ("ctest", "tests_failed_exception"),
    ("ctest", "tests_failed_exception_output_on_failure"),
    ("googletest", "tests_failed_exception"),
];

/// The first place `c`'s output points (what `kept::classify` finds, before git is asked which
/// of them is inside the project): its file and the kind of the row that found it.
fn first_place(c: &Case) -> Option<(String, Option<String>)> {
    let streams = Streams { stdout: &c.stdout, stderr: &c.stderr, report: &c.report };
    let t = classify(c.end, &streams, details_for(&c.cmd))?;
    t.candidates.first().map(|l| (l.file_path.clone(), l.kind.map(|k| format!("{k:?}").to_lowercase())))
}

#[test]
fn every_recorded_assertion_keeps_its_place_or_says_why_not() {
    let all = fixtures();
    let mut wrong = Vec::new();
    let mut said: BTreeSet<(&str, &str)> = BTreeSet::new();
    for c in all.iter().filter(|c| LOCATED.contains(&c.runner.as_str()) && c.truth == Assertion) {
        let l = layout(&c.runner);
        let t = translation(c, "r1", "b2").expect("a reading");
        if t["label"] != "tests_failed" {
            continue;
        }
        let at = first_place(c);
        match UNLOCATED.iter().find(|(r, f, _)| *r == c.runner && *f == c.id) {
            Some((r, f, _)) => {
                said.insert((r, f));
                if let Some(at) = at {
                    wrong.push(format!("{}/{}: UNLOCATED, yet it points at {at:?}", c.runner, c.id));
                }
            }
            None => match at {
                None => wrong.push(format!("{}/{}: an assertion red with no location (H6 would ask a card)", c.runner, c.id)),
                Some((file, kind)) => {
                    if kind.as_deref() != Some("assertion") || file == l.file {
                        wrong.push(format!("{}/{}: points at {file} as {kind:?}, not an assertion in a test", c.runner, c.id));
                    }
                }
            },
        }
    }
    for (r, f, why) in UNLOCATED {
        assert!(!why.is_empty(), "{r}/{f}: an UNLOCATED entry says why");
        assert!(said.contains(&(*r, *f)), "{r}/{f} is read as tests_failed and kept no place (take the entry out)");
    }
    for (r, f) in NO_PLACE {
        let c = all.iter().find(|c| c.runner == *r && c.id == *f).unwrap_or_else(|| panic!("{r}/{f} is recorded"));
        let t = translation(c, "r1", "b2").expect("a reading");
        if t["label"] != "tests_failed" || t.get("location").is_some_and(|v| !v.is_null()) || first_place(c).is_some() {
            wrong.push(format!("{r}/{f}: rule (3) wants tests_failed with no place at all, got {} at {:?}", t["label"], first_place(c)));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
