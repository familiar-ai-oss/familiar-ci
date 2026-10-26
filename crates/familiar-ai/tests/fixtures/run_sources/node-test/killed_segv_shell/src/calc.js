function add(a, b) {
  return a + b;
}

function parse(s) {
  if (!/^\d+$/.test(s)) {
    throw new Error(`not a number: ${s}`);
  }
  return Number(s);
}

module.exports = { add, parse };
