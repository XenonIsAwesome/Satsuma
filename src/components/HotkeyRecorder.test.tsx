import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { HotkeyRecorder } from "./HotkeyRecorder";
import { createModifierKeyDispatcher } from "../test/modifierEvents";
import type { HotkeyCombo } from "../types";

const SHIFT: HotkeyCombo = { ctrl: false, alt: false, shift: true, meta: false, key: null };
const NOTHING_HELD: HotkeyCombo = { ctrl: false, alt: false, shift: false, meta: false, key: null };

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

/** Mocks `poll_held_modifiers` to return each of `results` in turn (the
 * last one repeats for any further calls) — the initial one-off poll and
 * each interval tick both count as one call, in order. */
function mockPollSequence(...results: HotkeyCombo[]) {
  let call = 0;
  invokeMock.mockImplementation((command: string) => {
    if (command !== "poll_held_modifiers") return Promise.resolve(undefined);
    const result = results[Math.min(call, results.length - 1)];
    call += 1;
    return Promise.resolve(result);
  });
}

describe("HotkeyRecorder", () => {
  beforeEach(() => {
    // No native poll support by default (mirrors a plain browser with no
    // Tauri backend at all, as in these tests) — `pollHeldModifiers`
    // catches this and resolves to `null`, so the component falls back to
    // DOM keyboard events, exactly as it did before native polling existed.
    invokeMock.mockRejectedValue(new Error("no Tauri backend in this test"));
  });

  afterEach(() => {
    vi.clearAllMocks();
    vi.useRealTimers();
  });

  it("shows the current combination when not recording", () => {
    render(<HotkeyRecorder label="Format menu" combo={SHIFT} onChange={vi.fn()} testId="format-hotkey" />);
    expect(screen.getByTestId("format-hotkey-button")).toHaveTextContent("Shift");
  });

  it("enters recording mode on click and shows a placeholder before any key is pressed", async () => {
    const user = userEvent.setup();
    render(<HotkeyRecorder label="Format menu" combo={SHIFT} onChange={vi.fn()} testId="format-hotkey" />);

    await user.click(screen.getByTestId("format-hotkey-button"));

    expect(screen.getByTestId("format-hotkey-button")).toHaveTextContent("Press keys…");
    expect(screen.getByTestId("format-hotkey-hint")).toBeInTheDocument();
  });

  describe("DOM keyboard events (no native OS polling available)", () => {
    it("shows the live combination as keys are pressed", async () => {
      const user = userEvent.setup();
      render(<HotkeyRecorder label="Format menu" combo={SHIFT} onChange={vi.fn()} testId="format-hotkey" />);
      await user.click(screen.getByTestId("format-hotkey-button"));
      const dispatcher = createModifierKeyDispatcher();

      act(() => dispatcher.down("Shift"));
      expect(screen.getByTestId("format-hotkey-button")).toHaveTextContent("Shift");

      act(() => dispatcher.down("Alt"));
      expect(screen.getByTestId("format-hotkey-button")).toHaveTextContent("Shift+Alt");
    });

    it("detects Shift+Alt even when only Alt's own keydown ever fires", async () => {
      // Regression test for the DOM-only approach's real failure mode: Alt
      // is treated specially (a menu-accelerator key) by some window
      // managers/webviews and doesn't reliably deliver a clean
      // keydown/keyup for *every* key in a combo — a physically-held Shift
      // got dropped because Shift's own keydown never reached the page. A
      // real browser still stamps `shiftKey: true` on Alt's own keydown in
      // that situation, so reading the event's flags directly (rather than
      // tracking "have I personally seen a keydown for this key") must
      // still report the full Shift+Alt combo. Native OS polling (see the
      // "native OS polling" describe block below) sidesteps this class of
      // bug entirely by not depending on the DOM event arriving at all.
      const user = userEvent.setup();
      render(<HotkeyRecorder label="Tools menu" combo={SHIFT} onChange={vi.fn()} testId="tools-hotkey" />);
      await user.click(screen.getByTestId("tools-hotkey-button"));

      act(() =>
        window.dispatchEvent(new KeyboardEvent("keydown", { key: "Alt", altKey: true, shiftKey: true })),
      );

      expect(screen.getByTestId("tools-hotkey-button")).toHaveTextContent("Shift+Alt");
    });

    it("finalizes and calls onChange once every key is released", async () => {
      const user = userEvent.setup();
      const onChange = vi.fn();
      render(<HotkeyRecorder label="Format menu" combo={SHIFT} onChange={onChange} testId="format-hotkey" />);
      await user.click(screen.getByTestId("format-hotkey-button"));
      const dispatcher = createModifierKeyDispatcher();

      act(() => dispatcher.down("Control"));
      act(() => dispatcher.up("Control"));

      expect(onChange).toHaveBeenCalledWith({ ctrl: true, alt: false, shift: false, meta: false, key: null });
      // A controlled component: it keeps showing the `combo` prop until the
      // parent actually accepts the change and passes a new one back down.
      expect(screen.getByTestId("format-hotkey-button")).toHaveTextContent("Shift");
      expect(screen.queryByTestId("format-hotkey-hint")).not.toBeInTheDocument();
    });

    it("records the peak combination even if a key is released before the others", async () => {
      const user = userEvent.setup();
      const onChange = vi.fn();
      render(<HotkeyRecorder label="Tools menu" combo={SHIFT} onChange={onChange} testId="tools-hotkey" />);
      await user.click(screen.getByTestId("tools-hotkey-button"));
      const dispatcher = createModifierKeyDispatcher();

      act(() => dispatcher.down("Shift"));
      act(() => dispatcher.down("Alt"));
      act(() => dispatcher.up("Alt")); // released first, Shift still held
      act(() => dispatcher.up("Shift")); // now nothing is held

      expect(onChange).toHaveBeenCalledWith({ ctrl: false, alt: true, shift: true, meta: false, key: null });
    });

    it("cancels when the button is clicked again while recording", async () => {
      const user = userEvent.setup();
      const onChange = vi.fn();
      render(<HotkeyRecorder label="Format menu" combo={SHIFT} onChange={onChange} testId="format-hotkey" />);
      await user.click(screen.getByTestId("format-hotkey-button"));
      const dispatcher = createModifierKeyDispatcher();
      act(() => dispatcher.down("Alt"));

      await user.click(screen.getByTestId("format-hotkey-button"));

      expect(onChange).not.toHaveBeenCalled();
      expect(screen.getByTestId("format-hotkey-button")).toHaveTextContent("Shift");
    });

    it("captures a modifier plus an arbitrary extra key", async () => {
      const user = userEvent.setup();
      const onChange = vi.fn();
      render(<HotkeyRecorder label="Format menu" combo={SHIFT} onChange={onChange} testId="format-hotkey" />);
      await user.click(screen.getByTestId("format-hotkey-button"));
      const dispatcher = createModifierKeyDispatcher();

      act(() => dispatcher.down("Control"));
      act(() =>
        window.dispatchEvent(new KeyboardEvent("keydown", { key: "p", code: "KeyP", ctrlKey: true })),
      );
      expect(screen.getByTestId("format-hotkey-button")).toHaveTextContent("Ctrl+P");

      act(() =>
        window.dispatchEvent(new KeyboardEvent("keyup", { key: "p", code: "KeyP", ctrlKey: true })),
      );
      act(() => dispatcher.up("Control"));

      expect(onChange).toHaveBeenCalledWith({ ctrl: true, alt: false, shift: false, meta: false, key: "KeyP" });
    });

    it("ignores an unsupported key, recording only the modifiers", async () => {
      const user = userEvent.setup();
      const onChange = vi.fn();
      render(<HotkeyRecorder label="Format menu" combo={SHIFT} onChange={onChange} testId="format-hotkey" />);
      await user.click(screen.getByTestId("format-hotkey-button"));
      const dispatcher = createModifierKeyDispatcher();

      act(() => dispatcher.down("Control"));
      act(() =>
        window.dispatchEvent(
          new KeyboardEvent("keydown", { key: "NumLock", code: "NumLock", ctrlKey: true }),
        ),
      );
      expect(screen.getByTestId("format-hotkey-button")).toHaveTextContent("Ctrl");

      act(() => dispatcher.up("Control"));
      expect(onChange).toHaveBeenCalledWith({ ctrl: true, alt: false, shift: false, meta: false, key: null });
    });

    it("cancels when the window loses focus mid-recording", async () => {
      const user = userEvent.setup();
      const onChange = vi.fn();
      render(<HotkeyRecorder label="Format menu" combo={SHIFT} onChange={onChange} testId="format-hotkey" />);
      await user.click(screen.getByTestId("format-hotkey-button"));
      const dispatcher = createModifierKeyDispatcher();
      act(() => dispatcher.down("Alt"));

      act(() => window.dispatchEvent(new Event("blur")));

      expect(onChange).not.toHaveBeenCalled();
      expect(screen.getByTestId("format-hotkey-button")).toHaveTextContent("Shift");
    });
  });

  describe("native OS polling (Windows GetAsyncKeyState / Linux X11 XQueryPointer)", () => {
    it("stays in recording mode when the first poll (the realistic case) reports nothing held yet", async () => {
      // Regression test: the very first poll right after clicking the
      // button normally shows nothing held at all — the user hasn't
      // pressed a key yet. Treating that as "everything was released" was
      // a real bug that instantly cancelled recording before it was ever
      // possible to press anything, making the button appear unclickable.
      vi.useFakeTimers();
      mockPollSequence(NOTHING_HELD);
      const onChange = vi.fn();
      render(<HotkeyRecorder label="Format menu" combo={SHIFT} onChange={onChange} testId="format-hotkey" />);

      fireEvent.click(screen.getByTestId("format-hotkey-button"));
      await act(() => vi.advanceTimersByTimeAsync(0));

      expect(onChange).not.toHaveBeenCalled();
      expect(screen.getByTestId("format-hotkey-button")).toHaveTextContent("Press keys…");
      expect(screen.getByTestId("format-hotkey-hint")).toBeInTheDocument();
    });

    it("detects a two-key combo with no DOM keyboard events dispatched at all", async () => {
      // This is the actual regression the DOM-event approach couldn't
      // fix: some window managers/webviews never deliver Alt's own
      // keydown/keyup to the page. Polling the OS directly doesn't depend
      // on any DOM event arriving — this test proves that by never
      // dispatching one.
      vi.useFakeTimers();
      mockPollSequence(
        NOTHING_HELD, // initial poll: nothing pressed yet
        { ctrl: false, alt: false, shift: true, meta: false, key: null }, // tick: Shift down
        { ctrl: false, alt: true, shift: true, meta: false, key: null }, // tick: Shift+Alt down
        NOTHING_HELD, // tick: released
      );
      const onChange = vi.fn();
      render(<HotkeyRecorder label="Tools menu" combo={SHIFT} onChange={onChange} testId="tools-hotkey" />);

      fireEvent.click(screen.getByTestId("tools-hotkey-button"));
      await act(() => vi.advanceTimersByTimeAsync(0)); // flush the initial poll
      expect(screen.getByTestId("tools-hotkey-button")).toHaveTextContent("Press keys…");

      await act(() => vi.advanceTimersByTimeAsync(50));
      expect(screen.getByTestId("tools-hotkey-button")).toHaveTextContent("Shift");

      await act(() => vi.advanceTimersByTimeAsync(50));
      expect(screen.getByTestId("tools-hotkey-button")).toHaveTextContent("Shift+Alt");

      await act(() => vi.advanceTimersByTimeAsync(50));
      expect(onChange).toHaveBeenCalledWith({ ctrl: false, alt: true, shift: true, meta: false, key: null });
    });

    it("records the peak combination when the poll briefly shows both before either releases", async () => {
      vi.useFakeTimers();
      mockPollSequence(
        NOTHING_HELD,
        { ctrl: false, alt: false, shift: true, meta: false, key: null },
        { ctrl: false, alt: true, shift: true, meta: false, key: null },
        { ctrl: false, alt: false, shift: true, meta: false, key: null }, // Alt released first
        NOTHING_HELD, // then Shift
      );
      const onChange = vi.fn();
      render(<HotkeyRecorder label="Tools menu" combo={SHIFT} onChange={onChange} testId="tools-hotkey" />);

      fireEvent.click(screen.getByTestId("tools-hotkey-button"));
      await act(() => vi.advanceTimersByTimeAsync(0));
      await act(() => vi.advanceTimersByTimeAsync(50));
      await act(() => vi.advanceTimersByTimeAsync(50));
      await act(() => vi.advanceTimersByTimeAsync(50));
      await act(() => vi.advanceTimersByTimeAsync(50));

      expect(onChange).toHaveBeenCalledWith({ ctrl: false, alt: true, shift: true, meta: false, key: null });
    });

    it("still cancels on Escape while polling, after a key is actually captured", async () => {
      vi.useFakeTimers();
      mockPollSequence(NOTHING_HELD, { ctrl: false, alt: true, shift: true, meta: false, key: null });
      const onChange = vi.fn();
      render(<HotkeyRecorder label="Format menu" combo={SHIFT} onChange={onChange} testId="format-hotkey" />);

      fireEvent.click(screen.getByTestId("format-hotkey-button"));
      await act(() => vi.advanceTimersByTimeAsync(0));
      await act(() => vi.advanceTimersByTimeAsync(50));
      expect(screen.getByTestId("format-hotkey-button")).toHaveTextContent("Shift+Alt");

      act(() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" })));

      expect(onChange).not.toHaveBeenCalled();
      expect(screen.getByTestId("format-hotkey-button")).toHaveTextContent("Shift");
      expect(screen.queryByTestId("format-hotkey-hint")).not.toBeInTheDocument();
    });

    it("keeps polling for as long as recording stays active, and stops once it ends", async () => {
      vi.useFakeTimers();
      mockPollSequence(NOTHING_HELD);
      render(<HotkeyRecorder label="Format menu" combo={SHIFT} onChange={vi.fn()} testId="format-hotkey" />);

      fireEvent.click(screen.getByTestId("format-hotkey-button"));
      await act(() => vi.advanceTimersByTimeAsync(0));
      const callsAfterInitialPoll = invokeMock.mock.calls.length;
      expect(callsAfterInitialPoll).toBeGreaterThan(0);

      // Recording is still active (nothing held yet doesn't cancel it —
      // see the regression test above), so polling keeps going.
      await act(() => vi.advanceTimersByTimeAsync(150));
      expect(invokeMock.mock.calls.length).toBeGreaterThan(callsAfterInitialPoll);

      act(() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" })));
      const callsAtCancel = invokeMock.mock.calls.length;
      await act(() => vi.advanceTimersByTimeAsync(500));

      expect(invokeMock.mock.calls.length).toBe(callsAtCancel);
    });
  });

  describe("merging both sources — neither one alone is fully trustworthy", () => {
    it("still detects Shift+Alt when polling has a blind spot for Alt specifically (the bug this fixes)", async () => {
      // Reproduces the real-world report this dual-signal design exists
      // for: on some systems, native OS polling reliably tracks Shift's
      // own down/up transitions but never reports Alt as held, even while
      // it's genuinely being held — the mirror-image failure of the
      // DOM-only approach's own blind spot. DOM events (dispatched here
      // for both keys) independently catch what polling misses. Polling
      // still correctly tracks Shift's *real* release (rather than being
      // frozen forever), matching what an actual blind spot for one
      // specific key — not a fully broken poll — would look like.
      vi.useFakeTimers();
      mockPollSequence(
        NOTHING_HELD, // initial poll, right after the click
        { ctrl: false, alt: false, shift: true, meta: false, key: null }, // tick: sees Shift go down
        { ctrl: false, alt: false, shift: true, meta: false, key: null }, // tick: Alt press invisible to it
        { ctrl: false, alt: false, shift: true, meta: false, key: null }, // tick: Alt release invisible to it
        NOTHING_HELD, // tick: correctly sees Shift's real release
      );
      const onChange = vi.fn();
      render(<HotkeyRecorder label="Tools menu" combo={SHIFT} onChange={onChange} testId="tools-hotkey" />);
      const dispatcher = createModifierKeyDispatcher();

      fireEvent.click(screen.getByTestId("tools-hotkey-button"));
      await act(() => vi.advanceTimersByTimeAsync(0)); // initial poll: nothing held

      act(() => dispatcher.down("Shift"));
      await act(() => vi.advanceTimersByTimeAsync(50)); // poll tick: Shift held (agrees with DOM)

      act(() => dispatcher.down("Alt")); // polling will never corroborate this, DOM does
      await act(() => vi.advanceTimersByTimeAsync(50)); // poll tick: still just Shift, per its blind spot
      expect(screen.getByTestId("tools-hotkey-button")).toHaveTextContent("Shift+Alt");

      act(() => dispatcher.up("Alt"));
      await act(() => vi.advanceTimersByTimeAsync(50)); // poll tick: still just Shift
      expect(onChange).not.toHaveBeenCalled(); // Shift is still genuinely held

      act(() => dispatcher.up("Shift"));
      await act(() => vi.advanceTimersByTimeAsync(50)); // poll tick: now agrees nothing is held

      expect(onChange).toHaveBeenCalledWith({ ctrl: false, alt: true, shift: true, meta: false, key: null });
    });

    it("still detects Shift+Alt when DOM events only ever report Shift (polling catches Alt instead)", async () => {
      // The original failure mode, with polling now covering for it: only
      // Shift's own keydown/keyup ever reaches the DOM (Alt's is swallowed
      // by the window manager/webview), but polling independently observes
      // Alt going down and back up.
      vi.useFakeTimers();
      const onChange = vi.fn();
      render(<HotkeyRecorder label="Tools menu" combo={SHIFT} onChange={onChange} testId="tools-hotkey" />);
      const dispatcher = createModifierKeyDispatcher();

      mockPollSequence(NOTHING_HELD, { ctrl: false, alt: true, shift: false, meta: false, key: null }, NOTHING_HELD);
      fireEvent.click(screen.getByTestId("tools-hotkey-button"));
      await act(() => vi.advanceTimersByTimeAsync(0));

      act(() => dispatcher.down("Shift")); // DOM only ever sees Shift
      await act(() => vi.advanceTimersByTimeAsync(50)); // poll tick: Alt down
      expect(screen.getByTestId("tools-hotkey-button")).toHaveTextContent("Shift+Alt");

      act(() => dispatcher.up("Shift"));
      await act(() => vi.advanceTimersByTimeAsync(50)); // poll tick: everything released

      expect(onChange).toHaveBeenCalledWith({ ctrl: false, alt: true, shift: true, meta: false, key: null });
    });

    it("finalizes via DOM's own release even if polling is permanently stuck reporting a key as held", async () => {
      // Regression test: an earlier version required *both* sources to
      // agree nothing was held before finalizing. If one source ever got
      // stuck (kept reporting the same stale non-empty value forever,
      // never catching up to the real release), the recorder hung
      // indefinitely — reported as "it doesn't stop recording." Polling
      // here correctly sees Control go down, then — unlike a real
      // implementation — never notices it come back up again.
      vi.useFakeTimers();
      mockPollSequence(NOTHING_HELD, { ctrl: true, alt: false, shift: false, meta: false, key: null });
      const onChange = vi.fn();
      render(<HotkeyRecorder label="Format menu" combo={SHIFT} onChange={onChange} testId="format-hotkey" />);
      const dispatcher = createModifierKeyDispatcher();

      fireEvent.click(screen.getByTestId("format-hotkey-button"));
      await act(() => vi.advanceTimersByTimeAsync(0)); // initial poll: nothing held

      act(() => dispatcher.down("Control"));
      await act(() => vi.advanceTimersByTimeAsync(50)); // poll tick agrees: Control held

      act(() => dispatcher.up("Control")); // DOM's own fresh release, independent of poll's stuck value
      expect(onChange).toHaveBeenCalledWith({ ctrl: true, alt: false, shift: false, meta: false, key: null });
    });

    it("does not finalize early just because polling's stale first sample predates a DOM press", async () => {
      // Regression test for a bug caught during review, never actually
      // shipped: judging a fresh DOM observation against polling's *last
      // known* value (rather than against nothing) meant polling's very
      // first sample — taken before any key was pressed, and not refreshed
      // again until its next tick — could look like "polling confirms
      // release" the instant DOM captured a press, cutting recording short
      // after just one key.
      vi.useFakeTimers();
      mockPollSequence(
        NOTHING_HELD, // initial poll, still sitting there unrefreshed...
        { ctrl: false, alt: true, shift: true, meta: false, key: null }, // ...until this tick, well after the DOM presses below
      );
      const onChange = vi.fn();
      render(<HotkeyRecorder label="Tools menu" combo={SHIFT} onChange={onChange} testId="tools-hotkey" />);
      const dispatcher = createModifierKeyDispatcher();

      fireEvent.click(screen.getByTestId("tools-hotkey-button"));
      await act(() => vi.advanceTimersByTimeAsync(0)); // initial poll lands: nothing held

      // Both presses happen well inside the 50ms window before the next
      // poll tick, while polling's only sample so far is still "nothing".
      act(() => dispatcher.down("Shift"));
      act(() => dispatcher.down("Alt"));

      expect(onChange).not.toHaveBeenCalled();
      expect(screen.getByTestId("tools-hotkey-button")).toHaveTextContent("Shift+Alt");

      act(() => dispatcher.up("Alt"));
      act(() => dispatcher.up("Shift"));

      expect(onChange).toHaveBeenCalledWith({ ctrl: false, alt: true, shift: true, meta: false, key: null });
    });

    it("force-finalizes via the watchdog if somehow still recording after 8 seconds", async () => {
      // Last-resort backstop in case some future environment manages to
      // wedge both observation sources at once, so that *neither* one ever
      // independently produces a fresh "nothing held" reading of its own —
      // polling agrees Control is held for as long as recording continues,
      // and it's never released via DOM either.
      vi.useFakeTimers();
      mockPollSequence({ ctrl: true, alt: false, shift: false, meta: false, key: null });
      const onChange = vi.fn();
      render(<HotkeyRecorder label="Format menu" combo={SHIFT} onChange={onChange} testId="format-hotkey" />);
      const dispatcher = createModifierKeyDispatcher();

      fireEvent.click(screen.getByTestId("format-hotkey-button"));
      await act(() => vi.advanceTimersByTimeAsync(0));
      act(() => dispatcher.down("Control")); // never released via DOM either

      await act(() => vi.advanceTimersByTimeAsync(7999));
      expect(onChange).not.toHaveBeenCalled();

      await act(() => vi.advanceTimersByTimeAsync(1));
      expect(onChange).toHaveBeenCalledWith({ ctrl: true, alt: false, shift: false, meta: false, key: null });
    });
  });
});
