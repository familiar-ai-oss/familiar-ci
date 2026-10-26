// Familiar's vitest reporter. Added in Familiar's own run only:
//   --reporter=default --reporter=<kit>/vitest/reporter.mjs
// vitest >= 3 reporter API (onInit, onTestRunEnd with TestModule/TestCase); v5 checked.
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const C = require('../node/common.cjs');

const NO_TESTS = /No test suite found in file/;

export default class FamiliarVitest {
  onInit(vitest) {
    try {
      this.vitest = vitest;
      const plugins = [];
      for (const r of (vitest.config && vitest.config.reporters) || []) {
        const n = Array.isArray(r) ? r[0] : r;
        if (typeof n === 'string' && !n.includes('familiar') && !n.includes('kit/vitest')) plugins.push(n);
      }
      C.write({ type: 'session', v: 1, runner: 'vitest', runner_version: vitest.version || C.pkgVersion('vitest'),
        language: 'javascript', language_version: process.versions.node, plugins });
    } catch (e) { C.kitError(e); }
  }
  onTestRunEnd(testModules, unhandledErrors, reason) {
    try {
      const counts = { passed: 0, failed: 0, skipped: 0, deselected: 0, errors: 0 };
      for (const m of testModules || []) {
        const file = C.rel(m.moduleId);
        const merrs = (m.errors && m.errors()) || [];
        const real = merrs.filter(e => !NO_TESTS.test(C.strip(e && e.message)));
        for (const e of real) {
          C.write({ type: 'collect_error', file, error: C.error(e) });
          counts.errors++;
        }
        for (const t of m.children.allTests()) {
          const r = t.result();
          let outcome = r.state;
          if (outcome === 'pending') outcome = 'skipped';
          const rec = { type: 'test', id: file + '::' + t.fullName, file,
            line: t.location ? t.location.line : null, outcome, skip_reason: (r.note || null), error: null };
          if (outcome === 'failed') {
            rec.error = C.error((r.errors || [])[0] || null);
            counts.failed++;
          } else if (outcome === 'passed') counts.passed++;
          else if (outcome === 'skipped') counts.skipped++;
          C.write(rec);
        }
        // a hook that threw fails its suite and leaves the suite's tests skipped
        for (const s of m.children.allSuites()) {
          for (const e of (s.errors && s.errors()) || []) {
            C.write({ type: 'test', id: file + '::' + s.fullName + ' (suite)', file, line: s.location ? s.location.line : null,
              outcome: 'failed', skip_reason: null, error: C.error(e) });
            counts.failed++;
          }
        }
      }
      for (const e of unhandledErrors || []) {
        C.write({ type: 'collect_error', file: null, error: C.error(e) });
        counts.errors++;
      }
      const bad = counts.failed + counts.errors > 0;
      const none = counts.passed + counts.failed === 0;
      const pwnt = this.vitest && this.vitest.config && this.vitest.config.passWithNoTests;
      C.write({ type: 'end', exit: bad || reason === 'failed' ? 1 : (none && !pwnt && counts.skipped === 0 ? 1 : 0),
        counts, reason: reason || null });
    } catch (e) { C.kitError(e); }
  }
}
