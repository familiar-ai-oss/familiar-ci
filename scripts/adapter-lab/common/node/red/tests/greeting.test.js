const test = require("node:test");
const assert = require("node:assert");
const { greet } = require("../src/app");

test("greet says hello", () => {
  assert.strictEqual(greet(), "hello");
});
