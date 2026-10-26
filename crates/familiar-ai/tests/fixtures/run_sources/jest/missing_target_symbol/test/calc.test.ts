import { add, mul } from '../src/calc';

test('multiplies', () => {
  expect(mul(2, 3)).toBe(6);
  expect(add(2, 3)).toBe(5);
});
