const { add } = require('../src/calc');

test('adds', () => {
  expect(add(2, 2)).toBe(5);
});
