const assert = require('node:assert');
const { add } = require('../src/calc');

describe('calc', () => {
  it('adds', () => {
    assert.strictEqual(add(2, 2), 4);
  });

  it('crashes', () => {
    process.kill(process.pid, 'SIGKILL');
  });
});
