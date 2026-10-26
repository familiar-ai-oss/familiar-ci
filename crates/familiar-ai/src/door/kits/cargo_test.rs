//! cargo test as plain data for the reader table (plan A, lane akgorust; [draft]): tables only.
//! There is no reporter, no converter and no kit file; the probe kit here is also nextest's
//! (`kits::cargo_nextest::PROBES`).

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
pub const RUNNER: &str = "cargo-test";

/// The kit files: none.
pub const KIT_FILES: &[(&str, &str)] = &[];

/// No converter, no stream to read but the console.
pub const CONVERTER: &str = "";
pub const NATIVE_STREAM: &str = "";

/// Why there is no reporter (the A0 spike, cargo 1.94.0).
pub const NOTE: &str = "cargo test: tables only. libtest's --logfile is rewritten by every test binary (the last one, the doctests, leaves it empty), and --format json is nightly-only (RUSTC_BOOTSTRAP=1 makes it work on stable, but it needs a flag after -- and an env for the whole build): a reporter is a later spike. CARGO_TERM_COLOR=never as a pinned console is not recorded either.";

/// The only reader.
pub const READERS: &[Reader] = &[("cargo-test:tables", "tables", &[], &[], &[])];

/// cargo's target selection, which re-aim drops from the person's command (with whether the flag
/// takes a value); the filter names are positional and dropped by re-aim itself, and the probe's
/// `--test familiar_probe_<k>` is appended.
pub const SELECTION_FLAGS: &[(&str, bool)] = &[
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
    ("--doc", false),
];

/// libtest's own selection, after `--`.
pub const AFTER_DOUBLE_DASH: &[(&str, bool)] = &[("--skip", true), ("--exact", false), ("--ignored", false), ("--include-ignored", false)];

/// The setup files whose change calls for a new dry run, relative to the project root.
pub const WATCHED: &[&str] = &["**/Cargo.toml", "**/Cargo.lock"];

const P1: &str = r#"fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[test]
fn adds() {
    assert_eq!(add(2, 2), 4);
}
"#;

const P2: &str = r#"fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[test]
fn adds() {
    assert_eq!(add(2, 2), 5);
}
"#;

const P3: &str = r#"#[test]
fn muls() {
    assert_eq!(mul(2, 3), 6);
}
"#;

const P3B: &str = r#"fn mul(a: i32, b: i32) -> i32 {
    a * b
}

#[test]
fn muls() {
    assert_eq!(mul(2, 3), 6);
}
"#;

const P4: &str = r#"use nosuch::FOUR;

#[test]
fn adds() {
    assert_eq!(2 + 2, FOUR);
}
"#;

const P5: &str = r#"fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[test]
fn adds() {
    assert_eq!(add(2, 2), 4);
}

#[test]
#[ignore = "not yet"]
fn later() {
    assert_eq!(add(1, 1), 2);
}
"#;

const P6: &str = r#"#[allow(dead_code)]
fn helper() -> i32 {
    2
}
"#;

const P7: &str = r#"fn parse(s: &str) -> i32 {
    match s.parse() {
        Ok(n) => n,
        Err(_) => panic!("not a number: {}", s),
    }
}

#[test]
fn parses() {
    assert_eq!(parse("x1"), 1);
}
"#;

/// The probe kit, a test target per probe with the code under test in the file itself, so nothing
/// of the project's crate is named (the A0 spike's recordings are the same tests against a crate of
/// their own): stage 1 P1 pass, P2 assertion, P3 missing function, P4 broken import (a crate that is
/// not there is a missing module, not a missing name), P5 a pass beside an ignored test, P6 nothing
/// to run; stage 2 P3b (the function exists) pass and P7 a panic inside the code's own function.
/// Each probe copy builds in a target dir of its own: copies never share one.
pub const PROBES: &[Probe] = &[
    ("P1", "passed", "", &[("tests/familiar_probe_p1.rs", P1)]),
    ("P2", "tests_failed", "assertion", &[("tests/familiar_probe_p2.rs", P2)]),
    ("P3", "missing_target_symbol", "", &[("tests/familiar_probe_p3.rs", P3)]),
    ("P3b", "passed", "", &[("tests/familiar_probe_p3b.rs", P3B)]),
    ("P4", "collection_error", "", &[("tests/familiar_probe_p4.rs", P4)]),
    ("P5", "passed", "", &[("tests/familiar_probe_p5.rs", P5)]),
    ("P6", "no_tests_collected", "", &[("tests/familiar_probe_p6.rs", P6)]),
    ("P7", "tests_failed", "exception", &[("tests/familiar_probe_p7.rs", P7)]),
];

/// The tests of probe P5 that must read skipped, by name (an id is `crate::binary$test` for
/// nextest).
pub const SKIPPED: &[&str] = &["later"];
