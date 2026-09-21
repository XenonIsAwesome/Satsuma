import { describe, expect, it } from "vitest";
import {
  contrastOn,
  contrastRatio,
  derivePrimaryActive,
  derivePrimaryStrong,
  deriveOutline,
  hexToRgb,
  hslToRgb,
  isDarkBackground,
  relativeLuminance,
  rgbToHex,
  rgbToHsl,
} from "./color";

describe("hexToRgb / rgbToHex", () => {
  it("round-trips a 6-digit hex color", () => {
    expect(hexToRgb("#ff7a29")).toEqual({ r: 255, g: 122, b: 41 });
    expect(rgbToHex({ r: 255, g: 122, b: 41 })).toBe("#ff7a29");
  });

  it("expands a 3-digit hex shorthand", () => {
    expect(hexToRgb("#fff")).toEqual({ r: 255, g: 255, b: 255 });
  });
});

describe("rgbToHsl / hslToRgb", () => {
  it("round-trips within rounding tolerance", () => {
    const rgb = hexToRgb("#ff7a29");
    const hsl = rgbToHsl(rgb);
    const roundTripped = hslToRgb(hsl);
    expect(rgbToHex(roundTripped)).toBe(rgbToHex(rgb));
  });

  it("treats pure white as zero saturation", () => {
    const hsl = rgbToHsl({ r: 255, g: 255, b: 255 });
    expect(hsl.s).toBe(0);
    expect(hsl.l).toBe(100);
  });
});

describe("relativeLuminance / contrastRatio", () => {
  it("gives black and white the maximum 21:1 contrast", () => {
    expect(contrastRatio("#000000", "#ffffff")).toBeCloseTo(21, 0);
  });

  it("gives a color no contrast against itself", () => {
    expect(contrastRatio("#ff7a29", "#ff7a29")).toBeCloseTo(1, 5);
  });

  it("orders luminance from black to white", () => {
    expect(relativeLuminance("#000000")).toBeLessThan(relativeLuminance("#ffffff"));
  });
});

describe("contrastOn", () => {
  it("picks white text on a near-black background", () => {
    expect(contrastOn("#111111")).toBe("#ffffff");
  });

  it("picks black text on a near-white background", () => {
    expect(contrastOn("#f6f3ee")).toBe("#000000");
  });
});

describe("isDarkBackground", () => {
  it("classifies Citrus's light neutral as not dark", () => {
    expect(isDarkBackground("#f6f3ee")).toBe(false);
  });

  it("classifies Midnight Citrus's dark neutral as dark", () => {
    expect(isDarkBackground("#1b1815")).toBe(true);
  });
});

describe("derivePrimaryActive", () => {
  it("darkens on a light theme", () => {
    const active = derivePrimaryActive("#ff7a29", false);
    const { l: primaryLightness } = rgbToHsl(hexToRgb("#ff7a29"));
    const { l: activeLightness } = rgbToHsl(hexToRgb(active));
    expect(activeLightness).toBeLessThan(primaryLightness);
  });

  it("lightens on a dark theme", () => {
    const active = derivePrimaryActive("#ff9452", true);
    const { l: primaryLightness } = rgbToHsl(hexToRgb("#ff9452"));
    const { l: activeLightness } = rgbToHsl(hexToRgb(active));
    expect(activeLightness).toBeGreaterThan(primaryLightness);
  });
});

describe("derivePrimaryStrong", () => {
  it("reaches WCAG AA contrast (4.5:1) against white", () => {
    const strong = derivePrimaryStrong("#ff7a29");
    expect(contrastRatio(strong, "#ffffff")).toBeGreaterThanOrEqual(4.5);
  });

  it("is a no-op when the input already meets AA contrast", () => {
    const strong = derivePrimaryStrong("#2b2620");
    expect(contrastRatio(strong, "#ffffff")).toBeGreaterThanOrEqual(4.5);
  });
});

describe("deriveOutline", () => {
  it("produces a tone distinguishable from the source neutral", () => {
    const outline = deriveOutline("#f6f3ee");
    expect(outline).not.toBe("#f6f3ee");
  });
});
