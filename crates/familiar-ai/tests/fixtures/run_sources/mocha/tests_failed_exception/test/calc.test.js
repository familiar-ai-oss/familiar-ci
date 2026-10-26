const assert = require('node:assert');
const { parse } = require('../src/calc');

describe('calc', () => {
  it('parses', () => {
    assert.strictEqual(parse('x1'), 1);
  });
});
