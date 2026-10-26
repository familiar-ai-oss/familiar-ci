//! [draft] Maven (surefire and failsafe), as the A0 spike proved it (Maven 3.9.16, surefire 3.5.2,
//! JUnit 5.11.4, JDK 21): the reports need no change to the command (`target/surefire-reports/
//! TEST-*.xml`); the Maven 3.9 launcher honours `MAVEN_ARGS`, which appends `-B` (batch mode: no
//! colour, no progress) to whatever the person ran. `-q` and plain read the same.
//!
//! A report reader reads P1, P2, P3b, P5 and P7 right and asks on P3 and P4 (a compile error:
//! `cannot find symbol`, `package nosuch does not exist` write no XML, and a compile error leaves
//! the last run's XML behind, which the converter ignores by `run_started`) and on P6 (surefire
//! writes no XML for a class with no tests, exit 0). Those three stay with the console tables
//! (`[ERROR] ...ProbeTest.java:[10,29] cannot find symbol`, kind compile).
//!
//! The probe kit is JUnit 5 in package `calc`; Gradle's is the same text (`gradle::PROBES`).

use super::spec::{self, EnvOp, Kind, Op, Probe, ProbeFile, Reader};

pub const ID: &str = "maven";
pub const LANGUAGE: &str = "java";

/// Maven has no reporter to inject: the report files and the pinned console are all it has.
pub const KIT_FILES: &[spec::KitFile] = &[];

const BATCH: EnvOp = EnvOp { var: "MAVEN_ARGS", op: Op::Append, value: "-B", separator: " " };

pub const READERS: &[Reader] = &[
    Reader {
        id: "maven:report",
        runner: ID,
        kind: Kind::Report,
        env: &[BATCH],
        report_globs: &["target/surefire-reports/TEST-*.xml", "target/failsafe-reports/TEST-*.xml"],
        converter: "junit",
        cannot_read: &["P3", "P4", "P6"],
        ..Reader::NONE
    },
    Reader { id: "maven:pinned", runner: ID, kind: Kind::Pinned, env: &[BATCH], ..Reader::NONE },
    Reader { id: "maven:tables", runner: ID, kind: Kind::Tables, ..Reader::NONE },
];

/// Selection flags `re_aim` drops (`-Dtest=Foo#bar`, `-Dtest Foo`): the dry run's own probes are the only tests.
pub const DROP_SELECTION: &[&str] = &["-Dtest"];

/// Setup files whose change in a commit makes the choice stale (module poms at any depth too).
pub const WATCHED: &[&str] = &["pom.xml", "**/pom.xml"];

const CALC: &str = r#"package calc;

public class Calc {
    public static int add(int a, int b) {
        return a + b;
    }

    public static int parse(String s) {
        return Integer.parseInt(s);
    }
}
"#;

const CALC_WITH_MUL: &str = r#"package calc;

public class Calc {
    public static int add(int a, int b) {
        return a + b;
    }

    public static int parse(String s) {
        return Integer.parseInt(s);
    }

    public static int mul(int a, int b) {
        return a * b;
    }
}
"#;

const P1: &str = r#"package calc;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

class ProbeTest {
    @Test
    void adds() {
        assertEquals(4, Calc.add(2, 2));
    }
}
"#;

const P2: &str = r#"package calc;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

class ProbeTest {
    @Test
    void adds() {
        assertEquals(5, Calc.add(2, 2));
    }
}
"#;

const P3: &str = r#"package calc;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

class ProbeTest {
    @Test
    void multiplies() {
        assertEquals(6, Calc.mul(2, 3));
    }
}
"#;

const P4: &str = r#"package calc;

import static org.junit.jupiter.api.Assertions.assertEquals;

import nosuch.Helper;
import org.junit.jupiter.api.Test;

class ProbeTest {
    @Test
    void adds() {
        assertEquals(4, Calc.add(2, 2));
    }
}
"#;

const P5: &str = r#"package calc;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Disabled;
import org.junit.jupiter.api.Test;

class ProbeTest {
    @Test
    void adds() {
        assertEquals(4, Calc.add(2, 2));
    }

    @Test
    @Disabled("not yet")
    void later() {
    }
}
"#;

const P6: &str = r#"package calc;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

class ProbeTest {
    int helper() {
        return Calc.add(1, 1);
    }
}
"#;

const P7: &str = r#"package calc;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

class ProbeTest {
    @Test
    void parses() {
        assertEquals(1, Calc.parse("x1"));
    }
}
"#;

const TEST_PATH: &str = "src/test/java/calc/ProbeTest.java";
const CODE_PATH: &str = "src/main/java/calc/Calc.java";

/// P1-P6 are stage 1 (the tests-first run), P3b and P7 stage 2 (the fix's run).
pub const PROBES: &[Probe] = &[
    Probe { id: "P1", stage: 1, truth: spec::PASSED, files: &[ProbeFile { path: TEST_PATH, text: P1 }, ProbeFile { path: CODE_PATH, text: CALC }] },
    Probe { id: "P2", stage: 1, truth: spec::ASSERTION, files: &[ProbeFile { path: TEST_PATH, text: P2 }, ProbeFile { path: CODE_PATH, text: CALC }] },
    Probe { id: "P3", stage: 1, truth: spec::MISSING, files: &[ProbeFile { path: TEST_PATH, text: P3 }, ProbeFile { path: CODE_PATH, text: CALC }] },
    Probe { id: "P4", stage: 1, truth: spec::COLLECTION, files: &[ProbeFile { path: TEST_PATH, text: P4 }, ProbeFile { path: CODE_PATH, text: CALC }] },
    Probe { id: "P5", stage: 1, truth: spec::PASSED, files: &[ProbeFile { path: TEST_PATH, text: P5 }, ProbeFile { path: CODE_PATH, text: CALC }] },
    Probe { id: "P6", stage: 1, truth: spec::NOTHING, files: &[ProbeFile { path: TEST_PATH, text: P6 }, ProbeFile { path: CODE_PATH, text: CALC }] },
    Probe { id: "P3b", stage: 2, truth: spec::PASSED, files: &[ProbeFile { path: TEST_PATH, text: P3 }, ProbeFile { path: CODE_PATH, text: CALC_WITH_MUL }] },
    Probe { id: "P7", stage: 2, truth: spec::EXCEPTION, files: &[ProbeFile { path: TEST_PATH, text: P7 }, ProbeFile { path: CODE_PATH, text: CALC }] },
];
