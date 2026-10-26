'use strict';
// Familiar's mocha load hook, added beside the reporter in Familiar's own run:
//   --require <kit>/mocha/load.cjs
// mocha loads the spec files before it constructs any reporter, so a file that fails
// to load (P4) never reaches the reporter: mocha prints "Exception during run" and
// exits 1. This wraps Mocha#loadFilesAsync to write session + collect_error + end.
const C = require('../node/common.cjs');
try {
  const M = require(require.resolve('mocha', { paths: [process.cwd()] }));
  const Mocha = M.Mocha || M.default || M;
  const orig = Mocha.prototype.loadFilesAsync;
  if (typeof orig === 'function' && !orig.__familiar) {
    const wrapped = async function (...a) {
      try {
        return await orig.apply(this, a);
      } catch (err) {
        try {
          const e = C.error(err);
          if (e.kind === 'missing_name' || /Cannot find module/.test(e.message)) e.kind = /Cannot find module/.test(e.message) ? 'exception' : e.kind;
          const stack = (err && err.requireStack) || [];
          const file = stack.length ? C.rel(stack[0]) : ((e.frames.slice(-1)[0] || {}).file || null);
          C.write({ type: 'session', v: 1, runner: 'mocha', runner_version: C.pkgVersion('mocha'),
            language: 'javascript', language_version: process.versions.node, plugins: [] });
          C.write({ type: 'collect_error', file, error: e });
          C.write({ type: 'end', exit: 1, counts: { passed: 0, failed: 0, skipped: 0, deselected: 0, errors: 1 } });
        } catch (x) { C.kitError(x); }
        throw err;
      }
    };
    wrapped.__familiar = true;
    Mocha.prototype.loadFilesAsync = wrapped;
  }
} catch (e) { C.kitError(e); }
