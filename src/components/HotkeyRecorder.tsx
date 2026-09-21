import { useEffect, useRef, useState } from "react";
import type { HotkeyCombo } from "../types";
import { isSupportedKey } from "../lib/keys";
import { comboLabel, isComboEmpty } from "../lib/modifiers";
import { pollHeldModifiers } from "../lib/tauri";
import "./HotkeyRecorder.css";

interface HotkeyRecorderProps {
  label: string;
  combo: HotkeyCombo;
  onChange: (next: HotkeyCombo) => void;
  testId: string;
}

const NONE_HELD: HotkeyCombo = { ctrl: false, alt: false, shift: false, meta: false, key: null };

/** How often to re-query the OS for the currently-held modifiers while
 * recording. Tight enough that a deliberate key hold is never missed
 * between ticks, loose enough not to matter as IPC overhead for a UI
 * interaction that lasts at most a few seconds. */
const POLL_INTERVAL_MS = 50;

/** Safety net: force-finalizes (with whatever's been captured so far, or
 * just cancels if nothing has) if recording is somehow still going after
 * this long. Neither observation source is fully trustworthy on its own
 * (see the component doc comment) — this guards against the specific
 * failure mode where one of them gets stuck mid-hold reporting a key as
 * still down when it's genuinely been released, which would otherwise
 * leave the recorder waiting forever for a "nothing held" reading that
 * never comes. */
const WATCHDOG_MS = 8000;

/** Whether the key itself is one of the four we care about — used only to
 * decide whether to `preventDefault()` (e.g. so Alt doesn't trigger a
 * browser/webview menu-accelerator side effect), not to decide what to
 * record: see `snapshotFromEvent` below for that. */
function isModifierKey(key: string): boolean {
  return key === "Control" || key === "Alt" || key === "Shift" || key === "Meta";
}

function isAnyHeld(combo: HotkeyCombo): boolean {
  return combo.ctrl || combo.alt || combo.shift || combo.meta || combo.key !== null;
}

/** Extra keys (unlike the four modifiers) carry no held-state flag on the
 * `KeyboardEvent` itself — only `ctrlKey`/`altKey`/`shiftKey`/`metaKey` are
 * given for free. So the currently-held extra key, if any, is tracked
 * ourselves across each key's own keydown/keyup pair (see `heldExtraKey` in
 * the effect below) and folded in here alongside the four modifier flags
 * the browser does provide directly. */
function snapshotFromEvent(event: KeyboardEvent, heldExtraKey: string | null): HotkeyCombo {
  return {
    ctrl: event.ctrlKey,
    alt: event.altKey,
    shift: event.shiftKey,
    meta: event.metaKey,
    key: heldExtraKey,
  };
}

function union(a: HotkeyCombo, b: HotkeyCombo): HotkeyCombo {
  return {
    ctrl: a.ctrl || b.ctrl,
    alt: a.alt || b.alt,
    shift: a.shift || b.shift,
    meta: a.meta || b.meta,
    key: a.key ?? b.key,
  };
}

/**
 * A traditional "click to record, then press the keys you want" hotkey
 * picker: clicking the button starts recording, every modifier pressed
 * during that gesture is captured (the *union* of everything held at any
 * point, not just whatever's left at the end — so pressing Shift, then
 * also Alt, then releasing Alt first still records Shift+Alt), and it
 * finalizes the instant every key is released again. Escape, losing window
 * focus, or clicking the button again while recording cancels without
 * changing anything.
 *
 * Runs *two* independent ways of observing what's held, at the same time,
 * and merges whatever either one reports — rather than preferring one over
 * the other:
 *
 * 1. DOM keyboard events (`event.ctrlKey`/`altKey`/`shiftKey`/`metaKey`).
 *    Shipped first, but confirmed unreliable on its own: Alt is a
 *    menu-accelerator/mnemonic key that some native window
 *    toolkits/webviews intercept before its own keydown/keyup ever reaches
 *    the page's JS at all.
 * 2. Polling the OS directly (`pollHeldModifiers` — `GetAsyncKeyState` on
 *    Windows, `XQueryPointer`'s modifier mask on Linux/X11). Added to fix
 *    (1), but *also* confirmed unreliable on its own in practice: holding
 *    Shift+Alt together could still under-report Alt depending on the
 *    desktop environment's modifier mapping/keyboard driver quirks — the
 *    reverse failure mode from (1), on the same combination.
 *
 * Neither mechanism alone has turned out to be trustworthy for every
 * environment this app runs in, but their failure modes don't appear to
 * overlap — so running both and taking the union of whatever each
 * independently observes is more robust than picking one. `pollHeldModifiers`
 * resolving to `null` (no native query available at all, e.g. a Linux
 * Wayland session) just means the DOM side is carrying the full load, as
 * it did before native polling existed.
 *
 * Deciding when to *finalize* (everything released) judges each fresh
 * observation only against itself, never against the *other* source's
 * last known value — deliberately. An earlier version required both
 * sources to agree nothing was held, which hung forever if either one got
 * stuck reporting a key as still down after it was actually released. A
 * later attempt let either source's own last-known "nothing held" trigger
 * a release — which then finalized too early: one source's most recent
 * reading is often *stale* (e.g. polling's first-ever sample, taken before
 * any key was pressed, doesn't get refreshed until its next tick 50ms
 * later), and a stale "nothing held" looks identical to a genuine one.
 * Only trusting a reading at the exact moment it freshly arrives avoids
 * both failures: a source that stops updating altogether simply stops
 * contributing further release checks (instead of blocking, or worse
 * wrongly triggering off, the other source's own fresh checks).
 * `WATCHDOG_MS` is a last-resort backstop in case some future environment
 * manages to wedge both sources at once.
 */
