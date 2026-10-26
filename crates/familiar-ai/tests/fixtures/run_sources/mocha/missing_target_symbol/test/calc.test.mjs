import assert from 'node:assert';
import { mul } from '../src/calc.mjs';

describe('calc', () => {
  it('multiplies', () => {
    assert.strictEqual(mul(2, 3), 6);
  });
});
