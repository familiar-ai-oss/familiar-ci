. /tmp/claude-1000/-home-user-projects-familiar-ai-oss/ce46f81f-ec40-4c7d-885d-547ef9d911b3/scratchpad/run_logs/_gen/lib.sh
base() {
  fx node-test "$1"
  mk package.json <<'X'
{ "name": "fx", "private": true }
X
  mk src/calc.js <<'X'
function add(a, b) {
  return a + b;
}

function parse(s) {
  if (!/^\d+$/.test(s)) {
    throw new Error(`not a number: ${s}`);
  }
  return Number(s);
}

module.exports = { add, parse };
X
  setver 'node --version'
  setcmd 'node --test'
}
okt() { mk test/calc.test.js <<'X'
const test = require('node:test');
const assert = require('node:assert');
const { add } = require('../src/calc');

test('adds', () => {
  assert.strictEqual(add(2, 2), 4);
});
X
}
base passed; okt; rec
base tests_failed; mk test/calc.test.js <<'X'
const test = require('node:test');
const assert = require('node:assert');
const { add } = require('../src/calc');

test('adds', () => {
  assert.strictEqual(add(2, 2), 5);
});
X
rec
base tests_failed_exception; mk test/calc.test.js <<'X'
const test = require('node:test');
const assert = require('node:assert');
const { parse } = require('../src/calc');

test('parses', () => {
  assert.strictEqual(parse('x1'), 1);
});
X
rec
base collection_error; mk test/calc.test.js <<'X'
const test = require('node:test');
const assert = require('node:assert');
const { add } = require('../src/calc');
const helper = require('./nosuchhelper');

test('adds', () => {
  assert.strictEqual(add(2, 2), 4);
});
X
rec
base collection_error_syntax; mk test/calc.test.js <<'X'
const test = require('node:test');
const assert = require('node:assert');
const { add } = require('../src/calc');

test('adds', () => {
  assert.strictEqual(add(2, 2), 4);
);
X
rec
base missing_target_symbol; mk src/calc.mjs <<'X'
export function add(a, b) {
  return a + b;
}
X
mk test/calc.test.mjs <<'X'
import test from 'node:test';
import assert from 'node:assert';
import { mul } from '../src/calc.mjs';

test('multiplies', () => {
  assert.strictEqual(mul(2, 3), 6);
});
X
rec
base missing_target_symbol_js; mk test/calc.test.js <<'X'
const test = require('node:test');
const assert = require('node:assert');
const { mul } = require('../src/calc');

test('multiplies', () => {
  assert.strictEqual(mul(2, 3), 6);
});
X
rec
base no_tests_collected_file; mk test/calc.test.js <<'X'
const { add } = require('../src/calc');

function helper() {
  return add(1, 1);
}
X
rec
base no_tests_collected_filter; okt; setcmd 'node --test --test-name-pattern=nomatch'; rec
base no_tests_collected_path; okt; setcmd 'node --test nomatchpath'; rec
base setup_error; mk test/calc.test.js <<'X'
const { describe, before, it } = require('node:test');
const assert = require('node:assert');
const { add } = require('../src/calc');

describe('calc', () => {
  before(() => {
    throw new Error('db down');
  });

  it('adds', () => {
    assert.strictEqual(add(2, 2), 4);
  });
});
X
rec
base usage_error; okt; setcmd 'node --test --nosuchflag'; rec
base environment_error_cmd_not_found; okt; setcmd 'nodex --test'; rec
base environment_error_missing_dep; okt; mk src/calc.js <<'X'
const leftPad = require('left-pad');

function add(a, b) {
  return a + b;
}

module.exports = { add, leftPad };
X
rec
base runner_crash; okt; setcmd 'node --test --test-reporter=./nosuch-reporter.mjs'; rec
for k in segv segv_shell abort sigkill; do
  base killed_$k
  case $k in
    segv|segv_shell) body="process.kill(process.pid, 'SIGSEGV');";;
    abort) body='process.abort();';;
    sigkill) body="process.kill(process.pid, 'SIGKILL');";;
  esac
  mk test/calc.test.js <<X
const test = require('node:test');
const assert = require('node:assert');
const { add } = require('../src/calc');

test('adds', () => {
  assert.strictEqual(add(2, 2), 4);
});

test('crashes', () => {
  $body
});
X
  [ $k = segv_shell ] && setcmd 'node --test; exit $?'
  rec
done
base killed_segv_no_isolation; mk test/calc.test.js <<'X'
const test = require('node:test');
const assert = require('node:assert');
const { add } = require('../src/calc');

test('adds', () => {
  assert.strictEqual(add(2, 2), 4);
});

test('crashes', () => {
  process.kill(process.pid, 'SIGSEGV');
});
X
setcmd 'node --test --experimental-test-isolation=none'; rec
base big_run
node --input-type=commonjs - <<'X'
const fs = require('fs');
fs.mkdirSync('test', { recursive: true });
for (let f = 0; f < 5; f++) {
  let s = "const test = require('node:test');\nconst assert = require('node:assert');\nconst { add } = require('../src/calc');\n\n";
  for (let i = 0; i < 120; i++) {
    const want = (f === 2 && i === 60) ? -1 : i + 1;
    s += `test('add ${f}-${i}', () => {\n  assert.strictEqual(add(${i}, 1), ${want});\n});\n\n`;
  }
  fs.writeFileSync(`test/part${f}.test.js`, s);
}
X
rec
