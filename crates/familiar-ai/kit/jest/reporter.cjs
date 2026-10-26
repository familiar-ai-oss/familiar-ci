'use strict';
// Familiar's jest reporter. Added in Familiar's own run only:
//   --reporters=default --reporters=<kit>/jest/reporter.cjs
const C = require('../node/common.cjs');

const NO_TESTS = /Your test suite must contain at least one test/;

class FamiliarJest {
  constructor(globalConfig) {
    try {
      this.cfg = globalConfig || {};
      this.counts = { passed: 0, failed: 0, skipped: 0, deselected: 0, errors: 0 };
    } catch (e) { C.kitError(e); }
  }
  onRunStart() {
    try {
      const plugins = [];
      for (const r of this.cfg.reporters || []) {
        const n = Array.isArray(r) ? r[0] : r;
        if (typeof n === 'string' && !n.includes('familiar')) plugins.push(n);
      }
      C.write({ type: 'session', v: 1, runner: 'jest', runner_version: C.pkgVersion('jest'),
        language: 'javascript', language_version: process.versions.node, plugins });
    } catch (e) { C.kitError(e); }
  }
  onTestResult(test, tr) {
    try {
      const file = C.rel(tr.testFilePath || (test && test.path));
      if (tr.testExecError) {
        const ex = tr.testExecError;
        const msg = C.strip(ex.message || '');
        if (NO_TESTS.test(msg) && !(tr.testResults || []).length) return; // nothing in the file: not an error
        const err = C.error({ name: ex.type || (msg.match(/^(\w*Error)/) || [])[1] || 'Error', message: msg, stack: ex.stack });
        if (/Cannot find module/.test(msg)) err.kind = 'exception';
        C.write({ type: 'collect_error', file, error: err });
        this.counts.errors++;
        return;
      }
      for (const t of tr.testResults || []) {
        const id = file + '::' + [...(t.ancestorTitles || []), t.title].join(' > ');
        let outcome = t.status;
        if (outcome === 'pending' || outcome === 'todo' || outcome === 'skipped' || outcome === 'disabled') outcome = 'skipped';
        const rec = { type: 'test', id, file, line: t.location ? t.location.line : null, outcome, skip_reason: null, error: null };
        if (outcome === 'failed') {
          const d = (t.failureDetails || [])[0];
          const m = (t.failureMessages || [])[0] || '';
          let err;
          if (d && typeof d === 'object' && (d.message || d.stack)) {
            err = C.error({ name: d.name || (d.matcherResult ? 'JestAssertionError' : (C.strip(m).match(/^(\w*Error)/) || [])[1]),
              message: d.message, stack: d.stack || m, matcherResult: d.matcherResult, code: d.code });
          } else err = C.error(m);
          rec.error = err;
          this.counts.failed++;
        } else if (outcome === 'passed') this.counts.passed++;
        else if (outcome === 'skipped') this.counts.skipped++;
        C.write(rec);
      }
    } catch (e) { C.kitError(e); }
  }
  onRunComplete(_ctx, results) {
    try {
      const bad = this.counts.failed + this.counts.errors > 0;
      const none = (results && results.numTotalTests === 0);
      const exit = bad ? 1 : (none && !this.cfg.passWithNoTests ? 1 : 0);
      C.write({ type: 'end', exit, counts: this.counts });
    } catch (e) { C.kitError(e); }
  }
  getLastError() { return undefined; }
}
module.exports = FamiliarJest;
