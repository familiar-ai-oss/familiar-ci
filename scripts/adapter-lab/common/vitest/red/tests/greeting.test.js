import { expect, test } from "vitest";
import { greet } from "../src/app.js";

test("greet says hello", () => {
  expect(greet()).toBe("hello");
});
