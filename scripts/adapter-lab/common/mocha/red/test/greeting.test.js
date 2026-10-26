const assert = require("node:assert");
const { greet } = require("../src/app");

describe("greet", () => {
  it("says hello", () => {
    assert.strictEqual(greet(), "hello");
  });
});
