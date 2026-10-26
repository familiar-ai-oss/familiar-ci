const test = require('node:test');
const assert = require('node:assert');
const { add } = require('../src/calc');
const helper = require('./nosuchhelper');

test('adds', () => {
  assert.strictEqual(add(2, 2), 4);
});
