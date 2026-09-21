import { describe, expect, it } from "vitest";
import { describeWedge, polarToCartesian, wedgeAngles } from "./wedgeGeometry";

describe("wedgeAngles", () => {
  it("splits a circle into equal, contiguous slices starting at 0", () => {
    expect(wedgeAngles(4)).toEqual([
      { start: 0, end: 90 },
      { start: 90, end: 180 },
      { start: 180, end: 270 },
      { start: 270, end: 360 },
    ]);
  });

  it("handles a single option as the full circle", () => {
    expect(wedgeAngles(1)).toEqual([{ start: 0, end: 360 }]);
  });

  it("returns an empty array for zero options", () => {
    expect(wedgeAngles(0)).toEqual([]);
  });
});

describe("polarToCartesian", () => {
  it("places angle 0 (top) directly above the center", () => {
    const point = polarToCartesian(100, 100, 50, 0);
    expect(point.x).toBeCloseTo(100);
    expect(point.y).toBeCloseTo(50);
  });

  it("places angle 90 (right) directly right of the center", () => {
    const point = polarToCartesian(100, 100, 50, 90);
    expect(point.x).toBeCloseTo(150);
    expect(point.y).toBeCloseTo(100);
  });

  it("places angle 180 (bottom) directly below the center", () => {
    const point = polarToCartesian(100, 100, 50, 180);
    expect(point.x).toBeCloseTo(100);
    expect(point.y).toBeCloseTo(150);
  });
});

describe("describeWedge", () => {
  it("produces a non-empty SVG path string with the expected commands", () => {
    const path = describeWedge(160, 160, 150, 40, 0, 90);
    expect(path).toMatch(/^M /);
    expect(path).toContain("A 150 150");
    expect(path).toContain("A 40 40");
    expect(path.trim().endsWith("Z")).toBe(true);
  });

  it("sets the large-arc-flag when the wedge spans more than 180 degrees", () => {
    const wideWedge = describeWedge(160, 160, 150, 40, 0, 270);
    const narrowWedge = describeWedge(160, 160, 150, 40, 0, 90);
    expect(wideWedge).toMatch(/A 150 150 0 1 0/);
    expect(narrowWedge).toMatch(/A 150 150 0 0 0/);
  });
});
