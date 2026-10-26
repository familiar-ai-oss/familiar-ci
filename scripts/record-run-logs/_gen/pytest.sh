. /tmp/claude-1000/-home-user-projects-familiar-ai-oss/ce46f81f-ec40-4c7d-885d-547ef9d911b3/scratchpad/run_logs/_gen/lib.sh
base() {  # $1 outcome ; common project
  fx pytest "$1"
  mk pyproject.toml <<'X'
[tool.pytest.ini_options]
pythonpath = ["src"]
testpaths = ["tests"]
X
  mk src/calc.py <<'X'
def add(a, b):
    return a + b


def parse(s):
    if not s.isdigit():
        raise ValueError(f"not a number: {s!r}")
    return int(s)
X
  setenv 'export PATH="$ROOT/.cache/venv/bin:$PATH"'
  setpre 'find . -name __pycache__ -prune -exec rm -rf {} +; rm -rf .pytest_cache'
  setver 'pytest --version'
  setcmd 'pytest'
}
okt() { mk tests/test_calc.py <<'X'
from calc import add


def test_add():
    assert add(2, 2) == 4
X
}
base passed; okt; rec
base tests_failed; mk tests/test_calc.py <<'X'
from calc import add


def test_add():
    assert add(2, 2) == 5
X
rec
base tests_failed_exception; mk tests/test_calc.py <<'X'
from calc import parse


def test_parse():
    assert parse("x1") == 1
X
rec
base collection_error; mk tests/test_calc.py <<'X'
import nosuchmodule
from calc import add


def test_add():
    assert add(2, 2) == 4
X
rec
base collection_error_syntax; mk tests/test_calc.py <<'X'
from calc import add


def test_add(:
    assert add(2, 2) == 4
X
rec
base missing_target_symbol; mk tests/test_calc.py <<'X'
from calc import mul


def test_mul():
    assert mul(2, 3) == 6
X
rec
base no_tests_collected_file; mk tests/test_calc.py <<'X'
from calc import add


def helper():
    return add(1, 1)
X
rec
base no_tests_collected_filter; okt; setcmd 'pytest -k nomatch'; rec
base setup_error; mk tests/test_calc.py <<'X'
import pytest
from calc import add


@pytest.fixture
def db():
    raise RuntimeError("db down")


def test_add(db):
    assert add(2, 2) == 4
X
rec
base usage_error; okt; setcmd 'pytest --nosuchflag'; rec
base environment_error_cmd_not_found; okt; setcmd 'pytestx'; rec
base environment_error_missing_dep; okt; mk src/calc.py <<'X'
import pendulum


def add(a, b):
    return a + b
X
rec
base runner_crash; okt; mk conftest.py <<'X'
def pytest_collection_modifyitems(items):
    raise RuntimeError("plugin bug in conftest hook")
X
rec
base runner_crash_conftest; okt; mk tests/conftest.py <<'X'
raise RuntimeError("conftest blew up at import")
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

from calc import add


def test_add():
    assert add(2, 2) == 4


def test_crash():
    $body
X
  [ $k = segv_shell ] && setcmd 'pytest; exit $?'
  rec
done
base big_run
python3 - <<'X'
import os
os.makedirs('tests', exist_ok=True)
for f in range(5):
    with open(f'tests/test_part{f}.py', 'w') as fh:
        fh.write('import pytest\nfrom calc import add\n\n\n@pytest.mark.parametrize("i", range(120))\ndef test_add(i):\n')
        if f == 2:
            fh.write('    assert add(i, 1) == (i + 1 if i != 60 else -1)\n')
        else:
            fh.write('    assert add(i, 1) == i + 1\n')
X
rec
base missing_target_symbol_attr; mk tests/test_calc.py <<'X'
import calc


def test_mul():
    assert calc.mul(2, 3) == 6
X
rec
base big_run_verbose; cp -r ../big_run/tests .; setcmd 'pytest -v'; rec
base tb_short_typo; setcmd 'pytest --tb=short'; mk tests/test_calc.py <<'X'
import calc


def test_add():
    assert calc.mull(2, 2) == 4
X
rec
