import { describe, expect, it } from "vitest";
import {
  comboContains,
  comboEquals,
  comboLabel,
  DEFAULT_FORMAT_MODIFIERS,
  DEFAULT_TOOLS_MODIFIERS,
  extraToolsKeys,
  isComboEmpty,
  reservedComboWarning,
  validateHotkeys,
} from "./modifiers";
import type { HotkeyCombo } from "../types";

const EMPTY: HotkeyCombo = { ctrl: false, alt: false, shift: false, meta: false, key: null };

describe("isComboEmpty", () => {
  it("is true only when no modifier is held", () => {
    expect(isComboEmpty(EMPTY)).toBe(true);
    expect(isComboEmpty(DEFAULT_FORMAT_MODIFIERS)).toBe(false);
  });
});

describe("comboEquals", () => {
  it("compares field-by-field", () => {
    expect(comboEquals(DEFAULT_FORMAT_MODIFIERS, { ...DEFAULT_FORMAT_MODIFIERS })).toBe(true);
    expect(comboEquals(DEFAULT_FORMAT_MODIFIERS, DEFAULT_TOOLS_MODIFIERS)).toBe(false);
  });
});

describe("comboLabel", () => {
  it("joins held modifiers in a fixed order", () => {
    expect(comboLabel(DEFAULT_FORMAT_MODIFIERS)).toBe("Shift");
    expect(comboLabel(DEFAULT_TOOLS_MODIFIERS)).toBe("Shift+Alt");
    expect(comboLabel({ ctrl: true, alt: true, shift: true, meta: true, key: null })).toBe("Shift+Ctrl+Alt+Win");
  });

  it("labels an empty combo distinctly", () => {
    expect(comboLabel(EMPTY)).toBe("(none)");
  });
});

describe("validateHotkeys", () => {
  it("accepts the defaults", () => {
    expect(validateHotkeys(DEFAULT_FORMAT_MODIFIERS, DEFAULT_TOOLS_MODIFIERS)).toBeNull();
  });

  it("rejects an empty format combo", () => {
    expect(validateHotkeys(EMPTY, DEFAULT_TOOLS_MODIFIERS)).toMatch(/format/);
  });

  it("rejects an empty tools combo", () => {
    expect(validateHotkeys(DEFAULT_FORMAT_MODIFIERS, EMPTY)).toMatch(/tools/);
  });

  it("rejects identical bindings", () => {
    expect(validateHotkeys(DEFAULT_FORMAT_MODIFIERS, DEFAULT_FORMAT_MODIFIERS)).toMatch(/identical/);
  });

  it("allows a binding that is a superset of the other", () => {
    expect(validateHotkeys(DEFAULT_FORMAT_MODIFIERS, DEFAULT_TOOLS_MODIFIERS)).toBeNull();
  });
});

describe("comboContains", () => {
  it("is true when every required key is held, ignoring extras", () => {
    const held: HotkeyCombo = { ctrl: true, alt: true, shift: true, meta: false, key: null };
    const required: HotkeyCombo = { ctrl: false, alt: true, shift: false, meta: false, key: null };
    expect(comboContains(held, required)).toBe(true);
  });

  it("is false when a required key is missing", () => {
    const held: HotkeyCombo = { ctrl: false, alt: false, shift: true, meta: false, key: null };
    const required: HotkeyCombo = { ctrl: false, alt: true, shift: false, meta: false, key: null };
    expect(comboContains(held, required)).toBe(false);
  });

  it("is true for an empty requirement regardless of held state", () => {
    expect(comboContains(EMPTY, EMPTY)).toBe(true);
  });
});

describe("extraToolsKeys", () => {
  it("returns just Alt for the default Shift / Shift+Alt bindings", () => {
    expect(extraToolsKeys(DEFAULT_FORMAT_MODIFIERS, DEFAULT_TOOLS_MODIFIERS)).toEqual({
      ctrl: false,
      alt: true,
      shift: false,
      meta: false,
      key: null,
    });
  });

  it("returns every tools key when format and tools share nothing", () => {
    const format: HotkeyCombo = { ctrl: true, alt: false, shift: false, meta: false, key: null };
    const tools: HotkeyCombo = { ctrl: false, alt: false, shift: true, meta: true, key: null };
    expect(extraToolsKeys(format, tools)).toEqual(tools);
  });
});

describe("reservedComboWarning", () => {
  it("warns about a bare Win key on windows", () => {
    const winOnly: HotkeyCombo = { ctrl: false, alt: false, shift: false, meta: true, key: null };
    expect(reservedComboWarning(winOnly, "windows")).toMatch(/Win/);
  });

  it("does not warn about the default Shift binding", () => {
    expect(reservedComboWarning(DEFAULT_FORMAT_MODIFIERS, "windows")).toBeNull();
  });

  it("does not warn on an unrecognized platform", () => {
    const winOnly: HotkeyCombo = { ctrl: false, alt: false, shift: false, meta: true, key: null };
    expect(reservedComboWarning(winOnly, "macos")).toBeNull();
  });
});
