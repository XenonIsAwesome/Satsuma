import { useHeldModifiers } from "./useHeldModifiers";
import { comboEquals } from "../lib/modifiers";
import type { HotkeyCombo } from "../types";

interface ModifierMatchState {
  /** Whether the currently-held keys exactly match `formatCombo`. */
  formatMatched: boolean;
  /** Whether the currently-held keys exactly match `toolsCombo`. */
  toolsMatched: boolean;
}

/** Reports whether the currently-held modifier keys (see `useHeldModifiers`)
 * exactly match either of the two user-configured combinations — matching
 * is always exact-equality, never subset/superset, so a format binding of
 * plain Shift and a tools binding of Shift+Alt can never both read as
 * matched at once. Used for the in-app drag-and-drop trigger, where the
 * whole combination is held while this window already has focus and so is
 * fully observable from a clean "nothing held" start. */
export function useModifierKeys(formatCombo: HotkeyCombo, toolsCombo: HotkeyCombo): ModifierMatchState {
  // Only ever watches the specific extra key(s) `formatCombo`/`toolsCombo`
  // actually use, not `lib/keys.ts`'s whole supported set — see
  // `useHeldModifiers`'s own doc comment for why that distinction matters.
  const watchedKeys = [formatCombo.key, toolsCombo.key].filter((key): key is string => key !== null);
  const held = useHeldModifiers(watchedKeys);

  return {
    formatMatched: comboEquals(held, formatCombo),
    toolsMatched: comboEquals(held, toolsCombo),
  };
}
