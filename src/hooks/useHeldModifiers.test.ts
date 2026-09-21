import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useHeldModifiers } from "./useHeldModifiers";
import { createModifierKeyDispatcher } from "../test/modifierEvents";

describe("useHeldModifiers", () => {
  it("starts with nothing held", () => {
    const { result } = renderHook(() => useHeldModifiers());
    expect(result.current).toEqual({ ctrl: false, alt: false, shift: false, meta: false, key: null });
  });

  it("tracks each modifier independently", () => {
    const { result } = renderHook(() => useHeldModifiers());
    const dispatcher = createModifierKeyDispatcher();

    act(() => dispatcher.down("Control"));
    act(() => dispatcher.down("Meta"));
    expect(result.current).toEqual({ ctrl: true, alt: false, shift: false, meta: true, key: null });

    act(() => dispatcher.up("Control"));
    expect(result.current).toEqual({ ctrl: false, alt: false, shift: false, meta: true, key: null });
  });

  it("detects two modifiers held at once, even if only one key's own events are dispatched", () => {
    // Regression test: reconstructing state from each key's individual
    // keydown/keyup used to miss a second modifier pressed alongside an
    // already-held one in real usage (Alt in particular). Reading the
    // event's own ctrlKey/altKey/shiftKey/metaKey flags — as set here by
    // the dispatcher, exactly as a real browser would — must report both
    // as held even though only Alt's own keydown fires.
    const { result } = renderHook(() => useHeldModifiers());
    const dispatcher = createModifierKeyDispatcher();

    act(() => dispatcher.down("Shift"));
    act(() => dispatcher.down("Alt"));

    expect(result.current).toEqual({ ctrl: false, alt: true, shift: true, meta: false, key: null });
  });

  it("ignores non-modifier keys", () => {
    const { result } = renderHook(() => useHeldModifiers());
    act(() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter" })));
    expect(result.current).toEqual({ ctrl: false, alt: false, shift: false, meta: false, key: null });
  });

  it("ignores a supported key that isn't in watchedKeys", () => {
    // Regression test: this hook runs for as long as the app is mounted,
    // not just during a short deliberate recording gesture — reporting
    // *every* letter/digit/arrow key press regardless of whether any
    // configured combo cares about it caused unrelated keystrokes
    // elsewhere in the app (e.g. arrow-key wedge-menu navigation) to
    // trigger a re-render here, which in turn reset unrelated state
    // further down the tree. See the hook's own doc comment.
    const { result } = renderHook(() => useHeldModifiers(["KeyP"]));
    act(() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", code: "ArrowRight" })));
    expect(result.current).toEqual({ ctrl: false, alt: false, shift: false, meta: false, key: null });
  });

  it("tracks a watched extra key across its own keydown/keyup pair", () => {
    const { result } = renderHook(() => useHeldModifiers(["KeyP"]));

    act(() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "p", code: "KeyP" })));
    expect(result.current).toEqual({ ctrl: false, alt: false, shift: false, meta: false, key: "KeyP" });

    act(() => window.dispatchEvent(new KeyboardEvent("keyup", { key: "p", code: "KeyP" })));
    expect(result.current).toEqual({ ctrl: false, alt: false, shift: false, meta: false, key: null });
  });

  it("resets everything on window blur", () => {
    const { result } = renderHook(() => useHeldModifiers());
    const dispatcher = createModifierKeyDispatcher();

    act(() => {
      dispatcher.down("Shift");
      dispatcher.down("Alt");
    });
    act(() => window.dispatchEvent(new Event("blur")));

    expect(result.current).toEqual({ ctrl: false, alt: false, shift: false, meta: false, key: null });
  });
});
