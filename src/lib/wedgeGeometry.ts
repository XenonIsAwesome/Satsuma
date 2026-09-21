interface Point {
  x: number;
  y: number;
}

/** Converts a polar coordinate (angle measured clockwise from the top) to
 * cartesian coordinates centered at (cx, cy). */
export function polarToCartesian(cx: number, cy: number, radius: number, angleDeg: number): Point {
  const angleRad = ((angleDeg - 90) * Math.PI) / 180;
  return {
    x: cx + radius * Math.cos(angleRad),
    y: cy + radius * Math.sin(angleRad),
  };
}

/** Splits a full circle into `count` equal wedges, in degrees, starting at
 * the top and going clockwise. */
export function wedgeAngles(count: number): Array<{ start: number; end: number }> {
  if (count <= 0) return [];
  const step = 360 / count;
  return Array.from({ length: count }, (_, index) => ({
    start: index * step,
    end: (index + 1) * step,
  }));
}

/** Builds an SVG path `d` attribute for a citrus-slice-shaped wedge (an
 * annulus segment) between two angles. */
export function describeWedge(
  cx: number,
  cy: number,
  outerRadius: number,
  innerRadius: number,
  startAngle: number,
  endAngle: number,
): string {
  const outerStart = polarToCartesian(cx, cy, outerRadius, endAngle);
  const outerEnd = polarToCartesian(cx, cy, outerRadius, startAngle);
  const innerStart = polarToCartesian(cx, cy, innerRadius, startAngle);
  const innerEnd = polarToCartesian(cx, cy, innerRadius, endAngle);
  const largeArcFlag = endAngle - startAngle > 180 ? 1 : 0;

  return [
    `M ${outerStart.x} ${outerStart.y}`,
    `A ${outerRadius} ${outerRadius} 0 ${largeArcFlag} 0 ${outerEnd.x} ${outerEnd.y}`,
    `L ${innerStart.x} ${innerStart.y}`,
    `A ${innerRadius} ${innerRadius} 0 ${largeArcFlag} 1 ${innerEnd.x} ${innerEnd.y}`,
    "Z",
  ].join(" ");
}
