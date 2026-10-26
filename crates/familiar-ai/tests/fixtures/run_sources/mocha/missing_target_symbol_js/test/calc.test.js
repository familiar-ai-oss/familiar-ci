const assert = require('node:assert');
const { mul } = require('../src/calc');

describe('calc', () => {
  it('multiplies', () => {
    assert.strictEqual(mul(2, 3), 6);
  });
});
