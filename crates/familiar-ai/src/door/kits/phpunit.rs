//! [draft] PHPUnit and Pest (Pest runs PHPUnit; same readers, Pest not recorded), as the A0 spike
//! proved it (PHP 8.5.9, PHPUnit 13.4.2, composer:2): an extension loads with no change to
//! `phpunit.xml`.
//!
//! - `phpunit:reporter`: env `PHP_INI_SCAN_DIR=":<kit>/phpunit/ini"` (the leading `:` keeps the
//!   image's own scan dir; appended to a person's own value, glued on as written) and the added
//!   flag `--extension "Familiar\PhpunitExtension"`. The ini sets `auto_prepend_file=<kit>/
//!   phpunit/prepend.php`, which only registers an autoloader for `Familiar\*` and does nothing
//!   when `FAMILIAR_AI_REPORT` is unset; the extension is a Tracer that writes the per-test
//!   result file. 8 of 8 probes right under plain, `PHPUNIT_COMPACT_OUTPUT=1` and `--testdox`,
//!   the report the same in all three, the terminal output byte-identical to the bare run.
//!   PHPUnit 10 to 13 share the extension API; only 13.4.2 is recorded; PHPUnit 9 has no event
//!   system (the interface is missing: the reader fails its calibration and the next one is used).
//! - `phpunit:report`: the added flag `--log-junit <folder>/junit.xml`, converted. Right on every
//!   probe but P4 (a `require` of a missing file, exit 255): the log file is left empty, so it
//!   asks. Skipped when the command has its own `--log-junit` (a second one replaces the first).
//! - `PHPUNIT_COMPACT_OUTPUT` is unset in every PHP reader (Familiar's own run only): the tables
//!   that read the console when no report could be read need the normal output.
//!
//! The kit was recorded loaded two ways; the env-only mode (a prepend that loads the person's own
//! autoloader early to register the tracer) worked but is not shipped: too intrusive. PHPUnit
//! never tells an extension its exit code: the kit's end line has `exit: null`, the reader uses
//! the process's. P4's missing `require` is caught by PHPUnit's Application with no event; the
//! kit tracks the phase and writes `collect_error` + `end` at shutdown ("aborted_while_loading_
//! tests"): a missing class at load time would also land there as `collection_error`, not
//! `missing_target_symbol`.

use super::spec::{self, EnvOp, KitFile, Kind, Op, Probe, ProbeFile, Reader};

pub const ID: &str = "phpunit";
pub const LANGUAGE: &str = "php";

/// The kit, relative to `<temp>/familiar-ai-kit-<version>/`; the ini is a template for that folder.
pub const KIT_FILES: &[KitFile] = &[
    KitFile { path: "phpunit/ext.php", text: include_str!("../../../kit/phpunit/ext.php"), template: false },
    KitFile { path: "phpunit/prepend.php", text: include_str!("../../../kit/phpunit/prepend.php"), template: false },
    KitFile { path: "phpunit/ini/zz-familiar-ai.ini", text: include_str!("../../../kit/phpunit/ini/zz-familiar-ai.ini"), template: true },
];

/// The kit's files as written under `kit_dir`: (path relative to it, text), the ini rendered.
pub fn rendered(kit_dir: &str) -> Vec<(&'static str, String)> {
    spec::rendered(KIT_FILES, kit_dir)
}

const NORMAL_OUTPUT: EnvOp = EnvOp { var: "PHPUNIT_COMPACT_OUTPUT", op: Op::Unset, value: "", separator: "" };

pub const READERS: &[Reader] = &[
    Reader {
        id: "phpunit:reporter",
        runner: ID,
        kind: Kind::Reporter,
        env: &[EnvOp { var: "PHP_INI_SCAN_DIR", op: Op::Append, value: ":{KIT}/phpunit/ini", separator: "" }, NORMAL_OUTPUT],
        flags: &["--extension", "Familiar\\PhpunitExtension"],
        report_globs: &["{REPORT}"],
        converter: "schema",
        ..Reader::NONE
    },
    Reader {
        id: "phpunit:report",
        runner: ID,
        kind: Kind::Report,
        env: &[NORMAL_OUTPUT],
        flags: &["--log-junit", "{REPORT_DIR}/junit.xml"],
        skip_when_command_has: &["--log-junit"],
        report_globs: &["{REPORT_DIR}/junit.xml"],
        converter: "junit",
        cannot_read: &["P4"],
        ..Reader::NONE
    },
    Reader { id: "phpunit:pinned", runner: ID, kind: Kind::Pinned, env: &[NORMAL_OUTPUT], ..Reader::NONE },
    Reader { id: "phpunit:tables", runner: ID, kind: Kind::Tables, ..Reader::NONE },
];

