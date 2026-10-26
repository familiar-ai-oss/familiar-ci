import { expect, test } from 'vitest';
import { add } from '../src/calc.js';
import { helper } from './nosuchhelper.js';

test('adds', () => {
  expect(add(2, 2)).toBe(4);
});
