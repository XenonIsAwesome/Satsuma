/** The set of non-modifier keys a hotkey binding may include one of,
 * mirroring `satsuma_core::keys`' own table (kept as plain `code` strings
 * here — the frontend only ever needs to validate/label them, never map to
 * a native virtual-key or X11 keycode, so it doesn't need the rest of that
 * table). Keyed by `KeyboardEvent.code`, a physical-position identifier
 * independent of keyboard layout. */
const KEY_LABELS: Record<string, string> = {
  KeyA: "A",
  KeyB: "B",
  KeyC: "C",
  KeyD: "D",
  KeyE: "E",
  KeyF: "F",
  KeyG: "G",
  KeyH: "H",
  KeyI: "I",
  KeyJ: "J",
  KeyK: "K",
  KeyL: "L",
  KeyM: "M",
  KeyN: "N",
  KeyO: "O",
  KeyP: "P",
  KeyQ: "Q",
  KeyR: "R",
  KeyS: "S",
  KeyT: "T",
  KeyU: "U",
  KeyV: "V",
  KeyW: "W",
  KeyX: "X",
  KeyY: "Y",
  KeyZ: "Z",
  Digit0: "0",
  Digit1: "1",
  Digit2: "2",
  Digit3: "3",
  Digit4: "4",
  Digit5: "5",
  Digit6: "6",
  Digit7: "7",
  Digit8: "8",
  Digit9: "9",
  F1: "F1",
  F2: "F2",
  F3: "F3",
  F4: "F4",
  F5: "F5",
  F6: "F6",
  F7: "F7",
  F8: "F8",
  F9: "F9",
  F10: "F10",
  F11: "F11",
  F12: "F12",
  Space: "Space",
  Tab: "Tab",
  Enter: "Enter",
  Escape: "Esc",
  Backspace: "Backspace",
  Delete: "Delete",
  Insert: "Insert",
  Home: "Home",
  End: "End",
  PageUp: "Page Up",
  PageDown: "Page Down",
  ArrowUp: "↑",
  ArrowDown: "↓",
  ArrowLeft: "←",
  ArrowRight: "→",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Semicolon: ";",
  Quote: "'",
  Backslash: "\\",
  Comma: ",",
  Period: ".",
  Slash: "/",
  Backquote: "`",
};

export function isSupportedKey(code: string): boolean {
  return code in KEY_LABELS;
}

/** A short, readable label for `code` (e.g. `"KeyP"` → `"P"`,
 * `"ArrowUp"` → `"↑"`) — falls back to the raw code for anything
 * unrecognized, which shouldn't normally happen since only recognized
 * codes are ever recorded in the first place. */
export function keyLabel(code: string): string {
  return KEY_LABELS[code] ?? code;
}
