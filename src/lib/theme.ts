/** Resolves a theme's five saved colors into the full set of color tokens
 * the app's CSS custom properties consume — see DESIGN.md's "Custom theme
 * behavior" section, which every theme now follows equally: a theme
 * (built-in or user-made) is just these five colors, saved as a file under
 * `~/.config/satsuma/themes/` (see `lib/tauri.ts`'s `listThemes`/
 * `saveTheme`) rather than a hardcoded preset. `typography`/`rounded`/
 * `spacing` never change across themes, so only `colors.*` tokens are
 * handled here. */

import { contrastOn, derivePrimaryActive, derivePrimaryStrong, isDarkBackground } from "./color";
import type { CustomColors } from "../types";

interface ColorTokens {
  primary: string;
  onPrimary: string;
  primaryActive: string;
  primaryStrong: string;
  secondary: string;
  neutral: string;
  surface: string;
  onSurface: string;
  outline: string;
}

/** Citrus's own literal values — Satsuma's default theme, and the ultimate
 * fallback if the themes directory can't be read or the active theme was
 * deleted out from under a saved setting. */
export const FALLBACK_THEME_COLORS: CustomColors = {
  primary: "#ff7a29",
  secondary: "#2b2620",
  neutral: "#f6f3ee",
  surface: "#ffffff",
  outline: "#eae4d9",
};

/** Resolves `colors`' five saved tokens into the full token set, deriving
 * `primary-active`/`primary-strong`/every `on-*` pairing at run time so a
 * theme (including one a user hand-wrote) can never produce unreadable
 * text no matter what it picks. */
export function resolveThemeTokens(colors: CustomColors): ColorTokens {
  const dark = isDarkBackground(colors.neutral);

  return {
    primary: colors.primary,
    onPrimary: contrastOn(colors.primary),
    primaryActive: derivePrimaryActive(colors.primary, dark),
    primaryStrong: derivePrimaryStrong(colors.primary),
    secondary: colors.secondary,
    neutral: colors.neutral,
    surface: colors.surface,
    onSurface: contrastOn(colors.surface),
    outline: colors.outline,
  };
}

const TOKEN_CSS_VARS: Record<keyof ColorTokens, string> = {
  primary: "--color-primary",
  onPrimary: "--color-on-primary",
  primaryActive: "--color-primary-active",
  primaryStrong: "--color-primary-strong",
  secondary: "--color-secondary",
  neutral: "--color-neutral",
  surface: "--color-surface",
  onSurface: "--color-on-surface",
  outline: "--color-outline",
};

/** Applies `tokens` as CSS custom properties on this document's root — each
 * window (the main app and the borderless overlay) has its own `document`,
 * so this only ever affects the window it's called from. */
export function applyThemeTokens(tokens: ColorTokens): void {
  const root = document.documentElement;
  for (const key of Object.keys(TOKEN_CSS_VARS) as (keyof ColorTokens)[]) {
    root.style.setProperty(TOKEN_CSS_VARS[key], tokens[key]);
  }
}
