//! [draft] googletest (C/C++), a report reader only: `GTEST_OUTPUT=xml:<folder>/` makes each test
//! executable write `<folder>/<executable>.xml` (a trailing `/` names a folder, so several
//! executables under one `ctest` do not overwrite each other), and `GTEST_BRIEF` is unset in
//! Familiar's own run so the console the tables read is the full one.
//!
//! The XML is JUnit-shaped (`<testsuites>`, `<testsuite>`, `<testcase name classname status result
//! file line>`, `<failure message="a.cc:12&#x0A;Expected equality..." type="">`, `<skipped/>`); a
//! disabled test is `status="notrun" result="suppressed"` and reads as skipped. A failure names
//! no type: it reads as an assertion only because its message opens with the assertion's own
//! `file:line` line, and a C++ exception thrown in the test body (`unknown file`) as an exception.
//! The compile error of a missing function writes no report: the report reader never gives
//! `missing_target_symbol` or `collection_error`; those stay with the tables.

use super::spec::{EnvOp, Kind, KitFile, Op, Probe, Reader};

pub const ID: &str = "gtest";
pub const LANGUAGE: &str = "cpp";

/// C/C++ is last in the plan's coverage order (A-7): the readers below are written from
/// googletest's documented output and read by the JUnit converter from hand-made samples, never
/// from a recorded run, and no probe kit calibrates them.
pub const CALIBRATED: bool = false;
pub const NOT_CALIBRATED: &str = "googletest is not calibrated: no probe kit, no recorded run; the dry run cannot prove this reader here, so it never becomes a choice until a probe kit and recordings exist";

pub const KIT_FILES: &[KitFile] = &[];

pub const READERS: &[Reader] = &[
    Reader {
        id: "gtest:report",
        runner: ID,
        kind: Kind::Report,
        env: &[EnvOp { var: "GTEST_OUTPUT", op: Op::Set, value: "xml:{REPORT_DIR}/", separator: "" }, EnvOp { var: "GTEST_BRIEF", op: Op::Unset, value: "", separator: "" }],
        report_globs: &["{REPORT_DIR}/*.xml"],
        converter: "junit",
        cannot_read: &["P3", "P4"],
        ..Reader::NONE
    },
    Reader { id: "gtest:tables", runner: ID, kind: Kind::Tables, ..Reader::NONE },
];

/// Selection flags `re_aim` drops (`--gtest_filter=Suite.Case:-Other`).
pub const DROP_SELECTION: &[&str] = &["--gtest_filter"];

/// Setup files whose change in a commit makes the choice stale.
pub const WATCHED: &[&str] = &["CMakeLists.txt", "**/CMakeLists.txt"];

/// No probe kit: not calibrated.
pub const PROBES: &[Probe] = &[];
