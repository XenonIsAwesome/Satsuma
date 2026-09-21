/** Pure helpers around `HotkeyCombo`, shared by the Settings screen's
 * hotkey recorder and (mirroring the Rust-side `validate_hotkeys` in
 * `satsuma_core::settings`, which remains the authoritative check) for
 * instant client-side feedback before a save round-trips to the backend. */

import { keyLabel } from "./keys";
import type { HotkeyCombo } from "../types";

export const DEFAULT_FORMAT_MODIFIERS: HotkeyCombo = {
  ctrl: false,
  alt: false,
  shift: true,
  meta: false,
  key: null,
};
export const DEFAULT_TOOLS_MODIFIERS: HotkeyCombo = {
  ctrl: false,
  alt: true,
  shift: true,
  meta: false,
  key: null,
};

export function isComboEmpty(combo: HotkeyCombo): boolean {
  return !combo.ctrl && !combo.alt && !combo.shift && !combo.meta && !combo.key;
}

export function comboEquals(a: HotkeyCombo, b: HotkeyCombo): boolean {
  return a.ctrl === b.ctrl && a.alt === b.alt && a.shift === b.shift && a.meta === b.meta && a.key === b.key;
}

/** True if every key `required` asks for is also held in `held` — a subset
 * check, unlike `comboEquals`. Fields `required` doesn't ask for (a
 * `false` modifier, a `null` extra key) are ignored regardless of `held`'s
 * state for them. */
export function comboContains(held: HotkeyCombo, required: HotkeyCombo): boolean {
  return (
    (!required.ctrl || held.ctrl) &&
    (!required.alt || held.alt) &&
    (!required.shift || held.shift) &&
    (!required.meta || held.meta) &&
    (!required.key || held.key === required.key)
  );
}

/** The keys `tools` requires beyond `format`'s own — e.g. for the default
 * Shift / Shift+Alt bindings, just Alt. Used only for live-toggling an
 * already-open overlay (see `OverlayApp`): whichever keys the *original*
 * trigger required may already have been held before the overlay's window
 * had OS focus, and so never produce an observable `keydown` there — Phase
 * 0 sidestepped this by tracking only Alt, never Shift, for exactly that
 * reason. This generalizes that to "whatever tools needs on top of format",
 * which assumes tools is format plus something extra (true of every
 * built-in default and every combination Settings' collision check
 * naturally steers users toward) rather than a wholly unrelated or
 * subset combination. */
export function extraToolsKeys(format: HotkeyCombo, tools: HotkeyCombo): HotkeyCombo {
  return {
    ctrl: tools.ctrl && !format.ctrl,
    alt: tools.alt && !format.alt,
    shift: tools.shift && !format.shift,
    meta: tools.meta && !format.meta,
    key: tools.key && tools.key !== format.key ? tools.key : null,
  };
}

/** Human-readable label for a combination, e.g. "Shift+Alt" or "Ctrl+P" —
 * Shift/Ctrl/Alt/Win lead in that fixed order (matching Tangerine's own
 * Shift/Shift+Option convention this app's defaults mirror), with any
 * extra key (see `lib/keys.ts`) trailing last. */
export function comboLabel(combo: HotkeyCombo): string {
  const parts: string[] = [];
  if (combo.shift) parts.push("Shift");
  if (combo.ctrl) parts.push("Ctrl");
  if (combo.alt) parts.push("Alt");
  if (combo.meta) parts.push("Win");
  if (combo.key) parts.push(keyLabel(combo.key));
  return parts.length > 0 ? parts.join("+") : "(none)";
}

/** Mirrors `satsuma_core::settings::validate_hotkeys`'s two hard rules, so
 * the Settings screen can reject an invalid combination immediately rather
 * than waiting on a round-trip to the backend. The backend re-validates
 * independently before persisting anything — this is purely a UX nicety,
 * not the source of truth. */
export function validateHotkeys(format: HotkeyCombo, tools: HotkeyCombo): string | null {
  if (isComboEmpty(format)) return "the format-menu combination can't be empty";
  if (isComboEmpty(tools)) return "the tools-menu combination can't be empty";
  if (comboEquals(format, tools)) return "the format-menu and tools-menu combinations can't be identical";
  return null;
}

/** Best-effort, non-exhaustive heuristic for "this is already a well-known
 * OS/DE global shortcut" — Windows/Linux expose no portable API to query
 * this for a bare modifier-only combination (unlike a full accelerator with
 * a letter key), so this is a hardcoded list of combinations known to
 * already do something system-wide, surfaced as a heads-up rather than a
 * block. Not a guarantee of collision-freedom either way. */
const RESERVED_COMBOS: Record<string, HotkeyCombo[]> = {
  windows: [{ ctrl: false, alt: false, shift: false, meta: true, key: null }],
  linux: [{ ctrl: false, alt: false, shift: false, meta: true, key: null }],
};

export function reservedComboWarning(combo: HotkeyCombo, platform: string): string | null {
  const reserved = RESERVED_COMBOS[platform];
  if (!reserved) return null;
  const isReserved = reserved.some((candidate) => comboEquals(candidate, combo));
  if (!isReserved) return null;
  return `Heads up: ${comboLabel(combo)} alone is commonly bound to an OS/DE shortcut on ${platform} — it may not reach Satsuma reliably.`;
}
