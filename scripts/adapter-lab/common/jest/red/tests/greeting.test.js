const { greet } = require("../src/app");

test("greet says hello", () => {
  expect(greet()).toBe("hello");
});
