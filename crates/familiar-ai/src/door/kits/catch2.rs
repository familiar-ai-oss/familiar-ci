//! [draft] Catch2 v3 (C/C++), a report reader only: with `BAZEL_TEST=1` Catch2 writes a JUnit
//! report to the file named by `XML_OUTPUT_FILE` (Bazel's convention) beside its console output.
//! One file per process: a project that runs several Catch2 executables under one command would
//! overwrite it, which the dry run notices (the counts of the report and the console differ) and
//! answers by dropping the reader.
//!
//! Catch2's JUnit reporter names the macro that failed in `type` (`REQUIRE`, `CHECK_THAT`, ...) and
//! ends the failure text with `at <file>:<line>`: a failed macro is an assertion with that place.
//! A compile error of a missing function writes no report: the report reader never gives
//! `missing_target_symbol` or `collection_error`; those stay with the tables.

use super::spec::{EnvOp, Kind, KitFile, Op, Probe, Reader};

pub const ID: &str = "catch2";
pub const LANGUAGE: &str = "cpp";

/// C/C++ is last in the plan's coverage order (A-7): this reader is written from Catch2's
/// documented output and read by the JUnit converter from hand-made samples, never from a
/// recorded run, and no probe kit calibrates it.
pub const CALIBRATED: bool = false;
pub const NOT_CALIBRATED: &str = "Catch2 is not calibrated: no probe kit, no recorded run; the dry run cannot prove this reader here, so it never becomes a choice until a probe kit and recordings exist";

pub const KIT_FILES: &[KitFile] = &[];

pub const READERS: &[Reader] = &[
    Reader {
        id: "catch2:report",
        runner: ID,
        kind: Kind::Report,
        env: &[EnvOp { var: "BAZEL_TEST", op: Op::Set, value: "1", separator: "" }, EnvOp { var: "XML_OUTPUT_FILE", op: Op::Set, value: "{REPORT_DIR}/catch2.xml", separator: "" }],
        report_globs: &["{REPORT_DIR}/catch2.xml"],
        converter: "junit",
        cannot_read: &["P3", "P4"],
        ..Reader::NONE
    },
    Reader { id: "catch2:tables", runner: ID, kind: Kind::Tables, ..Reader::NONE },
];

/// Catch2 selects by positional test specs, which `re_aim` drops with the other targets.
pub const DROP_SELECTION: &[&str] = &[];

/// Setup files whose change in a commit makes the choice stale.
pub const WATCHED: &[&str] = &["CMakeLists.txt", "**/CMakeLists.txt"];

/// No probe kit: not calibrated.
pub const PROBES: &[Probe] = &[];