export function HotkeyRecorder({ label, combo, onChange, testId }: HotkeyRecorderProps) {
  const [recording, setRecording] = useState(false);
  const [captured, setCaptured] = useState<HotkeyCombo>(NONE_HELD);
  const capturedRef = useRef<HotkeyCombo>(NONE_HELD);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  useEffect(() => {
    if (!recording) return;

    let cancelled = false;
    let intervalId: ReturnType<typeof setInterval> | undefined;
    let heldExtraKey: string | null = null;

    capturedRef.current = NONE_HELD;
    setCaptured(NONE_HELD);

    function finalize() {
      setRecording(false);
      if (!isComboEmpty(capturedRef.current)) onChangeRef.current(capturedRef.current);
    }

    /** Folds a fresh reading from either observation source into the
     * accumulated capture, and finalizes if *this specific reading* shows
     * nothing held — never by cross-checking the other source's last
     * known value (see the component doc comment for why). */
    function observe(next: HotkeyCombo) {
      capturedRef.current = union(capturedRef.current, next);
      setCaptured(capturedRef.current);
      if (!isComboEmpty(capturedRef.current) && !isAnyHeld(next)) finalize();
    }

    // Always active regardless of which observation source(s) end up
    // contributing: Escape (not a menu-accelerator key, so reliably
    // delivered either way) and losing window focus both cancel without
    // recording anything.
    function handleEscape(event: KeyboardEvent) {
      if (event.key === "Escape") setRecording(false);
    }
    function handleBlur() {
      setRecording(false);
    }
    window.addEventListener("keydown", handleEscape);
    window.addEventListener("blur", handleBlur);

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") return; // handled by handleEscape above
      if (isModifierKey(event.key)) {
        event.preventDefault();
      } else if (isSupportedKey(event.code)) {
        event.preventDefault();
        heldExtraKey = event.code;
      }
      observe(snapshotFromEvent(event, heldExtraKey));
    }
    function handleKeyUp(event: KeyboardEvent) {
      if (isModifierKey(event.key)) {
        event.preventDefault();
      } else if (event.code === heldExtraKey) {
        event.preventDefault();
        heldExtraKey = null;
      }
      observe(snapshotFromEvent(event, heldExtraKey));
    }
    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);

    void pollHeldModifiers().then((initial) => {
      if (cancelled || initial === null) return; // no native query on this platform — DOM alone carries it
      observe(initial);
      intervalId = setInterval(() => {
        void pollHeldModifiers().then((next) => {
          if (!cancelled && next !== null) observe(next);
        });
      }, POLL_INTERVAL_MS);
    });

    const watchdogId = setTimeout(() => {
      if (!cancelled) finalize();
    }, WATCHDOG_MS);

    return () => {
      cancelled = true;
      clearTimeout(watchdogId);
      if (intervalId !== undefined) clearInterval(intervalId);
      window.removeEventListener("keydown", handleEscape);
      window.removeEventListener("blur", handleBlur);
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
    };
  }, [recording]);

  function startOrCancelRecording() {
    setRecording((wasRecording) => !wasRecording);
  }

  const buttonText = recording
    ? isComboEmpty(captured)
      ? "Press keys…"
      : comboLabel(captured)
    : comboLabel(combo);

  return (
    <div className="hotkey-recorder" data-testid={testId}>
      <span className="hotkey-recorder__label">{label}</span>
      <button
        type="button"
        className={`hotkey-recorder__button${recording ? " hotkey-recorder__button--recording" : ""}`}
        onClick={startOrCancelRecording}
        data-testid={`${testId}-button`}
        aria-pressed={recording}
      >
        {buttonText}
      </button>
      {recording && (
        <span className="hotkey-recorder__hint" data-testid={`${testId}-hint`}>
          Press Esc to cancel
        </span>
      )}
    </div>
  );
}
