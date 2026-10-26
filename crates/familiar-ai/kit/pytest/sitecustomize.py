"""The pytest kit's sitecustomize: the per-test CALL RECORDER in a child Python process.

Loaded by `site` at startup when the kit's folder is on PYTHONPATH. It is the first
sitecustomize the run's python finds (the folder is prepended), so it must not shadow the
one a project of its own has: after its own setup it finds and runs any other
sitecustomize on sys.path (a failure of the setup never stops the chain, and nothing here
may break interpreter startup).

It is active only when FAMILIAR_AI_TRACE=1 and FAMILIAR_AI_TEST_ID is set in the process
(a child of a test the recorder is recording: the plugin puts the test's nodeid in the
environment for the test's call and the child inherits it): it then installs the shared
recorder (familiar_ai_calls), which records the calls under FAMILIAR_AI_TRACE_ROOT (the
plugin's rootpath) and, at exit, writes them to $FAMILIAR_AI_TRACE_DIR/<pid>.json as
{test_id: [[file, function, line, index], ...]}.
"""
import os
import sys

try:
    import familiar_ai_calls
    familiar_ai_calls.install()
except Exception:
    pass


def _chain():
    """Run the sitecustomize this one shadows (the project's or the distro's)."""
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


_chain()
