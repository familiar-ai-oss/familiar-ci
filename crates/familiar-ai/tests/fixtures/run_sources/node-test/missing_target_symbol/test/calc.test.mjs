import test from 'node:test';
import assert from 'node:assert';
import { mul } from '../src/calc.mjs';

test('multiplies', () => {
  assert.strictEqual(mul(2, 3), 6);
});
