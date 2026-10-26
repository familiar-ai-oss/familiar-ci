"""The per-test CALL RECORDER, shared by the pytest plugin and the kit's sitecustomize (one
module, no copy): active only when FAMILIAR_AI_TRACE=1.

The profiler records, per test (the env FAMILIAR_AI_TEST_ID, the nodeid the plugin puts there
for the test's call), every Python function called whose file is under the root (the env
FAMILIAR_AI_TRACE_ROOT, the plugin's rootpath), not in site-packages, not under a hidden
folder, and not under a folder named tests, test or testing: the key (file, function, def
line) keeps the last call index, a per-process counter incremented on every 'call' event.
A module's own execution is not a function called and is not recorded.

At exit the process writes what it recorded to $FAMILIAR_AI_TRACE_DIR/<pid>.json as
{test_id: [[file, function, line, index], ...]}, where the plugin (the run's controller)
merges the files with its own records at session finish.

Never prints, never raises: Python clears a profiler that raises, so the profiler's body is
wrapped in a catch-all, and every other part is guarded; a failure only misses a record.
"""
import atexit
import json
import os
import sys

TEST_FOLDERS = ("tests", "test", "testing")

_counter = 0          # this process's 'call' events, the recorder's index
_records = {}         # test id -> {(file, function, def line): last call index}
_rel_cache = {}       # file name -> its path under the root, or None
_atexit_done = False


def _root():
    return os.environ.get("FAMILIAR_AI_TRACE_ROOT") or os.getcwd()


def _rel(path):
    """`path` under the root, if it is one of the run's own files (under the root, not in
    site-packages, not under a hidden folder, not under a folder named tests/test/testing),
    else None. Cached per file name."""
    if not isinstance(path, str) or path.startswith("<"):
        return None  # <string>, <frozen ...>: made by the interpreter, never under the root
    v = _rel_cache.get(path)
    if v is not None:
        return v
    out = None
    try:
        p = os.path.abspath(path)
        r = os.path.abspath(_root())
        if p.startswith(r + os.sep):
            rel = os.path.relpath(p, r)
            if "site-packages" not in rel:
                dirs = rel.split(os.sep)[:-1]
                if not any(d.startswith(".") for d in dirs) and not any(d in TEST_FOLDERS for d in dirs):
                    out = rel.replace(os.sep, "/")
    except Exception:
        out = None
    _rel_cache[path] = out
    return out


def _record(frame, event):
    if event != "call":
        return
    global _counter
    _counter += 1
    code = frame.f_code
    if code.co_name == "<module>":
        return  # the module's own execution, not a function called
    rel = _rel(code.co_filename)
    if rel is None:
        return
    tid = os.environ.get("FAMILIAR_AI_TEST_ID")
    if not tid:
        return
    b = _records.get(tid)
    if b is None:
        b = {}
        _records[tid] = b
    key = (rel, code.co_name, code.co_firstlineno)
    prev = b.get(key)
    if prev is None or _counter > prev:
        b[key] = _counter


def _profiler(frame, event, arg):
    # never let an exception out of the profiler: Python clears a profiler that raises, and
    # the run must go on
    try:
        _record(frame, event)
    except BaseException:
        pass


def install():
    """The startup install (the kit's sitecustomize): only in a process the recorder tags
    (FAMILIAR_AI_TRACE=1 and FAMILIAR_AI_TEST_ID set, inherited from the test that starts
    it: the plugin tags the run's environment for the test's call)."""
    try:
        if os.environ.get("FAMILIAR_AI_TRACE") != "1" or not os.environ.get("FAMILIAR_AI_TEST_ID"):
            return
        rearm()
    except Exception:
        pass


def rearm():
    """(Re)set the profiler: the plugin calls it at the start of every test's call, so a
    cleared profiler (a test that called sys.setprofile, one Python cleared) is back before
    the test runs; a no-op when FAMILIAR_AI_TRACE is not 1. Registers the atexit write once,
    so a process that ends the usual way still leaves what it recorded."""
    global _atexit_done
    try:
        if os.environ.get("FAMILIAR_AI_TRACE") != "1":
            return
        sys.setprofile(_profiler)
        if not _atexit_done:
            atexit.register(_write)
            _atexit_done = True
    except Exception:
        pass


def _write():
    """At exit: what this process recorded, for the run's controller to merge."""
    if not _records:
        return
    d = os.environ.get("FAMILIAR_AI_TRACE_DIR")
    if not d:
        return
    try:
        os.makedirs(d, exist_ok=True)
        data = {tid: [[k[0], k[1], k[2], i] for k, i in b.items()] for tid, b in _records.items()}
        if not data:
            return
        p = os.path.join(d, "%d.json" % os.getpid())
        tmp = "%s.%d" % (p, os.getpid())
        with open(tmp, "w") as f:
            json.dump(data, f)
        os.replace(tmp, p)
    except Exception:
        pass


def data():
    """What this process recorded so far: {test id: [[file, function, def line, index], ...]}."""
    try:
        return {tid: [list(k) + [i] for k, i in b.items()] for tid, b in _records.items()}
    except Exception:
        return {}
