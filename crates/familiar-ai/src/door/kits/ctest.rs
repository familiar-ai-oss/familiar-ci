//! [draft] CTest (CMake, C/C++), a report reader only: the added flag `--output-junit
//! <folder>/ctest.xml` (CMake 3.21 and later) writes one JUnit file for the whole run, whatever
//! the test executables are. It is skipped when the command has its own `--output-junit` (a second
//! one replaces the first).
//!
//! CTest only knows that a test program exited non-zero, crashed or timed out: a `<failure
//! message="Failed"/>` names no type and no place, so it reads as an exception with no frames
//! (which asks at stage 1 and counts as a failed attempt at stage 2). A disabled test is
//! `status="notrun"` and reads as skipped. Where the tests are googletest or Catch2 programs, their
//! own readers (`gtest:report`, `catch2:report`) see the assertions CTest cannot.

use super::spec::{Kind, KitFile, Probe, Reader};

pub const ID: &str = "ctest";
pub const LANGUAGE: &str = "cpp";

/// C/C++ is last in the plan's coverage order (A-7): this reader is written from CTest's
/// documented output and read by the JUnit converter from hand-made samples, never from a
/// recorded run, and no probe kit calibrates it.
pub const CALIBRATED: bool = false;
pub const NOT_CALIBRATED: &str = "CTest is not calibrated: no probe kit, no recorded run; the dry run cannot prove this reader here, so it never becomes a choice until a probe kit and recordings exist";

pub const KIT_FILES: &[KitFile] = &[];

pub const READERS: &[Reader] = &[
    Reader {
        id: "ctest:report",
        runner: ID,
        kind: Kind::Report,
        flags: &["--output-junit", "{REPORT_DIR}/ctest.xml"],
        skip_when_command_has: &["--output-junit"],
        report_globs: &["{REPORT_DIR}/ctest.xml"],
        converter: "junit",
        cannot_read: &["P3", "P4"],
        ..Reader::NONE
    },
    Reader { id: "ctest:tables", runner: ID, kind: Kind::Tables, ..Reader::NONE },
];

/// Selection flags `re_aim` drops (`-R regex`, `-E regex`, `-L label`, and their long forms).
pub const DROP_SELECTION: &[&str] = &["-R", "-E", "-L"];

/// Setup files whose change in a commit makes the choice stale.
pub const WATCHED: &[&str] = &["CMakeLists.txt", "**/CMakeLists.txt"];

/// No probe kit: not calibrated.
pub const PROBES: &[Probe] = &[];
