"""Familiar's pytest reporter (from the A0 spike, lane a-py).

Loaded only in Familiar's own run: PYTEST_ADDOPTS+=" -p familiar_ai_pytest" with this
folder prepended to PYTHONPATH. Writes the per-test result file (JSON Lines v1) to
$FAMILIAR_AI_REPORT. Never prints, never raises: an internal error writes a kit_error
line. Under xdist only the controller writes; workers attach the structured error to
each report (an attribute, serialised by xdist) because the controller has no excinfo.

With FAMILIAR_AI_TRACE=1 (off by default) the per-test CALL RECORDER comes with it: for
the test's call the test's nodeid is put in FAMILIAR_AI_TEST_ID (a child python inherits
it; the kit's sitecustomize installs the shared recorder, familiar_ai_calls, in it), and
the profiler is re-armed at the start of every test's call. At session finish the
controller merges its own records with the <pid>.json files the child processes (and a
xdist worker, which inherits the folder and attaches its calls to the report like the
error) leave in FAMILIAR_AI_TRACE_DIR and writes one calls line per test.
"""
import functools
import json
import os
import sys
import traceback

import pytest

# the recorder is a byproduct of the run (FAMILIAR_AI_TRACE=1): its absence never breaks the reporter
try:
    import familiar_ai_calls as _calls
except Exception:
    _calls = None

_ATTR = "familiar_ai_error"
_CALLS_ATTR = "familiar_ai_calls"
_state = {"fh": None, "seen": set(), "tests": {}, "counts": None, "root": None, "worker": False,
          "tracing": False, "calls": {}}


def _worker(config):
    return hasattr(config, "workerinput")


def _write(obj):
    fh = _state["fh"]
    if fh is None:
        return
    try:
        fh.write(json.dumps(obj, default=str) + "\n")
        fh.flush()
    except Exception:
        pass


def _guard(fn):
    @functools.wraps(fn)
    def inner(*a, **k):
        try:
            return fn(*a, **k)
        except Exception as e:  # never raise into the person's run
            _write({"type": "kit_error", "where": fn.__name__,
                    "message": f"{type(e).__name__}: {e}",
                    "trace": traceback.format_exc()[-800:]})
    return inner


def _try_file(f):
    """A try's file names are the project's (FAMILIAR_AI_TRACE_ROOT, named by the binary), not
    pytest's rootdir's, which may be the test files' folder; any other run: as pytest gives them."""
    try:
        root = os.environ.get("FAMILIAR_AI_TRACE_ROOT") if _state.get("tracing") else None
        if not root or not f:
            return f
        p = os.path.abspath(os.path.join(_state["root"] or os.getcwd(), str(f)))
        r = os.path.abspath(root)
        if p.startswith(r + os.sep):
            return os.path.relpath(p, r).replace(os.sep, "/")
        return f
    except Exception:
        return f


def _rel(path):
    try:
        p = os.path.abspath(str(path))
        root = _state["root"] or os.getcwd()
        if p.startswith(root + os.sep):
            return os.path.relpath(p, root)
        return p
    except Exception:
        return str(path)


def _in_project(path):
    if str(path).startswith("<"):
        return False
    p = os.path.abspath(str(path))
    root = _state["root"] or os.getcwd()
    return p.startswith(root + os.sep) and "site-packages" not in p and os.sep + "." not in p[len(root):]


def _chain(exc):
    """The exception and its causes, outermost first (CollectError wraps the ImportError)."""
    out, seen = [], set()
    while exc is not None and id(exc) not in seen:
        seen.add(id(exc))
        out.append(exc)
        exc = exc.__cause__ or exc.__context__
    return out


def _frames(tb):
    fr = []
    for f, lineno in traceback.walk_tb(tb):
        fn = f.f_code.co_filename
        if _in_project(fn):
            fr.append({"file": _rel(fn), "line": lineno, "function": f.f_code.co_name})
    return fr


def _kind(exc, frames, own_file, phase):
    if phase == "setup":
        return "setup"
    deepest_own = bool(frames) and own_file is not None and frames[-1]["file"] == own_file
    msg = str(exc)
    if isinstance(exc, ModuleNotFoundError):
        return "exception"
    if isinstance(exc, ImportError) and "cannot import name" in msg and deepest_own:
        return "missing_name"
    if isinstance(exc, (NameError,)) and deepest_own:
        return "missing_name"
    if isinstance(exc, AttributeError) and "has no attribute" in msg and deepest_own:
        return "missing_name"
    if isinstance(exc, AssertionError) or type(exc).__name__ == "Failed":
        return "assertion"
    return "exception"


