import { expect, test } from 'vitest';
import { parse } from '../src/calc.js';

test('parses', () => {
  expect(parse('x1')).toBe(1);
});
