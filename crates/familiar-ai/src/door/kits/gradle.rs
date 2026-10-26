//! [draft] Gradle, as the A0 spike proved it (Gradle 8.10.2, JDK 21, JUnit 5): the XML reports
//! need no change to the command (`build/test-results/<task>/TEST-*.xml`, `test` and any other
//! test task, hence the `**`). Same pattern as Maven: P1, P2, P3b, P5 and P7 read right; P3 and
//! P4 (compile errors) and P6 (no XML at exit 0 for a class with no tests) ask and stay with the
//! console tables.
//!
//! Gradle writes every exception as `<failure>`: the converter takes the kind from the exception
//! type (P7's `java.lang.NumberFormatException` is an exception, not an assertion). Not run:
//! Gradle 9 (`failOnNoDiscoveredTests`), Kotlin DSL projects (the same reports), failsafe.
//! Gradle's probe kit is Maven's (the same JUnit 5 sources, `maven::PROBES`).

use super::spec::{self, Kind, Probe, Reader};

pub const ID: &str = "gradle";
pub const LANGUAGE: &str = "java";

/// Gradle has no reporter to inject: the report files are what it has.
pub const KIT_FILES: &[spec::KitFile] = &[];

pub const READERS: &[Reader] = &[
    Reader {
        id: "gradle:report",
        runner: ID,
        kind: Kind::Report,
        report_globs: &["build/test-results/**/*.xml"],
        converter: "junit",
        cannot_read: &["P3", "P4", "P6"],
        ..Reader::NONE
    },
    Reader { id: "gradle:tables", runner: ID, kind: Kind::Tables, ..Reader::NONE },
];

/// Selection flags `re_aim` drops (`--tests Foo`, `--tests=Foo`).
pub const DROP_SELECTION: &[&str] = &["--tests"];

/// Setup files whose change in a commit makes the choice stale (subprojects' too).
pub const WATCHED: &[&str] = &["build.gradle", "build.gradle.kts", "**/build.gradle", "**/build.gradle.kts"];

/// The same JUnit 5 probes as Maven's: the build tool differs, the sources do not.
pub const PROBES: &[Probe] = super::maven::PROBES;
