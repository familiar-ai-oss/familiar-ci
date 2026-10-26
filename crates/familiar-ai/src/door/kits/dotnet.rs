//! [draft] dotnet (VSTest), as the A0 spike proved it (SDK 8.0.426, VSTest 17.11.1, xunit 2.9.2):
//!
//! - Env only works: `VSTestLogger=trx%3BLogFileName=<run's folder>/results.trx` and
//!   `VSTestResultsDirectory=<the folder>` (MSBuild takes env vars as properties). The `;` MUST be
//!   escaped as `%3B`: with a raw `;` VSTest splits it and prints "The Test Logger URI
//!   'LogFileName=...' is not valid. The Test Logger will be ignored." and writes no TRX. It adds
//!   one console line, `Results File: <path>.trx`; otherwise the output is identical.
//! - The person's own `--logger` (or `-l`) beats the env property (a command-line global property
//!   beats an env one): the env reader reads nothing there. The added flag
//!   `--logger "trx;LogFileName=<folder>/results.trx"` works beside the person's own (several
//!   loggers are allowed): the `dotnet:trx-flag` reader, used when the command names its own.
//! - TRX is locale-proof (`DOTNET_CLI_UI_LANGUAGE=de` makes the console German, the TRX reads the
//!   same). The console is pinned for the tables all the same: `DOTNET_CLI_UI_LANGUAGE=en`,
//!   `MSBUILDTERMINALLOGGER=off` (SDK 8 does not enable the terminal logger for redirected
//!   output; later SDKs may), `DOTNET_NOLOGO=1`.
//! - P3 (CS0117) and P4 (CS0246) are build errors: no TRX, so a report reader asks and the tables
//!   read them. P6 (no test in the assembly) writes a TRX with no results: it reads right.
//!
//! Not run: Microsoft.Testing.Platform projects (`VSTestLogger` does not apply; MTP takes
//! `--report-trx` after `--`), NUnit, MSTest, SDK 9 and 10. The probe kit is xunit.

use super::spec::{self, EnvOp, Kind, Op, Probe, ProbeFile, Reader};

pub const ID: &str = "dotnet";
pub const LANGUAGE: &str = "csharp";

/// The TRX readers need no kit: VSTest writes the file.
pub const KIT_FILES: &[spec::KitFile] = &[];

pub const READERS: &[Reader] = &[
    Reader {
        id: "dotnet:trx",
        runner: ID,
        kind: Kind::Report,
        env: &[
            EnvOp { var: "VSTestLogger", op: Op::Set, value: "trx%3BLogFileName={REPORT_DIR}/results.trx", separator: "" },
            EnvOp { var: "VSTestResultsDirectory", op: Op::Set, value: "{REPORT_DIR}", separator: "" },
            EnvOp { var: "DOTNET_CLI_UI_LANGUAGE", op: Op::Set, value: "en", separator: "" },
            EnvOp { var: "MSBUILDTERMINALLOGGER", op: Op::Set, value: "off", separator: "" },
            EnvOp { var: "DOTNET_NOLOGO", op: Op::Set, value: "1", separator: "" },
        ],
        skip_when_command_has: &["--logger", "-l"],
        report_globs: &["{REPORT_DIR}/results.trx"],
        converter: "trx",
        cannot_read: &["P3", "P4"],
        ..Reader::NONE
    },
    Reader {
        id: "dotnet:trx-flag",
        runner: ID,
        kind: Kind::Report,
        env: &[
            EnvOp { var: "DOTNET_CLI_UI_LANGUAGE", op: Op::Set, value: "en", separator: "" },
            EnvOp { var: "MSBUILDTERMINALLOGGER", op: Op::Set, value: "off", separator: "" },
            EnvOp { var: "DOTNET_NOLOGO", op: Op::Set, value: "1", separator: "" },
        ],
        flags: &["--logger", "trx;LogFileName={REPORT_DIR}/results.trx"],
        only_when_command_has: &["--logger", "-l"],
        report_globs: &["{REPORT_DIR}/results.trx"],
        converter: "trx",
        cannot_read: &["P3", "P4"],
        ..Reader::NONE
    },
    Reader { id: "dotnet:tables", runner: ID, kind: Kind::Tables, ..Reader::NONE },
];

/// Selection flags `re_aim` drops (`--filter "FullyQualifiedName~Foo"`, `--filter=...`).
pub const DROP_SELECTION: &[&str] = &["--filter"];

/// Setup files whose change in a commit makes the choice stale.
pub const WATCHED: &[&str] = &["*.csproj", "**/*.csproj", "global.json", "Directory.*.props", "**/Directory.*.props", "*.runsettings", "**/*.runsettings"];

const CALC: &str = r#"namespace CalcLib;

public static class Calc
{
    public static int Add(int a, int b) => a + b;

    public static int Parse(string s)
    {
        return int.Parse(s);
    }
}
"#;

const CALC_WITH_MUL: &str = r#"namespace CalcLib;

public static class Calc
{
    public static int Add(int a, int b) => a + b;

    public static int Parse(string s)
    {
        return int.Parse(s);
    }

    public static int Mul(int a, int b) => a * b;
}
"#;

const P1: &str = r#"using Xunit;
using CalcLib;

namespace Tests;

public class ProbeTests
{
    [Fact]
    public void Adds()
    {
        Assert.Equal(4, Calc.Add(2, 2));
    }
}
"#;

const P2: &str = r#"using Xunit;
using CalcLib;

namespace Tests;

public class ProbeTests
{
    [Fact]
    public void Adds()
    {
        Assert.Equal(5, Calc.Add(2, 2));
    }
}
"#;

const P3: &str = r#"using Xunit;
using CalcLib;

namespace Tests;

public class ProbeTests
{
    [Fact]
    public void Multiplies()
    {
        Assert.Equal(6, Calc.Mul(2, 3));
    }
}
"#;

const P4: &str = r#"using Nosuch.Helpers;
using Xunit;
using CalcLib;

namespace Tests;

public class ProbeTests
{
    [Fact]
    public void Adds()
    {
        Assert.Equal(4, Calc.Add(2, 2));
    }
}
"#;

const P5: &str = r#"using Xunit;
using CalcLib;

namespace Tests;

public class ProbeTests
{
    [Fact]
    public void Adds()
    {
        Assert.Equal(4, Calc.Add(2, 2));
    }

    [Fact(Skip = "not yet")]
    public void Later()
    {
    }
}
"#;

const P6: &str = r#"using Xunit;
using CalcLib;

namespace Tests;

public class ProbeTests
{
    public int Helper() => Calc.Add(1, 1);
}
"#;

const P7: &str = r#"using Xunit;
using CalcLib;

namespace Tests;

public class ProbeTests
{
    [Fact]
    public void Parses()
    {
        Assert.Equal(1, Calc.Parse("x1"));
    }
}
"#;

const TEST_PATH: &str = "tests/ProbeTests.cs";
const CODE_PATH: &str = "src/Calc.cs";

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
