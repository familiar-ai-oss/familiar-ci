. /tmp/claude-1000/-home-user-projects-familiar-ai-oss/ce46f81f-ec40-4c7d-885d-547ef9d911b3/scratchpad/run_logs/_gen/php-common.sh
RN=pest; BIN=vendor/bin/pest
gen_php
base setup_error; mk tests/CalcTest.php <<'X'
<?php

declare(strict_types=1);

require_once __DIR__ . '/../src/Calc.php';

use App\Calc;

beforeAll(function () {
    throw new \RuntimeException('db down');
});

test('Add', function () {
    expect(Calc::add(2, 2))->toBe(4);
});
X
rec
# big_run: 600 tests in 5 files (tests/Part{0..4}Test.php, one test() over a 120-case
# dataset each), ONE failing expectation in the middle file (Part2Test, i == 60)
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

test('add', function (int $i) {{
    expect(Calc::add($i, 1))->toBe({want});
}})->with(range(0, 119));
''')
X
}
base big_run; bigtests; rec
# H4 (lane h4cppphp, 2026-10-19): --compact and --ci, and a skip beside a pass
pestt() { mkdir -p tests; printf '%s\n' '<?php' '' 'declare(strict_types=1);' '' "require_once __DIR__ . '/../src/Calc.php';" '' 'use App\Calc;' '' "test('Add', function () {" '    expect(Calc::add(2, 2))->toBe(4);' '});' '' "test('Later', function () {" '    expect(true)->toBeTrue();' "})->skip('later');" > tests/CalcTest.php; }
CPT="$(D_ "$BIN --compact")"; CIT="$(D_ "$BIN --ci")"
base passed_compact; t Add '' 'expect(Calc::add(2, 2))->toBe(4);'; setcmd "$CPT"; rec
base passed_ci; t Add '' 'expect(Calc::add(2, 2))->toBe(4);'; setcmd "$CIT"; rec
base passed_skipped; pestt; rec
base tests_failed_compact; t Add '' 'expect(Calc::add(2, 2))->toBe(5);'; setcmd "$CPT"; rec
base tests_failed_ci; t Add '' 'expect(Calc::add(2, 2))->toBe(5);'; setcmd "$CIT"; rec
base tests_failed_exception_compact; t Parse '' 'expect(Calc::parse("x1"))->toBe(1);'; setcmd "$CPT"; rec
base missing_target_symbol_compact; t Mul '' 'expect(Calc::mul(2, 3))->toBe(6);'; setcmd "$CPT"; rec