/// Selection flags `re_aim` drops (`--filter testFoo`, `--group slow`, and their `=` forms).
pub const DROP_SELECTION: &[&str] = &["--filter", "--group"];

/// Setup files whose change in a commit makes the choice stale.
pub const WATCHED: &[&str] = &["composer.json", "composer.lock", "phpunit.xml", "**/phpunit.xml"];

const CALC: &str = r#"<?php

declare(strict_types=1);

namespace App;

final class Calc
{
    public static function add(int $a, int $b): int
    {
        return $a + $b;
    }

    public static function parse(string $s): int
    {
        if (preg_match('/^\d+$/', $s) !== 1) {
            throw new \InvalidArgumentException("not a number: $s");
        }
        return (int) $s;
    }
}
"#;

const CALC_WITH_MUL: &str = r#"<?php

declare(strict_types=1);

namespace App;

final class Calc
{
    public static function add(int $a, int $b): int
    {
        return $a + $b;
    }

    public static function parse(string $s): int
    {
        if (preg_match('/^\d+$/', $s) !== 1) {
            throw new \InvalidArgumentException("not a number: $s");
        }
        return (int) $s;
    }

    public static function mul(int $a, int $b): int
    {
        return $a * $b;
    }
}
"#;

const P1: &str = r#"<?php

declare(strict_types=1);

require_once __DIR__ . '/../src/Calc.php';


use App\Calc;
use PHPUnit\Framework\TestCase;

final class ProbeTest extends TestCase
{
    public function testAdd(): void
    {
        $this->assertSame(4, Calc::add(2, 2));
    }
}
"#;

const P2: &str = r#"<?php

declare(strict_types=1);

require_once __DIR__ . '/../src/Calc.php';


use App\Calc;
use PHPUnit\Framework\TestCase;

final class ProbeTest extends TestCase
{
    public function testAdd(): void
    {
        $this->assertSame(5, Calc::add(2, 2));
    }
}
"#;

const P3: &str = r#"<?php

declare(strict_types=1);

require_once __DIR__ . '/../src/Calc.php';


use App\Calc;
use PHPUnit\Framework\TestCase;

final class ProbeTest extends TestCase
{
    public function testMul(): void
    {
        $this->assertSame(6, Calc::mul(2, 3));
    }
}
"#;

const P4: &str = r#"<?php

declare(strict_types=1);

require_once __DIR__ . '/../src/Calc.php';
require_once __DIR__ . '/../src/NosuchHelper.php';

use App\Calc;
use PHPUnit\Framework\TestCase;

final class ProbeTest extends TestCase
{
    public function testAdd(): void
    {
        $this->assertSame(4, Calc::add(2, 2));
    }
}
"#;

const P5: &str = r#"<?php

declare(strict_types=1);

require_once __DIR__ . '/../src/Calc.php';


use App\Calc;
use PHPUnit\Framework\TestCase;

final class ProbeTest extends TestCase
{
    public function testAdd(): void
    {
        $this->assertSame(4, Calc::add(2, 2));
    }
    public function testLater(): void
    {
        $this->markTestSkipped('not yet');
    }
}
"#;

const P6: &str = r#"<?php

declare(strict_types=1);

require_once __DIR__ . '/../src/Calc.php';


use App\Calc;
use PHPUnit\Framework\TestCase;

final class ProbeTest extends TestCase
{
    public function helper(): int
    {
        return 2;
    }
}
"#;

const P7: &str = r#"<?php

declare(strict_types=1);

require_once __DIR__ . '/../src/Calc.php';


use App\Calc;
use PHPUnit\Framework\TestCase;

final class ProbeTest extends TestCase
{
    public function testParse(): void
    {
        $this->assertSame(1, Calc::parse('x1'));
    }
}
"#;

const TEST_PATH: &str = "tests/ProbeTest.php";
const CODE_PATH: &str = "src/Calc.php";

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
