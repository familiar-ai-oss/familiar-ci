const test = require('node:test');
const assert = require('node:assert');
const { mul } = require('../src/calc');

test('multiplies', () => {
  assert.strictEqual(mul(2, 3), 6);
});
