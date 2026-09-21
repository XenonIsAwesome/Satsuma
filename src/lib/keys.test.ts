import { describe, expect, it } from "vitest";
import { isSupportedKey, keyLabel } from "./keys";

describe("isSupportedKey", () => {
  it("recognizes letters, digits, and function keys", () => {
    expect(isSupportedKey("KeyP")).toBe(true);
    expect(isSupportedKey("Digit3")).toBe(true);
    expect(isSupportedKey("F5")).toBe(true);
  });

  it("rejects an unrecognized code", () => {
    expect(isSupportedKey("NumpadEnter")).toBe(false);
    expect(isSupportedKey("")).toBe(false);
  });
});

describe("keyLabel", () => {
  it("labels a letter key with just the letter", () => {
    expect(keyLabel("KeyP")).toBe("P");
  });

  it("labels a digit key with just the digit", () => {
    expect(keyLabel("Digit3")).toBe("3");
  });

  it("labels an arrow key with an arrow glyph", () => {
    expect(keyLabel("ArrowUp")).toBe("↑");
  });

  it("falls back to the raw code for something unrecognized", () => {
    expect(keyLabel("NumpadEnter")).toBe("NumpadEnter");
  });
});
