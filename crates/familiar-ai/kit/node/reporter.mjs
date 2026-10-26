// Familiar's node --test reporter. Injected by env only:
//   NODE_OPTIONS+=" --test-reporter=<kit>/node/reporter.mjs --test-reporter-destination=<tmp>/node.out
//                   --test-reporter=tap --test-reporter-destination=stdout"
// It writes the schema to $FAMILIAR_AI_REPORT itself and yields nothing, so its
// destination stays empty. The destination must be a real file: /dev/null makes
// node 22 crash at exit (fsync EINVAL) and the run exit 1.
import { createRequire } from 'node:module';
import path from 'node:path';
const require = createRequire(import.meta.url);
const C = require('./common.cjs');

export default async function* familiarNode(source) {
  const counts = { passed: 0, failed: 0, skipped: 0, deselected: 0, errors: 0 };
  const stderrByFile = new Map();
  const stack = []; // names of open suites by nesting
  let started = false;
  const session = () => {
    if (started) return;
    started = true;
    C.write({ type: 'session', v: 1, runner: 'node-test', runner_version: process.versions.node,
      language: 'javascript', language_version: process.versions.node, plugins: [] });
  };
  try {
    for await (const ev of source) {
      try {
        session();
        const d = ev.data || {};
        if (ev.type === 'test:stderr' && d.file) {
          stderrByFile.set(d.file, (stderrByFile.get(d.file) || '') + (d.message || ''));
          continue;
        }
        if (ev.type === 'test:start') { stack[d.nesting] = d.name; stack.length = d.nesting + 1; continue; }
        if (ev.type !== 'test:pass' && ev.type !== 'test:fail') continue;
        const file = C.rel(d.file);
        if (d.details && d.details.type === 'suite') {
          // a suite fails for its own hook (hookFailed, said once, on the suite: its tests are only
          // cancelled) or because a test in it failed (subtestsFailed: those tests say so themselves)
          const serr = d.details.error || {};
          if (ev.type === 'test:fail' && serr.failureType === 'hookFailed') {
            const e = C.error(serr.cause && typeof serr.cause === 'object' ? serr.cause : serr);
            e.kind = 'setup';
            const inSuite = stack.slice(0, d.nesting).filter(Boolean).concat([d.name]);
            counts.failed++;
            C.write({ type: 'test', id: file + '::' + inSuite.join(' > ') + ' (hook)', file, line: d.line || null,
              outcome: 'failed', skip_reason: null, error: e });
          }
          continue;
        }
        // node's synthetic per-file test: a file that ran no test of its own (nothing
        // ran: pass) or that failed before any test (a load error: fail).
        const synthetic = d.nesting === 0 && file && (d.name === file || d.name === d.file || path.resolve(d.name) === path.resolve(d.file || ''))
          && d.line === 1 && d.column === 1;
        if (synthetic) {
          if (ev.type === 'test:fail') {
            const err = (d.details || {}).error || {};
            const cause = err.cause;
            const se = stderrByFile.get(file) || stderrByFile.get(d.file) || '';
            let e;
            const m = se.match(/^(\w*Error)(?: \[\w+\])?: (.*)$/m);
            if (m) e = C.error({ name: m[1], message: m[2], stack: se.slice(se.indexOf(m[0])) });
            else if (cause && typeof cause === 'object' && cause.message) e = C.error(cause);
            else e = { type: null, message: String(err.message || 'test failed'), kind: 'exception', frames: [] };
            if (/Cannot find module|ERR_MODULE_NOT_FOUND/.test(e.message)) e.kind = 'exception';
            C.write({ type: 'collect_error', file, error: e });
            counts.errors++;
          }
          continue;
        }
        const names = stack.slice(0, d.nesting).filter(Boolean).concat([d.name]);
        const rec = { type: 'test', id: file + '::' + names.join(' > '), file, line: d.line || null,
          outcome: 'passed', skip_reason: null, error: null };
        if (ev.type === 'test:fail') {
          const err = (d.details || {}).error || {};
          const real = err.cause && typeof err.cause === 'object' ? err.cause : err;
          rec.outcome = 'failed';
          rec.error = C.error(real);
          if (err.failureType === 'hookFailed') rec.error.kind = 'setup';
          counts.failed++;
        } else if (d.skip !== undefined || d.todo !== undefined) {
          rec.outcome = 'skipped';
          rec.skip_reason = typeof d.skip === 'string' ? d.skip : (typeof d.todo === 'string' ? d.todo : null);
          counts.skipped++;
        } else counts.passed++;
        C.write(rec);
      } catch (e) { C.kitError(e); }
    }
    session();
    C.write({ type: 'end', exit: counts.failed + counts.errors > 0 ? 1 : 0, counts });
  } catch (e) { C.kitError(e); }
}
