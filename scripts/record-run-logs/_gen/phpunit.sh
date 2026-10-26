. /tmp/claude-1000/-home-user-projects-familiar-ai-oss/ce46f81f-ec40-4c7d-885d-547ef9d911b3/scratchpad/run_logs/_gen/php-common.sh
RN=phpunit; BIN=vendor/bin/phpunit
gen_php
base setup_error; mk tests/CalcTest.php <<'X'
<?php

declare(strict_types=1);

require_once __DIR__ . '/../src/Calc.php';

use App\Calc;
use PHPUnit\Framework\TestCase;

final class CalcTest extends TestCase
{
    public static function setUpBeforeClass(): void
    {
        throw new \RuntimeException('db down');
    }

    public function testAdd(): void
    {
        $this->assertSame(4, Calc::add(2, 2));
    }
}
X
rec
# big_run: 600 tests in 5 classes (tests/Part{0..4}Test.php, one data-provided method
# over 120 cases each), ONE failing assertion in the middle class (Part2Test, i == 60)
bigtests() { python3 - <<'X'
import os
os.makedirs('tests', exist_ok=True)
for f in range(5):
    with open(f'tests/Part{f}Test.php', 'w') as fh:
        want = '$i === 60 ? -1 : $i + 1' if f == 2 else '$i + 1'
        fh.write(f'''<?php

declare(strict_types=1);

require_once __DIR__ . '/../src/Calc.php';

use App\\Calc;
use PHPUnit\\Framework\\Attributes\\DataProvider;
use PHPUnit\\Framework\\TestCase;

final class Part{f}Test extends TestCase
{{
    public static function numbers(): array
    {{
        return array_map(fn (int $i): array => [$i], range(0, 119));
    }}

    #[DataProvider('numbers')]
    public function testAdd(int $i): void
    {{
        $this->assertSame({want}, Calc::add($i, 1));
    }}
}}
''')
X
}
base big_run; bigtests; rec
base big_run_verbose; bigtests; setcmd "$(D_ "$BIN --testdox")"; rec
# H4 (lane h4cppphp, 2026-10-19): PHPUnit 13.4's --compact printer, and a skip beside a pass
phpt() { mkdir -p tests; printf '%s\n' '<?php' '' 'declare(strict_types=1);' '' "require_once __DIR__ . '/../src/Calc.php';" '' 'use App\Calc;' 'use PHPUnit\Framework\TestCase;' '' 'final class CalcTest extends TestCase' '{' '    public function testAdd(): void' '    {' '        $this->assertSame(4, Calc::add(2, 2));' '    }' '' '    public function testLater(): void' '    {' "        \$this->markTestSkipped('later');" '    }' '}' > tests/CalcTest.php; }
CPT="$(D_ "$BIN --compact")"
base passed_compact; t Add '$this->assertSame(4, Calc::add(2, 2));' ''; setcmd "$CPT"; rec
base passed_compact_skipped; phpt; setcmd "$CPT"; rec
base passed_skipped; phpt; rec
base tests_failed_compact; t Add '$this->assertSame(5, Calc::add(2, 2));' ''; setcmd "$CPT"; rec
base tests_failed_exception_compact; t Parse '$this->assertSame(1, Calc::parse("x1"));' ''; setcmd "$CPT"; rec
base missing_target_symbol_compact; t Mul '$this->assertSame(6, Calc::mul(2, 3));' ''; setcmd "$CPT"; rec
# H4 (lane h4cppphp, 2026-10-19): the other compact errors, as the default format's setup_error, collection_error and environment_error_missing_class
base setup_error_compact; mk tests/CalcTest.php <<'X'
<?php

declare(strict_types=1);

require_once __DIR__ . '/../src/Calc.php';

use App\Calc;
use PHPUnit\Framework\TestCase;

final class CalcTest extends TestCase
{
    public static function setUpBeforeClass(): void
    {
        throw new \RuntimeException('db down');
    }

    public function testAdd(): void
    {
        $this->assertSame(4, Calc::add(2, 2));
    }
}
X
setcmd "$CPT"; rec
base collection_error_compact; t Add '$this->assertSame(4, Calc::add(2, 2))' ''; setcmd "$CPT"; rec
base environment_error_missing_class_compact; t Add '$this->assertSame(4, Calc::add(2, 2));' ''; sed -i 's#        return \$a + \$b;#        return (new \\Carbon\\CarbonImmutable())->year * 0 + $a + $b;#' src/Calc.php; setcmd "$CPT"; rec
