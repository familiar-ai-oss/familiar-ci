. /tmp/claude-1000/-home-user-projects-familiar-ai-oss/ce46f81f-ec40-4c7d-885d-547ef9d911b3/scratchpad/run_logs/_gen/lib.sh
base() {
  fx unittest "$1"
  mk calc.py <<'X'
def add(a, b):
    return a + b


def parse(s):
    if not s.isdigit():
        raise ValueError(f"not a number: {s!r}")
    return int(s)
X
  mk tests/__init__.py </dev/null
  setpre 'find . -name __pycache__ -prune -exec rm -rf {} +'
  setver 'python3 --version'
  setcmd 'python3 -m unittest'
}
okt() { mk tests/test_calc.py <<'X'
import unittest

from calc import add


class CalcTest(unittest.TestCase):
    def test_add(self):
        self.assertEqual(add(2, 2), 4)
X
}
base passed; okt; rec
base tests_failed; mk tests/test_calc.py <<'X'
import unittest

from calc import add


class CalcTest(unittest.TestCase):
    def test_add(self):
        self.assertEqual(add(2, 2), 5)
X
rec
base tests_failed_exception; mk tests/test_calc.py <<'X'
import unittest

from calc import parse


class CalcTest(unittest.TestCase):
    def test_parse(self):
        self.assertEqual(parse("x1"), 1)
X
rec
base collection_error; mk tests/test_calc.py <<'X'
import unittest

import nosuchmodule


class CalcTest(unittest.TestCase):
    def test_add(self):
        self.assertEqual(1, 1)
X
rec
base collection_error_syntax; mk tests/test_calc.py <<'X'
import unittest


class CalcTest(unittest.TestCase):
    def test_add(self:
        self.assertEqual(1, 1)
X
rec
base missing_target_symbol; mk tests/test_calc.py <<'X'
import unittest

from calc import mul


class CalcTest(unittest.TestCase):
    def test_mul(self):
        self.assertEqual(mul(2, 3), 6)
X
rec
base no_tests_collected_file; mk tests/test_calc.py <<'X'
import unittest

from calc import add


class CalcTest(unittest.TestCase):
    def helper(self):
        return add(1, 1)
X
rec
base no_tests_collected_filter; okt; setcmd 'python3 -m unittest -k nomatch'; rec
base setup_error; mk tests/test_calc.py <<'X'
import unittest

from calc import add


class CalcTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        raise RuntimeError("db down")

    def test_add(self):
        self.assertEqual(add(2, 2), 4)
X
rec
base setup_error_setUp; mk tests/test_calc.py <<'X'
import unittest

from calc import add


class CalcTest(unittest.TestCase):
    def setUp(self):
        raise RuntimeError("db down")

    def test_add(self):
        self.assertEqual(add(2, 2), 4)
X
rec
base usage_error; okt; setcmd 'python3 -m unittest --nosuchflag'; rec
base environment_error_cmd_not_found; okt; setcmd 'python3x -m unittest'; rec
base environment_error_missing_dep; okt; mk calc.py <<'X'
import pendulum


def add(a, b):
    return a + b
X
rec
base runner_crash; okt; mk tests/test_calc.py <<'X'
import unittest


def load_tests(loader, tests, pattern):
    return 42


class CalcTest(unittest.TestCase):
    def test_add(self):
        self.assertEqual(1, 1)
X
rec
for k in segv segv_shell abort sigkill; do
  base killed_$k
  case $k in
    segv|segv_shell) body='os.kill(os.getpid(), signal.SIGSEGV)';;
    abort) body='os.abort()';;
    sigkill) body='os.kill(os.getpid(), signal.SIGKILL)';;
  esac
  mk tests/test_calc.py <<X
import os
import signal
import unittest

from calc import add


class CalcTest(unittest.TestCase):
    def test_add(self):
        self.assertEqual(add(2, 2), 4)

    def test_crash(self):
        $body
X
  [ $k = segv_shell ] && setcmd 'python3 -m unittest; exit $?'
  rec
done
# big_run: 600 tests in 5 files (one TestCase class each, 120 methods), ONE failing
# assertion in the middle file (test_part2, i == 60)
bigtests() { python3 - <<'X'
for f in range(5):
    with open(f'tests/test_part{f}.py', 'w') as fh:
        fh.write(f'import unittest\n\nfrom calc import add\n\n\nclass Part{f}Test(unittest.TestCase):\n')
        for i in range(120):
            want = -1 if (f == 2 and i == 60) else i + 1
            fh.write(f'    def test_add_{i:03d}(self):\n        self.assertEqual(add({i}, 1), {want})\n\n')
X
}
base big_run; bigtests; rec
base big_run_verbose; bigtests; setcmd 'python3 -m unittest -v'; rec
