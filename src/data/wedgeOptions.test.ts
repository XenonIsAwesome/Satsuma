import { describe, expect, it } from "vitest";
import { formatWedgeOptions, getToolOptions } from "./wedgeOptions";

describe("formatWedgeOptions", () => {
  it("maps recognized extensions to display options, preserving order", () => {
    const options = formatWedgeOptions(["mov", "mkv", "webm"]);
    expect(options.map((o) => o.id)).toEqual(["mov", "mkv", "webm"]);
    expect(options.every((o) => o.label && o.icon)).toBe(true);
  });

  it("skips an extension it has no display mapping for, rather than crashing", () => {
    const options = formatWedgeOptions(["jpg", "totally-made-up"]);
    expect(options.map((o) => o.id)).toEqual(["jpg"]);
  });

  it("returns an empty list for an empty target list", () => {
    expect(formatWedgeOptions([])).toEqual([]);
  });

  it("gives WebP its special-cased display label", () => {
    const [option] = formatWedgeOptions(["webp"]);
    expect(option.label).toBe("WebP");
  });
});

describe("getToolOptions", () => {
  it("returns tool options for video including trim/split/merge", () => {
    const options = getToolOptions("video");
    expect(options.map((o) => o.id)).toEqual(
      expect.arrayContaining(["compress", "crop", "trim", "split", "merge"]),
    );
  });

  it("returns tool options for audio without crop", () => {
    const options = getToolOptions("audio");
    expect(options.map((o) => o.id)).not.toContain("crop");
    expect(options.map((o) => o.id)).toContain("trim");
  });

  it("returns Extract Archive for archive files", () => {
    const options = getToolOptions("archive");
    expect(options.map((o) => o.id)).toEqual(["extract"]);
  });

  it("returns no options for an unknown category", () => {
    expect(getToolOptions("unknown")).toEqual([]);
  });
});