def _error(excinfo, own_file, phase):
    chain = _chain(excinfo.value)
    # a CollectError carries the real cause: take the innermost non-pytest exception
    exc = chain[-1] if type(chain[0]).__name__ == "CollectError" and len(chain) > 1 else chain[0]
    frames = _frames(exc.__traceback__)
    return {"type": type(exc).__name__, "message": str(exc)[:2000],
            "kind": _kind(exc, frames, own_file, phase), "frames": frames}


def pytest_configure(config):
    try:
        _state["root"] = str(config.rootpath)
        _state["worker"] = _worker(config)
        if os.environ.get("FAMILIAR_AI_TRACE") == "1":
            _state["tracing"] = True
            _trace_configure()
        if _state["worker"] or not os.environ.get("FAMILIAR_AI_REPORT"):
            return
        _state["fh"] = open(os.environ["FAMILIAR_AI_REPORT"], "a", encoding="utf-8")
        _state["counts"] = {"passed": 0, "failed": 0, "skipped": 0, "deselected": 0,
                            "errors": 0, "xfailed": 0, "xpassed": 0}
    except Exception:
        _state["fh"] = None


def _trace_configure():
    # The run's environment for the recorder: its root (this process's rootpath, so a xdist
    # worker records under the same root), and, in the controller, a fresh folder for the
    # <pid>.json files the child processes (and a worker, which inherits it) leave.
    try:
        # the binary names the project's root when it knows it (a try): pytest's rootdir may be
        # the test files' folder, which holds none of the project's code
        os.environ["FAMILIAR_AI_TRACE_ROOT"] = os.environ.get("FAMILIAR_AI_TRACE_ROOT") or _state["root"] or os.getcwd()
        if not _state["worker"] and not os.environ.get("FAMILIAR_AI_TRACE_DIR"):
            import tempfile
            os.environ["FAMILIAR_AI_TRACE_DIR"] = tempfile.mkdtemp(prefix="familiar-ai-calls-")
    except Exception:
        pass


@pytest.hookimpl(trylast=True)
@_guard
def pytest_sessionstart(session):
    if _state["fh"] is None:
        return
    pm = session.config.pluginmanager
    plugins = sorted({f"{d.project_name}-{d.version}" for _, d in pm.list_plugin_distinfo()})
    _write({"type": "session", "v": 1, "runner": "pytest", "runner_version": pytest.__version__,
            "language": "python", "language_version": sys.version.split()[0], "plugins": plugins,
            "xdist": bool(getattr(session.config.option, "numprocesses", None))})


# --- workers and the in-process run: attach the structured error to the report ---

@pytest.hookimpl(hookwrapper=True)
def pytest_make_collect_report(collector):
    outcome = yield
    try:
        rep = outcome.get_result()
        call = rep.__dict__.get("call")
        if rep.failed and call is not None and call.excinfo is not None:
            own = _rel(collector.path) if getattr(collector, "path", None) else None
            setattr(rep, _ATTR, json.dumps(_error(call.excinfo, own, "collect"), default=str))
    except Exception:
        pass


@pytest.hookimpl(hookwrapper=True)
def pytest_runtest_call(item):
    # For the test's call, put the test's nodeid in the environment (a child python inherits
    # it and its calls go on the test's line) and re-arm the profiler (a test that called
    # sys.setprofile, or a profiler Python cleared, is back before the test runs).
    old = None
    tagged = False
    try:
        if _state["tracing"]:
            old = os.environ.get("FAMILIAR_AI_TEST_ID")
            os.environ["FAMILIAR_AI_TEST_ID"] = item.nodeid
            tagged = True
            if _calls is not None:
                _calls.rearm()
    except Exception:
        pass
    try:
        yield
    finally:
        if tagged:
            try:
                if old is None:
                    os.environ.pop("FAMILIAR_AI_TEST_ID", None)
                else:
                    os.environ["FAMILIAR_AI_TEST_ID"] = old
            except Exception:
                pass


@pytest.hookimpl(hookwrapper=True)
def pytest_runtest_makereport(item, call):
    outcome = yield
    try:
        rep = outcome.get_result()
        if rep.failed and call.excinfo is not None:
            setattr(rep, _ATTR, json.dumps(_error(call.excinfo, _rel(item.path), call.when), default=str))
        # a worker writes no report file: its records ride to the controller on the report
        # (an attribute, serialised by xdist), like the structured error
        if _state["tracing"] and _state["worker"] and call.when == "call" and _calls is not None:
            rows = _calls.data().get(item.nodeid)
            if rows:
                setattr(rep, _CALLS_ATTR, json.dumps({item.nodeid: rows}))
    except Exception:
        pass


# --- the controller (or the only process): write the file ---

