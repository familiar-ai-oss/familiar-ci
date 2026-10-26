//! cargo-nextest as plain data for the reader table (plan A, lane akgorust; [draft]): the readers,
//! the converter and the stream it reads, the selection flags re-aim drops, the setup files that are
//! watched. The probe kit is cargo test's (`kits::cargo_test::PROBES`). There is no kit file:
//! nextest's own `libtest-json-plus` is the report, and `door::convert::libtest_json` reads it from
//! the run's stdout.

/// An env op: (VAR, op `set|append|prepend|unset`, value, the separator put between the person's
/// own value and ours).
pub type EnvOp = (&'static str, &'static str, &'static str, &'static str);

/// A reader: (id, kind `reporter|report|pinned|tables`, env ops, flags added to Familiar's own run,
/// report globs).
pub type Reader = (&'static str, &'static str, &'static [EnvOp], &'static [&'static str], &'static [&'static str]);

/// A probe: (id, truth label, truth kind of a `tests_failed` or empty, files: path relative to the package root
/// and text; the first is the test the command names). The probes of stage 2 are P3b and P7.
pub type Probe = (&'static str, &'static str, &'static str, &'static [(&'static str, &'static str)]);

/// The runner's name, as the tables and the adapter file name it.
pub const RUNNER: &str = "cargo-nextest";

/// The kit files: none. nextest's own `libtest-json-plus` is the report and
/// `door::convert::libtest_json` (Rust) reads it.
pub const KIT_FILES: &[(&str, &str)] = &[];

/// The converter in `door::convert` and the stream of the run it reads.
pub const CONVERTER: &str = "libtest_json";
pub const NATIVE_STREAM: &str = "stdout";

/// What the reader needs and what it cannot read (the A0 spike, cargo-nextest 0.9.143).
pub const NOTE: &str = "Both variables are needed: NEXTEST_EXPERIMENTAL_LIBTEST_JSON alone prints no JSON, and NEXTEST_MESSAGE_FORMAT alone fails the run. The person's own --message-format beats them, so the reader is skipped then (SKIP_WHEN). A build failure and exit 4 (no tests to run) print no JSON, only cargo's diagnostics on stderr: no session line, the next reader. An ignored test is a test started with no end. nextest's human lines stay on stderr unchanged. JUnit (--tool-config-file familiar-ai:<file> with [profile.default.junit]) is not reachable by env, leaves ignored tests out, and ranks below this reader: it is not one here.";

/// The readers, best first. The report is nextest's own structured output on the run's stdout (the
/// plan's table lists it with the report files), so there is no glob.
pub const READERS: &[Reader] = &[
    (
        "cargo-nextest:report",
        "report",
        &[("NEXTEST_EXPERIMENTAL_LIBTEST_JSON", "set", "1", ""), ("NEXTEST_MESSAGE_FORMAT", "set", "libtest-json-plus", "")],
        &[],
        &[],
    ),
    ("cargo-nextest:tables", "tables", &[], &[], &[]),
];

/// Readers that do not apply when the person's command has one of these flags: (reader id, flags).
pub const SKIP_WHEN: &[(&str, &[&str])] = &[("cargo-nextest:report", &["--message-format"])];

/// The selection flags re-aim drops from the person's command, with whether the flag takes a value;
/// the filter names are positional and dropped by re-aim itself, and the probe's
/// `--test familiar_probe_<k>` is appended.
pub const SELECTION_FLAGS: &[(&str, bool)] = &[
    ("-E", true),
    ("--filter-expr", true),
    ("--run-ignored", true),
    ("--partition", true),
    ("--test", true),
    ("--lib", false),
    ("--bin", true),
    ("--bins", false),
    ("--example", true),
    ("--examples", false),
    ("--tests", false),
    ("--bench", true),
    ("--benches", false),
    ("--all-targets", false),
];

/// The setup files whose change calls for a new dry run, relative to the project root.
pub const WATCHED: &[&str] = &["**/Cargo.toml", "**/Cargo.lock", ".config/nextest.toml"];

/// The probe kit, the same as cargo test's.
pub const PROBES: &[Probe] = super::cargo_test::PROBES;

/// The tests of probe P5 that must read skipped, the same as cargo test's.
pub const SKIPPED: &[&str] = super::cargo_test::SKIPPED;
