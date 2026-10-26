const { add } = require('../src/calc');

test('crashes', () => {
  process.kill(process.pid, 'SIGSEGV');
});