@_guard
def pytest_collectreport(report):
    if _state["fh"] is None:
        return
    if report.failed:
        key = ("c", report.nodeid)
        if key in _state["seen"]:
            return  # every xdist worker collects; report each error once
        _state["seen"].add(key)
        err = json.loads(getattr(report, _ATTR, None) or "null") or {
            "type": None, "message": (report.longreprtext or "")[-500:], "kind": "exception", "frames": []}
        _write({"type": "collect_error", "file": _try_file(report.fspath), "error": err})
    elif report.skipped:
        _state["counts"]["skipped"] += 1


@_guard
def pytest_deselected(items):
    if _state["fh"] is not None:
        _state["counts"]["deselected"] += len(items)


@_guard
def pytest_runtest_logreport(report):
    if _state["fh"] is None:
        return
    # a xdist worker's records, attached to its report
    if _state["tracing"]:
        raw = getattr(report, _CALLS_ATTR, None)
        if isinstance(raw, str) and raw:
            try:
                _merge_calls(_state["calls"], json.loads(raw))
            except Exception:
                pass
    t = _state["tests"].setdefault(report.nodeid, {"nodeid": report.nodeid, "outcome": None,
                                                    "error": None, "skip_reason": None,
                                                    "loc": report.location})
    xfail = hasattr(report, "wasxfail")
    if report.failed:
        err = json.loads(getattr(report, _ATTR, None) or "null") or {
            "type": None, "message": (report.longreprtext or "")[-500:],
            "kind": "setup" if report.when == "setup" else "exception", "frames": []}
        if report.when == "call":
            t["outcome"], t["error"] = ("xpassed_strict" if xfail else "failed"), err
        elif t["outcome"] != "failed":
            t["outcome"], t["error"] = "error", err
    elif report.skipped and t["outcome"] is None:
        t["outcome"] = "xfailed" if xfail else "skipped"
        lr = report.longrepr
        t["skip_reason"] = lr[2] if isinstance(lr, tuple) and len(lr) == 3 else str(lr)
    elif report.passed and report.when == "call" and t["outcome"] is None:
        t["outcome"] = "xpassed" if xfail else "passed"
    if report.when == "teardown":
        _flush_test(t)


def _flush_test(t):
    if t.get("done"):
        return
    t["done"] = True
    out = t["outcome"] or "error"
    c = _state["counts"]
    c[{"passed": "passed", "failed": "failed", "skipped": "skipped", "error": "errors",
       "xfailed": "xfailed", "xpassed": "xpassed", "xpassed_strict": "failed"}[out]] += 1
    file, line = (t["loc"][0], t["loc"][1]) if t["loc"] else (None, None)
    _write({"type": "test", "id": t["nodeid"], "file": _try_file(file),
            "line": (line + 1) if isinstance(line, int) else None,
            "outcome": "failed" if out == "xpassed_strict" else out,
            "skip_reason": t["skip_reason"], "error": t["error"]})


def _merge_calls(merged, data):
    # data: {test id: [[file, function, def line, call index], ...]}; a key keeps the largest index
    for tid, rows in (data or {}).items():
        b = merged.setdefault(tid, {})
        for row in rows or []:
            try:
                f, n, ln, i = row[0], row[1], row[2], int(row[3])
            except Exception:
                continue
            cur = b.get((f, n, ln))
            if cur is None or i > cur:
                b[(f, n, ln)] = i


def _finish_calls():
    # At session finish (the controller alone has the file): merge this process's own records
    # with every <pid>.json in the run's folder (a child process, a xdist worker that
    # inherits it), and write one line per test, sorted by file and line. The calls never
    # break the run's own report.
    if not _state["tracing"] or _calls is None:
        return
    try:
        _merge_calls(_state["calls"], _calls.data())
        d = os.environ.get("FAMILIAR_AI_TRACE_DIR")
        if d:
            for name in os.listdir(d):
                if not (name.endswith(".json") and name[:-5].isdigit()):
                    continue
                try:
                    with open(os.path.join(d, name), encoding="utf-8") as f:
                        _merge_calls(_state["calls"], json.load(f))
                except Exception:
                    continue
        for tid, b in _state["calls"].items():
            rows = sorted(((f, n, ln, i) for (f, n, ln), i in b.items()), key=lambda r: (r[0], r[2], r[1]))
            _write({"type": "calls", "id": tid, "funcs": [[f, n, ln, i] for f, n, ln, i in rows]})
    except Exception:
        pass


@pytest.hookimpl(trylast=True)
@_guard
def pytest_sessionfinish(session, exitstatus):
    if _state["fh"] is None:
        return
    for t in _state["tests"].values():
        _flush_test(t)  # a test with no teardown report (a crashed worker)
    _finish_calls()
    _write({"type": "end", "exit": int(exitstatus), "counts": _state["counts"]})
    try:
        _state["fh"].close()
    finally:
        _state["fh"] = None
