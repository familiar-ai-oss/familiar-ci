"""Familiar's unittest reporter, loaded through sitecustomize.

Injected by env only: this folder prepended to PYTHONPATH, FAMILIAR_AI_REPORT set to a
path outside the work tree. Python imports `sitecustomize` at startup; this one chains to
the next sitecustomize on sys.path (the person's own, or a distro's) so it does not
shadow it, then patches unittest so the first process that runs a TextTestRunner writes
the per-test result file (JSON Lines v1: session, test and collect_error lines, end).
Never prints, never raises; an internal error writes a kit_error line, which makes the
reader take the file as no structured output.

The file speaks only the schema's words: outcomes passed, failed, skipped, error (an
expected failure is skipped, with its reason; an unexpected success is failed), counts
passed, failed, skipped, deselected, errors. The end line's exit is the exit this
python's unittest.main will give: 5 for a run that found no tests only where this
python's unittest says so (3.12+), else 0.
"""
import os
import sys


def _chain():
    """Run the sitecustomize this one shadows (the person's or the distro's)."""
    try:
        import importlib.machinery
        import importlib.util
        here = os.path.dirname(os.path.abspath(__file__))
        rest = [p for p in sys.path if os.path.abspath(p or os.curdir) != here]
        spec = importlib.machinery.PathFinder.find_spec("sitecustomize", rest)
        if spec is not None and spec.origin and os.path.abspath(spec.origin) != os.path.abspath(__file__):
            mod = importlib.util.module_from_spec(spec)
            sys.modules["_familiar_ai_chained_sitecustomize"] = mod
            spec.loader.exec_module(mod)
    except Exception:
        pass


