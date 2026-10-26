const { mul } = require('../src/calc');

test('multiplies', () => {
  expect(mul(2, 3)).toBe(6);
});
