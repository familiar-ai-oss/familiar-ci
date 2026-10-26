const { add } = require('../src/calc');
const helper = require('./nosuchhelper');

test('adds', () => {
  expect(add(2, 2)).toBe(4);
});
