const { parse } = require('../src/calc');

test('parses', () => {
  expect(parse('x1')).toBe(1);
});
