import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useModifierKeys } from "./useModifierKeys";
import { DEFAULT_FORMAT_MODIFIERS, DEFAULT_TOOLS_MODIFIERS } from "../lib/modifiers";
import { createModifierKeyDispatcher } from "../test/modifierEvents";
import type { HotkeyCombo } from "../types";

describe("useModifierKeys", () => {
  it("starts with neither combination matched", () => {
    const { result } = renderHook(() => useModifierKeys(DEFAULT_FORMAT_MODIFIERS, DEFAULT_TOOLS_MODIFIERS));
    expect(result.current).toEqual({ formatMatched: false, toolsMatched: false });
  });

  it("matches the format combo (Shift) once Shift is held", () => {
    const { result } = renderHook(() => useModifierKeys(DEFAULT_FORMAT_MODIFIERS, DEFAULT_TOOLS_MODIFIERS));
    const dispatcher = createModifierKeyDispatcher();

    act(() => dispatcher.down("Shift"));
    expect(result.current).toEqual({ formatMatched: true, toolsMatched: false });

    act(() => dispatcher.up("Shift"));
    expect(result.current).toEqual({ formatMatched: false, toolsMatched: false });
  });

  it("matches the tools combo (Shift+Alt) once both are held, exclusively", () => {
    const { result } = renderHook(() => useModifierKeys(DEFAULT_FORMAT_MODIFIERS, DEFAULT_TOOLS_MODIFIERS));
    const dispatcher = createModifierKeyDispatcher();

    act(() => {
      dispatcher.down("Shift");
      dispatcher.down("Alt");
    });
    expect(result.current).toEqual({ formatMatched: false, toolsMatched: true });
  });

  it("resets to unmatched when the window loses focus", () => {
    const { result } = renderHook(() => useModifierKeys(DEFAULT_FORMAT_MODIFIERS, DEFAULT_TOOLS_MODIFIERS));
    const dispatcher = createModifierKeyDispatcher();

    act(() => {
      dispatcher.down("Shift");
      dispatcher.down("Alt");
    });
    act(() => window.dispatchEvent(new Event("blur")));

    expect(result.current).toEqual({ formatMatched: false, toolsMatched: false });
  });

  it("matches a remapped combination that doesn't involve Shift at all", () => {
    const format: HotkeyCombo = { ctrl: true, alt: false, shift: false, meta: false, key: null };
    const tools: HotkeyCombo = { ctrl: true, alt: true, shift: false, meta: false, key: null };
    const { result } = renderHook(() => useModifierKeys(format, tools));
    const dispatcher = createModifierKeyDispatcher();

    act(() => dispatcher.down("Control"));
    expect(result.current).toEqual({ formatMatched: true, toolsMatched: false });

    act(() => dispatcher.down("Alt"));
    expect(result.current).toEqual({ formatMatched: false, toolsMatched: true });
  });

  it("matches a combination that includes an extra key", () => {
    const format: HotkeyCombo = { ctrl: true, alt: false, shift: false, meta: false, key: "KeyP" };
    const { result } = renderHook(() => useModifierKeys(format, DEFAULT_TOOLS_MODIFIERS));
    const dispatcher = createModifierKeyDispatcher();

    act(() => dispatcher.down("Control"));
    expect(result.current.formatMatched).toBe(false);

    act(() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "p", code: "KeyP", ctrlKey: true })));
    expect(result.current.formatMatched).toBe(true);

    act(() => window.dispatchEvent(new KeyboardEvent("keyup", { key: "p", code: "KeyP", ctrlKey: true })));
    expect(result.current.formatMatched).toBe(false);
  });

  it("ignores a key press that isn't part of either configured combination", () => {
    // Companion to useHeldModifiers' own regression test: a combo with no
    // extra key configured must not react to arbitrary typing elsewhere in
    // the app (e.g. arrow-key wedge-menu navigation).
    const { result } = renderHook(() => useModifierKeys(DEFAULT_FORMAT_MODIFIERS, DEFAULT_TOOLS_MODIFIERS));
    const before = result.current;

    act(() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", code: "ArrowRight" })));

    expect(result.current).toBe(before);
  });
});
