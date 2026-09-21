import { describe, expect, it } from "vitest";
import { contrastRatio } from "./color";
import { FALLBACK_THEME_COLORS, resolveThemeTokens } from "./theme";
import type { CustomColors } from "../types";

describe("resolveThemeTokens", () => {
  it("resolves Citrus's own literal token values", () => {
    const tokens = resolveThemeTokens(FALLBACK_THEME_COLORS);
    expect(tokens.primary).toBe("#ff7a29");
    expect(tokens.secondary).toBe("#2b2620");
    expect(tokens.neutral).toBe("#f6f3ee");
    expect(tokens.surface).toBe("#ffffff");
    expect(tokens.outline).toBe("#eae4d9");
  });

  it("gives Citrus AA-contrasting on-primary/on-surface text", () => {
    const tokens = resolveThemeTokens(FALLBACK_THEME_COLORS);
    expect(contrastRatio(tokens.surface, tokens.onSurface)).toBeGreaterThanOrEqual(4.5);
  });

  it("derives a lightened primary-active for a dark theme", () => {
    const midnightCitrus: CustomColors = {
      primary: "#ff9452",
      secondary: "#f5efe6",
      neutral: "#1b1815",
      surface: "#262220",
      outline: "#3a342e",
    };
    const tokens = resolveThemeTokens(midnightCitrus);
    expect(tokens.neutral).toBe("#1b1815");
    expect(tokens.primaryActive).not.toBe(tokens.primary);
  });

  it("uses the given colors directly for a user-made theme", () => {
    const custom: CustomColors = {
      primary: "#3366cc",
      secondary: "#111111",
      neutral: "#f0f0f0",
      surface: "#ffffff",
      outline: "#dddddd",
    };
    const tokens = resolveThemeTokens(custom);
    expect(tokens.primary).toBe(custom.primary);
    expect(tokens.secondary).toBe(custom.secondary);
    expect(tokens.neutral).toBe(custom.neutral);
    expect(tokens.surface).toBe(custom.surface);
    expect(tokens.outline).toBe(custom.outline);
  });

  it("never crashes on a theme with unreadable-looking picks", () => {
    const allWhite: CustomColors = {
      primary: "#ffffff",
      secondary: "#ffffff",
      neutral: "#ffffff",
      surface: "#ffffff",
      outline: "#ffffff",
    };
    const tokens = resolveThemeTokens(allWhite);
    expect(contrastRatio(tokens.surface, tokens.onSurface)).toBeGreaterThanOrEqual(4.5);
  });
});
