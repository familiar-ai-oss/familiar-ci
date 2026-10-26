const test = require('node:test');
const assert = require('node:assert');
const { parse } = require('../src/calc');

test('parses', () => {
  assert.strictEqual(parse('x1'), 1);
});
