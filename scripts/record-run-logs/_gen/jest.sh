. /tmp/claude-1000/-home-user-projects-familiar-ai-oss/ce46f81f-ec40-4c7d-885d-547ef9d911b3/scratchpad/run_logs/_gen/lib.sh
base() {
  fx jest "$1"
  ln -s ../../.cache/node/node_modules node_modules
  mk package.json <<'X'
{ "name": "fx", "private": true, "jest": { "cacheDirectory": ".jest-cache" } }
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
  setpre 'rm -rf .jest-cache'
  setver 'npx jest --version'
  setcmd 'npx jest'
}
okt() { mk test/calc.test.js <<'X'
const { add } = require('../src/calc');

test('adds', () => {
  expect(add(2, 2)).toBe(4);
});
X
}
base passed; okt; rec
base tests_failed; mk test/calc.test.js <<'X'
const { add } = require('../src/calc');

test('adds', () => {
  expect(add(2, 2)).toBe(5);
});
X
rec
base tests_failed_exception; mk test/calc.test.js <<'X'
const { parse } = require('../src/calc');

test('parses', () => {
  expect(parse('x1')).toBe(1);
});
X
rec
base collection_error; mk test/calc.test.js <<'X'
const { add } = require('../src/calc');
const helper = require('./nosuchhelper');

test('adds', () => {
  expect(add(2, 2)).toBe(4);
});
X
rec
base collection_error_syntax; mk test/calc.test.js <<'X'
const { add } = require('../src/calc');

test('adds', () => {
  expect(add(2, 2)).toBe(4);
);
X
rec
base missing_target_symbol; rm -f src/calc.js; mk src/calc.ts <<'X'
export function add(a: number, b: number): number {
  return a + b;
}
X
mk test/calc.test.ts <<'X'
import { add, mul } from '../src/calc';

test('multiplies', () => {
  expect(mul(2, 3)).toBe(6);
  expect(add(2, 3)).toBe(5);
});
X
mk jest.config.js <<'X'
module.exports = { preset: 'ts-jest', testEnvironment: 'node', cacheDirectory: '.jest-cache' };
X
mk tsconfig.json <<'X'
{ "compilerOptions": { "strict": true, "esModuleInterop": true, "module": "commonjs", "target": "es2020", "types": ["jest"] } }
X
mk package.json <<'X'
{ "name": "fx", "private": true }
X
rec
base missing_target_symbol_js; mk test/calc.test.js <<'X'
const { mul } = require('../src/calc');

test('multiplies', () => {
  expect(mul(2, 3)).toBe(6);
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
base no_tests_collected_filter; okt; setcmd 'npx jest -t nomatch'; rec
base no_tests_collected_path; okt; setcmd 'npx jest nomatchpath'; rec
base setup_error; mk test/calc.test.js <<'X'
const { add } = require('../src/calc');

beforeAll(() => {
  throw new Error('db down');
});

test('adds', () => {
  expect(add(2, 2)).toBe(4);
});
X
rec
base usage_error; okt; setcmd 'npx jest --nosuchflag'; rec
base environment_error_cmd_not_found; okt; setcmd 'jestx'; rec
base environment_error_npx_missing; okt; rm node_modules; setcmd 'npx --no jest'; setver 'npm --version'; rec
base environment_error_missing_dep; okt; mk src/calc.js <<'X'
const leftPad = require('left-pad');

function add(a, b) {
  return a + b;
}

module.exports = { add, leftPad };
X
rec
base runner_crash; okt; mk jest.config.js <<'X'
module.exports = { testEnvironment: 'nosuch-environment', cacheDirectory: '.jest-cache' };
X
mk package.json <<'X'
{ "name": "fx", "private": true }
X
rec
base runner_crash_config_throws; okt; mk jest.config.js <<'X'
throw new Error('config exploded');
X
mk package.json <<'X'
{ "name": "fx", "private": true }
X
rec
for k in segv segv_shell abort sigkill segv_workers; do
  base killed_$k
  case $k in
    segv|segv_shell|segv_workers) body="process.kill(process.pid, 'SIGSEGV');";;
    abort) body='process.abort();';;
    sigkill) body="process.kill(process.pid, 'SIGKILL');";;
  esac
  mk test/calc.test.js <<X
const { add } = require('../src/calc');

test('adds', () => {
  expect(add(2, 2)).toBe(4);
});

test('crashes', () => {
  $body
});
X
  [ $k = segv_shell ] && setcmd 'npx jest; exit $?'
  if [ $k = segv_workers ]; then okt; mv test/calc.test.js test/a.test.js; okt; mv test/calc.test.js test/b.test.js; mk test/calc.test.js <<X
const { add } = require('../src/calc');

test('crashes', () => {
  $body
});
X
  fi
  rec
done
base big_run
node - <<'X'
const fs = require('fs');
fs.mkdirSync('test', { recursive: true });
for (let f = 0; f < 10; f++) {
  let s = "const { add } = require('../src/calc');\n\n";
  for (let i = 0; i < 60; i++) {
    const want = (f === 5 && i === 30) ? -1 : i + 1;
    s += `test('add ${f}-${i}', () => {\n  expect(add(${i}, 1)).toBe(${want});\n});\n\n`;
  }
  fs.writeFileSync(`test/part${f}.test.js`, s);
}
X
rec
base big_run_verbose
node - <<'X'
const fs = require('fs');
fs.mkdirSync('test', { recursive: true });
for (let f = 0; f < 10; f++) {
  let s = "const { add } = require('../src/calc');\n\n";
  for (let i = 0; i < 60; i++) {
    const want = (f === 5 && i === 30) ? -1 : i + 1;
    s += `test('add ${f}-${i}', () => {\n  expect(add(${i}, 1)).toBe(${want});\n});\n\n`;
  }
  fs.writeFileSync(`test/part${f}.test.js`, s);
}
X
setcmd 'npx jest --verbose'
rec
