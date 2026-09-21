/** Small, dependency-free color-math helpers backing the Custom theme's
 * derived tokens (see DESIGN.md's "Custom theme behavior"): computing
 * `primary-active`/`primary-strong` from a user-picked `primary`, and
 * picking black/white for every `on-*` pairing via real WCAG contrast, so a
 * Custom theme can never produce unreadable text no matter what the user
 * picks. */

interface Rgb {
  r: number;
  g: number;
  b: number;
}

interface Hsl {
  h: number;
  s: number;
  l: number;
}

export function hexToRgb(hex: string): Rgb {
  const normalized = hex.replace("#", "");
  const value = normalized.length === 3 ? normalized.replace(/./g, (c) => c + c) : normalized;
  const int = parseInt(value, 16);
  return { r: (int >> 16) & 255, g: (int >> 8) & 255, b: int & 255 };
}

function toHexByte(value: number): string {
  return Math.round(clamp(value, 0, 255)).toString(16).padStart(2, "0");
}

export function rgbToHex({ r, g, b }: Rgb): string {
  return `#${toHexByte(r)}${toHexByte(g)}${toHexByte(b)}`;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

export function rgbToHsl({ r, g, b }: Rgb): Hsl {
  const rn = r / 255;
  const gn = g / 255;
  const bn = b / 255;
  const max = Math.max(rn, gn, bn);
  const min = Math.min(rn, gn, bn);
  const l = (max + min) / 2;

  if (max === min) {
    return { h: 0, s: 0, l: l * 100 };
  }

  const delta = max - min;
  const s = l > 0.5 ? delta / (2 - max - min) : delta / (max + min);
  let h: number;
  switch (max) {
    case rn:
      h = ((gn - bn) / delta + (gn < bn ? 6 : 0)) * 60;
      break;
    case gn:
      h = ((bn - rn) / delta + 2) * 60;
      break;
    default:
      h = ((rn - gn) / delta + 4) * 60;
      break;
  }
  return { h, s: s * 100, l: l * 100 };
}

export function hslToRgb({ h, s, l }: Hsl): Rgb {
  const sn = clamp(s, 0, 100) / 100;
  const ln = clamp(l, 0, 100) / 100;

  if (sn === 0) {
    const gray = ln * 255;
    return { r: gray, g: gray, b: gray };
  }

  const q = ln < 0.5 ? ln * (1 + sn) : ln + sn - ln * sn;
  const p = 2 * ln - q;
  const hn = ((h % 360) + 360) % 360 / 360;

  function hueToChannel(t: number): number {
    let tt = t;
    if (tt < 0) tt += 1;
    if (tt > 1) tt -= 1;
    if (tt < 1 / 6) return p + (q - p) * 6 * tt;
    if (tt < 1 / 2) return q;
    if (tt < 2 / 3) return p + (q - p) * (2 / 3 - tt) * 6;
    return p;
  }

  return {
    r: hueToChannel(hn + 1 / 3) * 255,
    g: hueToChannel(hn) * 255,
    b: hueToChannel(hn - 1 / 3) * 255,
  };
}

/** WCAG relative luminance (0-1) of a `#rrggbb` color. */
export function relativeLuminance(hex: string): number {
  const { r, g, b } = hexToRgb(hex);
  const channel = (value: number) => {
    const c = value / 255;
    return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
  };
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

/** WCAG contrast ratio (1-21) between two `#rrggbb` colors. */
export function contrastRatio(hexA: string, hexB: string): number {
  const a = relativeLuminance(hexA) + 0.05;
  const b = relativeLuminance(hexB) + 0.05;
  return a > b ? a / b : b / a;
}

/** Pure black or pure white, whichever gives the higher WCAG contrast
 * against `hex` — used for every `on-*` token pairing on a Custom theme. */
export function contrastOn(hex: string): string {
  return contrastRatio(hex, "#000000") >= contrastRatio(hex, "#ffffff") ? "#000000" : "#ffffff";
}

export function isDarkBackground(hex: string): boolean {
  return relativeLuminance(hex) < 0.5;
}

function adjust(hex: string, deltaLightness: number, deltaSaturation: number): string {
  const hsl = rgbToHsl(hexToRgb(hex));
  return rgbToHex(
    hslToRgb({
      h: hsl.h,
      s: clamp(hsl.s + deltaSaturation, 0, 100),
      l: clamp(hsl.l + deltaLightness, 0, 100),
    }),
  );
}

/** The Custom theme's `primary-active`: `primary` pushed ~10% more
 * saturated and, on a light background, ~10 points darker (or lighter, on
 * a dark-background custom theme) — the same relationship Citrus's
 * `#FF7A29` → `#E85F0F` demonstrates. */
export function derivePrimaryActive(primaryHex: string, isDark: boolean): string {
  return adjust(primaryHex, isDark ? 10 : -10, 10);
}

/** The Custom theme's `primary-strong`: `primary` darkened just enough to
 * hold WCAG AA contrast (4.5:1) against white text, for the same role
 * Citrus's `#C24A0A` plays against `#FF7A29`. Falls back to the darkest
 * attempt if 4.5:1 is unreachable without crushing the color to black. */
export function derivePrimaryStrong(primaryHex: string): string {
  let current = primaryHex;
  for (let step = 0; step < 40; step += 1) {
    if (contrastRatio(current, "#ffffff") >= 4.5) return current;
    current = adjust(current, -2, 0);
  }
  return current;
}

/** A subtle, barely-there border tone derived from `neutral` — offset
 * further from `neutral` on a dark theme, since a light offset there would
 * move the wrong direction toward washing out. */
export function deriveOutline(neutralHex: string): string {
  return adjust(neutralHex, isDarkBackground(neutralHex) ? 8 : -6, 0);
}
