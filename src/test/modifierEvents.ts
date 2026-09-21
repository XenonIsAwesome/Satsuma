import type { HotkeyCombo } from "../types";

const FIELD_FOR: Record<string, "ctrl" | "alt" | "shift" | "meta"> = {
  Control: "ctrl",
  Alt: "alt",
  Shift: "shift",
  Meta: "meta",
};

/**
 * Dispatches `window` keydown/keyup events with realistic, cumulative
 * `ctrlKey`/`altKey`/`shiftKey`/`metaKey` flags — real browsers set these
 * on *every* keyboard event to reflect everything currently held, not just
 * the one key that event is for, which `useHeldModifiers` and
 * `HotkeyRecorder` both rely on directly (see their doc comments for why:
 * reconstructing state from each key's own down/up pair alone was tried
 * first and confirmed broken for a real Shift+Alt press). A plain
 * `new KeyboardEvent("keydown", { key })` leaves those flags at their
 * default `false`, which would silently fail to exercise that logic.
 */
export function createModifierKeyDispatcher() {
  const held: HotkeyCombo = { ctrl: false, alt: false, shift: false, meta: false, key: null };

  function eventInit(key: string) {
    return { key, ctrlKey: held.ctrl, altKey: held.alt, shiftKey: held.shift, metaKey: held.meta };
  }

  return {
    down(key: string) {
      const field = FIELD_FOR[key];
      if (field) held[field] = true;
      window.dispatchEvent(new KeyboardEvent("keydown", eventInit(key)));
    },
    up(key: string) {
      const field = FIELD_FOR[key];
      if (field) held[field] = false;
      window.dispatchEvent(new KeyboardEvent("keyup", eventInit(key)));
    },
  };
}