def _install():
    path = os.environ.get("FAMILIAR_AI_REPORT")
    if not path:
        return
    # a python the run's process started: that process has taken the file (run() below), and this
    # one pays for none of the kit's imports
    owner = os.environ.get("FAMILIAR_AI_UNITTEST_PID")
    if owner and owner != str(os.getpid()):
        return
    import json
    import traceback
    import unittest
    import unittest.loader
    import unittest.runner

    root = os.getcwd()
    # "pid" is the process that owns the file: the first to run tests (claimed in run()); -1 for a
    # python that is not the run, which writes nothing
    state = {"fh": None, "import_exc": {}, "passed": [], "done": False, "pid": None}

    def write(obj):
        try:
            if state["pid"] != os.getpid():
                return
            if state["fh"] is None:
                state["fh"] = open(path, "a", encoding="utf-8")
            state["fh"].write(json.dumps(obj, default=str) + "\n")
            state["fh"].flush()
        except Exception:
            pass

    def guard(fn):
        def inner(*a, **k):
            try:
                return fn(*a, **k)
            except Exception as e:
                write({"type": "kit_error", "where": fn.__name__, "message": f"{type(e).__name__}: {e}",
                       "trace": traceback.format_exc()[-800:]})
        return inner

    def rel(p):
        p = os.path.abspath(p)
        return os.path.relpath(p, root) if p.startswith(root + os.sep) else p

    def in_project(fn):
        if fn.startswith("<"):
            return False
        p = os.path.abspath(fn)
        return p.startswith(root + os.sep) and "site-packages" not in p

    def frames(tb):
        return [{"file": rel(f.f_code.co_filename), "line": ln, "function": f.f_code.co_name}
                for f, ln in traceback.walk_tb(tb) if in_project(f.f_code.co_filename)]

    def test_file(test):
        try:
            sub = getattr(unittest.case, "_SubTest", None)
            if sub is not None and isinstance(test, sub):
                test = test.test_case  # a subtest's own class is unittest's; its file is its test case's
            mod = sys.modules.get(type(test).__module__)
            return rel(mod.__file__) if mod and getattr(mod, "__file__", None) else None
        except Exception:
            return None

    def kind(exc, fr, own, setup=False):
        if setup:
            return "setup"
        deepest_own = bool(fr) and own is not None and fr[-1]["file"] == own
        msg = str(exc)
        if isinstance(exc, ModuleNotFoundError):
            return "exception"
        if isinstance(exc, ImportError) and "cannot import name" in msg:
            return "missing_name"
        if isinstance(exc, NameError) and deepest_own:
            return "missing_name"
        if isinstance(exc, AttributeError) and "has no attribute" in msg and deepest_own:
            return "missing_name"
        if isinstance(exc, AssertionError):
            return "assertion"
        return "exception"

    def error(err, own, setup=False):
        et, ev, tb = err
        fr = frames(tb)
        return {"type": et.__name__, "message": str(ev)[:2000], "kind": kind(ev, fr, own, setup), "frames": fr}

    # the loader turns an import failure into a _FailedTest holding only text: keep the real exception
    orig_failed_import = unittest.loader._make_failed_import_test

    def failed_import(name, suiteClass):
        try:
            state["import_exc"][name] = sys.exc_info()
        except Exception:
            pass
        return orig_failed_import(name, suiteClass)
    unittest.loader._make_failed_import_test = failed_import

    R = unittest.runner.TextTestResult
    orig = {n: getattr(R, n) for n in ("addSuccess", "addFailure", "addError", "addSkip",
                                       "addExpectedFailure", "addUnexpectedSuccess", "addSubTest")}
    counts = {"passed": 0, "failed": 0, "skipped": 0, "deselected": 0, "errors": 0}

    class UnexpectedSuccess(Exception):
        pass

    def tid(test):
        return test.id()

    @guard
    def on_test(test, outcome, err=None, reason=None, failure=False):
        if isinstance(test, unittest.loader._FailedTest):
            name = test._testMethodName
            real = state["import_exc"].get(name)
            e = error(real, None) if real and real[0] else error(err, None)
            mod = sys.modules.get(name)
            f = name.replace(".", os.sep) + ".py"
            # a module that failed to import: its own import line is the place; re-judge missing_name
            if real and real[0] and isinstance(real[1], ImportError) and not isinstance(real[1], ModuleNotFoundError) \
                    and "cannot import name" in str(real[1]):
                e["kind"] = "missing_name"
            write({"type": "collect_error", "file": f, "error": e})
            counts["errors"] += 1
            return
        if type(test).__name__ == "_ErrorHolder":  # setUpClass / setUpModule failure
            write({"type": "test", "id": test.description, "file": None, "line": None, "outcome": "error",
                   "skip_reason": None, "error": error(err, None, setup=True)})
            counts["errors"] += 1
            return
        own = test_file(test)
        rec = {"type": "test", "id": tid(test), "file": own, "line": None, "outcome": outcome,
               "skip_reason": reason, "error": None}
        if err is not None:
            rec["error"] = error(err, own)
        write(rec)
        counts[{"passed": "passed", "failed": "failed", "error": "errors", "skipped": "skipped"}[outcome]] += 1

    def addSuccess(self, test):
        on_test(test, "passed")
        return orig["addSuccess"](self, test)

    def addFailure(self, test, err):
        on_test(test, "failed", err)
        return orig["addFailure"](self, test, err)

    def addError(self, test, err):
        on_test(test, "error" if isinstance(test, unittest.loader._FailedTest) else "failed", err)
        return orig["addError"](self, test, err)

    def addSkip(self, test, reason):
        on_test(test, "skipped", reason=reason)
        return orig["addSkip"](self, test, reason)

    def addExpectedFailure(self, test, err):
        on_test(test, "skipped", reason="expected failure")
        return orig["addExpectedFailure"](self, test, err)

    def addUnexpectedSuccess(self, test):
        on_test(test, "failed", (UnexpectedSuccess, UnexpectedSuccess("expected failure, but the test passed"), None))
        return orig["addUnexpectedSuccess"](self, test)

    def addSubTest(self, test, subtest, err):
        if err is not None:
            on_test(subtest, "failed", err)
        return orig["addSubTest"](self, test, subtest, err)

    for n, f in (("addSuccess", addSuccess), ("addFailure", addFailure), ("addError", addError),
                 ("addSkip", addSkip), ("addExpectedFailure", addExpectedFailure),
                 ("addUnexpectedSuccess", addUnexpectedSuccess), ("addSubTest", addSubTest)):
        setattr(R, n, f)

    orig_run = unittest.runner.TextTestRunner.run

    def run(self, test):
        if state["pid"] is None:
            # The first process to run tests takes the file. A python that test starts leaves it
            # alone (the variable is inherited), and so does a python that only started this one
            # (poetry run, tox, nox): it loaded the kit but ran no tests.
            owner = os.environ.get("FAMILIAR_AI_UNITTEST_PID")
            if not owner or owner == str(os.getpid()):
                state["pid"] = os.getpid()
                os.environ["FAMILIAR_AI_UNITTEST_PID"] = str(os.getpid())
            else:
                state["pid"] = -1
        if not state["done"]:
            write({"type": "session", "v": 1, "runner": "unittest", "runner_version": sys.version.split()[0],
                   "language": "python", "language_version": sys.version.split()[0], "plugins": []})
        result = orig_run(self, test)
        if not state["done"]:
            state["done"] = True
            try:
                if result.testsRun == 0 and not result.skipped:
                    # unittest.main's _NO_TESTS_EXITCODE exists from 3.12; before it, no tests exit 0
                    code = getattr(sys.modules.get("unittest.main"), "_NO_TESTS_EXITCODE", 0)
                else:
                    code = 0 if result.wasSuccessful() else 1
                write({"type": "end", "exit": code, "counts": counts})
                state["fh"].close()
            except Exception:
                pass
        return result
    unittest.runner.TextTestRunner.run = run


try:
    _install()
except Exception:
    pass
_chain()
