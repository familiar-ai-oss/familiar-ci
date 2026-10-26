. /tmp/claude-1000/-home-user-projects-familiar-ai-oss/ce46f81f-ec40-4c7d-885d-547ef9d911b3/scratchpad/run_logs/_gen/lib.sh
base() {
  fx vitest "$1"
  ln -s ../../.cache/node/node_modules node_modules
  mk package.json <<'X'
{ "name": "fx", "private": true, "type": "module" }
X
  mk src/calc.js <<'X'
export function add(a, b) {
  return a + b;
}

export function parse(s) {
  if (!/^\d+$/.test(s)) {
    throw new Error(`not a number: ${s}`);
  }
  return Number(s);
}
X
  setver 'npx vitest --version'
  setcmd 'npx vitest run'
}
okt() { mk test/calc.test.js <<'X'
import { expect, test } from 'vitest';
import { add } from '../src/calc.js';

test('adds', () => {
  expect(add(2, 2)).toBe(4);
});
X
}
base passed; okt; rec
base tests_failed; mk test/calc.test.js <<'X'
import { expect, test } from 'vitest';
import { add } from '../src/calc.js';

test('adds', () => {
  expect(add(2, 2)).toBe(5);
});
X
rec
base tests_failed_exception; mk test/calc.test.js <<'X'
import { expect, test } from 'vitest';
import { parse } from '../src/calc.js';

test('parses', () => {
  expect(parse('x1')).toBe(1);
});
X
rec
base collection_error; mk test/calc.test.js <<'X'
import { expect, test } from 'vitest';
import { add } from '../src/calc.js';
import { helper } from './nosuchhelper.js';

test('adds', () => {
  expect(add(2, 2)).toBe(4);
});
X
rec
base collection_error_syntax; mk test/calc.test.js <<'X'
import { expect, test } from 'vitest';
import { add } from '../src/calc.js';

test('adds', () => {
  expect(add(2, 2)).toBe(4);
);
X
rec
base missing_target_symbol; mk test/calc.test.js <<'X'
import { expect, test } from 'vitest';
import { mul } from '../src/calc.js';

test('multiplies', () => {
  expect(mul(2, 3)).toBe(6);
});
X
rec
base missing_target_symbol_ts; rm src/calc.js; mk src/calc.ts <<'X'
export function add(a: number, b: number): number {
  return a + b;
}
X
mk test/calc.test.ts <<'X'
import { expect, test } from 'vitest';
import { add, mul } from '../src/calc';

test('multiplies', () => {
  expect(mul(2, 3)).toBe(6);
  expect(add(2, 3)).toBe(5);
});
X
mk tsconfig.json <<'X'
{ "compilerOptions": { "strict": true, "module": "esnext", "moduleResolution": "bundler", "target": "es2022", "noEmit": true } }
X
rec
base no_tests_collected_file; mk test/calc.test.js <<'X'
import { add } from '../src/calc.js';

export function helper() {
  return add(1, 1);
}
X
rec
base no_tests_collected_filter; okt; setcmd 'npx vitest run -t nomatch'; rec
base no_tests_collected_path; okt; setcmd 'npx vitest run nomatchpath'; rec
base setup_error; mk test/calc.test.js <<'X'
import { beforeAll, expect, test } from 'vitest';
import { add } from '../src/calc.js';

beforeAll(() => {
  throw new Error('db down');
});

test('adds', () => {
  expect(add(2, 2)).toBe(4);
});
X
rec
base usage_error; okt; setcmd 'npx vitest run --nosuchflag'; rec
base environment_error_cmd_not_found; okt; setcmd 'vitestx run'; rec
base environment_error_missing_dep; okt; mk src/calc.js <<'X'
import leftPad from 'left-pad';

export function add(a, b) {
  return a + b;
}

export { leftPad };
X
rec
base runner_crash; okt; mk vitest.config.js <<'X'
throw new Error('config exploded');
X
rec
for k in segv segv_shell abort sigkill; do
  base killed_$k
  case $k in
    segv|segv_shell) body="process.kill(process.pid, 'SIGSEGV');";;
    abort) body='process.abort();';;
    sigkill) body="process.kill(process.pid, 'SIGKILL');";;
  esac
  mk test/calc.test.js <<X
import { expect, test } from 'vitest';
import { add } from '../src/calc.js';

test('adds', () => {
  expect(add(2, 2)).toBe(4);
});

test('crashes', () => {
  $body
});
X
  [ $k = segv_shell ] && setcmd 'npx vitest run; exit $?'
  rec
done
base big_run
node --input-type=commonjs - <<'X'
const fs = require('fs');
fs.mkdirSync('test', { recursive: true });
for (let f = 0; f < 5; f++) {
  let s = "import { expect, test } from 'vitest';\nimport { add } from '../src/calc.js';\n\n";
  for (let i = 0; i < 120; i++) {
    const want = (f === 2 && i === 60) ? -1 : i + 1;
    s += `test('add ${f}-${i}', () => {\n  expect(add(${i}, 1)).toBe(${want});\n});\n\n`;
  }
  fs.writeFileSync(`test/part${f}.test.js`, s);
}
X
rec
base big_run_verbose; cp -r ../big_run/test .; setcmd 'npx vitest run --reporter=verbose'; rec
