import { expect, test } from 'vitest';
import { add } from '../src/calc.js';

test('adds', () => {
  expect(add(2, 2)).toBe(4);
});

test('crashes', () => {
  process.kill(process.pid, 'SIGSEGV');
});
