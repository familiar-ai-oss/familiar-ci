. /tmp/claude-1000/-home-user-projects-familiar-ai-oss/ce46f81f-ec40-4c7d-885d-547ef9d911b3/scratchpad/run_logs/_gen/lib.sh
IMG=composer:2
D_() { printf "../../_bin/dock $IMG '%s'" "$1"; }
# RN = phpunit | pest ; BIN = vendor/bin/phpunit | vendor/bin/pest
base() {
  fx "$RN" "$1"
  # pest derives its root from the real path of vendor/, so it gets a hard-linked copy; phpunit a symlink
  if [ "$RN" = pest ]; then cp -al "../../.cache/php/$RN/vendor" vendor; else ln -s "../../.cache/php/$RN/vendor" vendor; fi
  mk phpunit.xml <<'X'
<?xml version="1.0" encoding="UTF-8"?>
<phpunit bootstrap="vendor/autoload.php" colors="false">
  <testsuites>
    <testsuite name="default">
      <directory>tests</directory>
    </testsuite>
  </testsuites>
</phpunit>
X
  mk src/Calc.php <<'X'
<?php

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
X
  setpre 'rm -rf .phpunit.cache .phpunit.result.cache'
  setver "$(D_ "php --version | head -1; $BIN --version")"
  setcmd "$(D_ "$BIN")"
}
# t BODY : one test whose body is BODY (phpunit: a TestCase method; pest: a test() closure)
t() { mkdir -p tests
  if [ "$RN" = phpunit ]; then
    printf '%s\n' '<?php' '' 'declare(strict_types=1);' '' "require_once __DIR__ . '/../src/Calc.php';" '' 'use App\Calc;' 'use PHPUnit\Framework\TestCase;' '' 'final class CalcTest extends TestCase' '{' "    public function test$1(): void" '    {' "        $2" '    }' '}' > tests/CalcTest.php
  else
    printf '%s\n' '<?php' '' 'declare(strict_types=1);' '' "require_once __DIR__ . '/../src/Calc.php';" '' 'use App\Calc;' '' "test('$1', function () {" "    $3" '});' > tests/CalcTest.php
  fi
}
gen_php() {
base passed; t Add '$this->assertSame(4, Calc::add(2, 2));' 'expect(Calc::add(2, 2))->toBe(4);'; rec
base tests_failed; t Add '$this->assertSame(5, Calc::add(2, 2));' 'expect(Calc::add(2, 2))->toBe(5);'; rec
base tests_failed_exception; t Parse '$this->assertSame(1, Calc::parse("x1"));' 'expect(Calc::parse("x1"))->toBe(1);'; rec
base collection_error; t Add '$this->assertSame(4, Calc::add(2, 2))' 'expect(Calc::add(2, 2))->toBe(4)'; rec
base collection_error_import; t Add '$this->assertSame(4, Calc::add(2, 2));' 'expect(Calc::add(2, 2))->toBe(4);'; sed -i "s#^require_once __DIR__ . '/../src/Calc.php';#&\nrequire_once __DIR__ . '/../src/NosuchHelper.php';#" tests/CalcTest.php; rec
base missing_target_symbol; t Mul '$this->assertSame(6, Calc::mul(2, 3));' 'expect(Calc::mul(2, 3))->toBe(6);'; rec
base no_tests_collected_file; mkdir -p tests
if [ "$RN" = phpunit ]; then printf '%s\n' '<?php' '' 'declare(strict_types=1);' '' 'use PHPUnit\Framework\TestCase;' '' 'final class CalcTest extends TestCase' '{' '    public function helper(): int' '    {' '        return 2;' '    }' '}' > tests/CalcTest.php
else printf '%s\n' '<?php' '' 'declare(strict_types=1);' '' 'function helper(): int' '{' '    return 2;' '}' > tests/CalcTest.php; fi; rec
base no_tests_collected_filter; t Add '$this->assertSame(4, Calc::add(2, 2));' 'expect(Calc::add(2, 2))->toBe(4);'; setcmd "$(D_ "$BIN --filter nomatch")"; rec
base usage_error; t Add '$this->assertSame(4, Calc::add(2, 2));' 'expect(Calc::add(2, 2))->toBe(4);'; setcmd "$(D_ "$BIN --nosuchflag")"; rec
base environment_error_cmd_not_found; t Add '$this->assertSame(4, Calc::add(2, 2));' 'expect(Calc::add(2, 2))->toBe(4);'; setcmd "$(D_ "${BIN}x")"; rec
base environment_error_interpreter_missing; t Add '$this->assertSame(4, Calc::add(2, 2));' 'expect(Calc::add(2, 2))->toBe(4);'; setcmd "$BIN"; setver 'command -v php || echo "php: not installed on this host (the point of this fixture)"'; rec
base environment_error_missing_dep; t Add '$this->assertSame(4, Calc::add(2, 2));' 'expect(Calc::add(2, 2))->toBe(4);'; rm -rf vendor; setver "$(D_ 'php --version | head -1')"; rec
base environment_error_missing_class; t Add '$this->assertSame(4, Calc::add(2, 2));' 'expect(Calc::add(2, 2))->toBe(4);'; sed -i 's#        return \$a + \$b;#        return (new \\Carbon\\CarbonImmutable())->year * 0 + $a + $b;#' src/Calc.php; rec
base runner_crash; t Add '$this->assertSame(4, Calc::add(2, 2));' 'expect(Calc::add(2, 2))->toBe(4);'; printf '%s\n' '<?xml version="1.0" encoding="UTF-8"?>' '<phpunit bootstrap="vendor/autoload.php">' '  <testsuites>' '    <testsuite name="default">' '      <directory>tests</directory>' '  </testsuites>' '</phpunit>' > phpunit.xml; rec
for k in segv segv_shell abort sigkill; do
  base killed_$k
  case $k in segv|segv_shell) sig=11;; abort) sig=6;; sigkill) sig=9;; esac  # numeric: SIG* constants need ext-pcntl, absent here
  t Crash "posix_kill(getmypid(), $sig); sleep(5);" "posix_kill(getmypid(), $sig); sleep(5);"
  [ $k = segv_shell ] && setcmd "$(D_ "$BIN"); exit \$?"
  rec
done
}
