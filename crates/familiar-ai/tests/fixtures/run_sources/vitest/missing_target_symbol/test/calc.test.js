import { expect, test } from 'vitest';
import { mul } from '../src/calc.js';

test('multiplies', () => {
  expect(mul(2, 3)).toBe(6);
});
