import { useEffect, useState } from "react";
import { comboEquals } from "../lib/modifiers";
import type { HotkeyCombo } from "../types";

const NONE_HELD: HotkeyCombo = { ctrl: false, alt: false, shift: false, meta: false, key: null };

function isModifierKey(key: string): boolean {
  return key === "Control" || key === "Alt" || key === "Shift" || key === "Meta";
}

/** Unlike the four modifiers, an extra key carries no held-state flag on
 * the event itself — it's tracked across its own keydown/keyup pair (see
 * `heldExtraKey` below) and folded in here alongside the flags the browser
 * does give for free. */
function snapshot(event: KeyboardEvent, heldExtraKey: string | null): HotkeyCombo {
  return {
    ctrl: event.ctrlKey,
    alt: event.altKey,
    shift: event.shiftKey,
    meta: event.metaKey,
    key: heldExtraKey,
  };
}

/** Tracks live Ctrl/Alt/Shift/Win key state via window-level listeners.
 *
 * Reads the browser's own `ctrlKey`/`altKey`/`shiftKey`/`metaKey` flags —
 * present on *every* keyboard event, not just the one for that specific
 * key — rather than reconstructing state from each key's own
 * keydown/keyup pair. That reconstruction was tried first and confirmed
 * broken in practice: pressing Shift then Alt together could end up
 * recording Alt alone, because Alt is treated specially as a menu-
 * accelerator key by some window managers/webviews and doesn't reliably
 * deliver its own clean keydown/keyup here. Reading the flags directly
 * self-corrects on whichever event *does* arrive, since they always
 * reflect the browser's own up-to-date idea of everything currently held,
 * not just what this listener personally observed transition.
 *
 * Resets to nothing-held on blur so a modifier released while the window
 * is unfocused doesn't get stuck "held" forever.
 *
 * `watchedKeys` are the only non-modifier keys this hook will ever report
 * as held — e.g. the specific extra key(s), if any, that the caller's own
 * configured combos actually use. Deliberately *not* "any key from
 * `lib/keys.ts`'s whole supported set": this hook runs at the window level
 * for as long as the app is mounted, not just for a short deliberate
 * recording gesture like `HotkeyRecorder`. Reporting every letter, digit,
 * or arrow key press — regardless of whether any configured combo cares
 * about it — meant a totally unrelated keystroke elsewhere in the app
 * (e.g. arrow-key navigation inside the wedge menu) changed this hook's
 * return value, triggering a re-render of every consumer on every
 * keystroke; that re-render, in turn, was observed to reset unrelated
 * component state further down the tree that happened to depend on a
 * freshly-recreated prop reference. Narrowing to just the keys that matter
 * keeps this hook quiet unless something it actually needs to observe
 * changes.
 */
export function useHeldModifiers(watchedKeys: readonly string[] = []): HotkeyCombo {
  const [held, setHeld] = useState<HotkeyCombo>(NONE_HELD);

  useEffect(() => {
    let heldExtraKey: string | null = null;

    function handleKeyEvent(event: KeyboardEvent) {
      if (event.type === "keydown" && !isModifierKey(event.key) && watchedKeys.includes(event.code)) {
        heldExtraKey = event.code;
      } else if (event.type === "keyup" && event.code === heldExtraKey) {
        heldExtraKey = null;
      }
      const next = snapshot(event, heldExtraKey);
      setHeld((prev) => (comboEquals(prev, next) ? prev : next));
    }
    function handleBlur() {
      heldExtraKey = null;
      setHeld(NONE_HELD);
    }

    window.addEventListener("keydown", handleKeyEvent);
    window.addEventListener("keyup", handleKeyEvent);
    window.addEventListener("blur", handleBlur);
    return () => {
      window.removeEventListener("keydown", handleKeyEvent);
      window.removeEventListener("keyup", handleKeyEvent);
      window.removeEventListener("blur", handleBlur);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [watchedKeys.join(",")]);

  return held;
}
