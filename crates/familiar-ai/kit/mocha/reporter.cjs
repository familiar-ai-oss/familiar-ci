'use strict';
// Familiar's mocha reporter. Added in Familiar's own run only:
//   --reporter <kit>/mocha/reporter.cjs
// It wraps Spec, so the console the person sees is mocha's own spec output.
const C = require('../node/common.cjs');
let Mocha;
try { Mocha = require(require.resolve('mocha', { paths: [process.cwd()] })); } catch (e) { Mocha = null; C.kitError(e); }
const Base = Mocha ? Mocha.reporters.Spec : class { constructor() {} };

class FamiliarMocha extends Base {
  constructor(runner, options) {
    super(runner, options);
    try {
      const counts = { passed: 0, failed: 0, skipped: 0, deselected: 0, errors: 0 };
      const file = t => C.rel(t && (t.file || (t.parent && t.parent.file)));
      const id = t => file(t) + '::' + (t.fullTitle ? t.fullTitle() : t.title);
      runner.once('start', () => {
        try {
          C.write({ type: 'session', v: 1, runner: 'mocha', runner_version: C.pkgVersion('mocha'),
            language: 'javascript', language_version: process.versions.node, plugins: [] });
        } catch (e) { C.kitError(e); }
      });
      runner.on('pass', t => { try { counts.passed++; C.write({ type: 'test', id: id(t), file: file(t), line: null, outcome: 'passed', skip_reason: null, error: null }); } catch (e) { C.kitError(e); } });
      runner.on('pending', t => { try { counts.skipped++; C.write({ type: 'test', id: id(t), file: file(t), line: null, outcome: 'skipped', skip_reason: null, error: null }); } catch (e) { C.kitError(e); } });
      runner.on('fail', (t, err) => {
        try {
          const e = C.error(err);
          if (t.type === 'hook') e.kind = 'setup';
          counts.failed++;
          C.write({ type: 'test', id: id(t), file: file(t), line: null, outcome: 'failed', skip_reason: null, error: e });
        } catch (x) { C.kitError(x); }
      });
      runner.once('end', () => {
        try { C.write({ type: 'end', exit: Math.min(counts.failed, 255), counts }); } catch (e) { C.kitError(e); }
      });
    } catch (e) { C.kitError(e); }
  }
}
module.exports = FamiliarMocha;
