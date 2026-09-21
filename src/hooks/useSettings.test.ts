import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_SETTINGS, useSettings } from "./useSettings";
import type { Settings, ThemeFile } from "../types";

const { invokeMock, listenMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: listenMock,
}));

const CUSTOM_SETTINGS: Settings = {
  formatModifiers: { ctrl: true, alt: false, shift: false, meta: false, key: null },
  toolsModifiers: { ctrl: true, alt: true, shift: false, meta: false, key: null },
  activeThemeId: "yuzu",
  colorOverrides: null,
  ffmpegPath: null,
  pdfiumPath: null,
};

const YUZU: ThemeFile = {
  id: "yuzu",
  name: "Yuzu",
  colors: { primary: "#d4a017", secondary: "#26241c", neutral: "#f7f5ec", surface: "#ffffff", outline: "#e8e2d0" },
};

describe("useSettings", () => {
  beforeEach(() => {
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
    listenMock.mockResolvedValue(() => {});
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "get_settings":
          return Promise.resolve(CUSTOM_SETTINGS);
        case "list_themes":
          return Promise.resolve([YUZU]);
        case "save_settings":
          return Promise.resolve(undefined);
        default:
          return Promise.resolve(undefined);
      }
    });
    document.documentElement.style.cssText = "";
  });

  afterEach(() => {
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
    vi.clearAllMocks();
  });

  it("starts from the built-in defaults before the async load resolves", () => {
    invokeMock.mockImplementation(() => new Promise(() => {})); // never resolves
    const { result } = renderHook(() => useSettings());
    expect(result.current.settings).toEqual(DEFAULT_SETTINGS);
  });

  it("loads persisted settings and the theme list", async () => {
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.settings).toEqual(CUSTOM_SETTINGS));
    expect(result.current.themes).toEqual([YUZU]);
  });

  it("applies the active theme's tokens to the document root", async () => {
    renderHook(() => useSettings());
    await waitFor(() =>
      expect(document.documentElement.style.getPropertyValue("--color-primary")).toBe("#d4a017"),
    );
  });

  it("applies colorOverrides instead of the active theme's own colors when set", async () => {
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "get_settings":
          return Promise.resolve({ ...CUSTOM_SETTINGS, colorOverrides: { ...YUZU.colors, primary: "#123456" } });
        case "list_themes":
          return Promise.resolve([YUZU]);
        default:
          return Promise.resolve(undefined);
      }
    });
    renderHook(() => useSettings());
    await waitFor(() => expect(document.documentElement.style.getPropertyValue("--color-primary")).toBe("#123456"));
  });

  it("falls back to Citrus's colors when the active theme isn't found", async () => {
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "get_settings":
          return Promise.resolve({ ...CUSTOM_SETTINGS, activeThemeId: "does-not-exist" });
        case "list_themes":
          return Promise.resolve([YUZU]);
        default:
          return Promise.resolve(undefined);
      }
    });
    renderHook(() => useSettings());
    await waitFor(() => expect(document.documentElement.style.getPropertyValue("--color-primary")).toBe("#ff7a29"));
  });

  it("persists and applies an update via updateSettings", async () => {
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.settings).toEqual(CUSTOM_SETTINGS));

    const next: Settings = { ...CUSTOM_SETTINGS, activeThemeId: "citrus" };
    await act(async () => {
      await result.current.updateSettings(next);
    });

    expect(invokeMock).toHaveBeenCalledWith("save_settings", { settings: next });
    expect(result.current.settings).toEqual(next);
  });

  it("updates live when a settings-changed event arrives from another window", async () => {
    let emitSettingsChange: ((event: { payload: Settings }) => void) | undefined;
    listenMock.mockImplementation((name: string, handler: (event: { payload: unknown }) => void) => {
      if (name === "settings-changed") emitSettingsChange = handler as typeof emitSettingsChange;
      return Promise.resolve(() => {});
    });

    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(emitSettingsChange).toBeDefined());

    const pushedFromElsewhere: Settings = { ...CUSTOM_SETTINGS, activeThemeId: "midnight-citrus" };
    act(() => {
      emitSettingsChange?.({ payload: pushedFromElsewhere });
    });

    expect(result.current.settings).toEqual(pushedFromElsewhere);
  });

  it("updates the theme list live when a themes-changed event arrives (e.g. an external file edit)", async () => {
    let emitThemesChange: ((event: { payload: ThemeFile[] }) => void) | undefined;
    listenMock.mockImplementation((name: string, handler: (event: { payload: unknown }) => void) => {
      if (name === "themes-changed") emitThemesChange = handler as typeof emitThemesChange;
      return Promise.resolve(() => {});
    });

    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(emitThemesChange).toBeDefined());

    const CUSTOM_THEME: ThemeFile = { id: "my-theme", name: "My Theme", colors: YUZU.colors };
    act(() => {
      emitThemesChange?.({ payload: [CUSTOM_THEME] });
    });

    expect(result.current.themes).toEqual([CUSTOM_THEME]);
  });

  it("falls back to Citrus's colors once a themes-changed event drops the active theme", async () => {
    let emitThemesChange: ((event: { payload: ThemeFile[] }) => void) | undefined;
    listenMock.mockImplementation((name: string, handler: (event: { payload: unknown }) => void) => {
      if (name === "themes-changed") emitThemesChange = handler as typeof emitThemesChange;
      return Promise.resolve(() => {});
    });

    renderHook(() => useSettings());
    await waitFor(() =>
      expect(document.documentElement.style.getPropertyValue("--color-primary")).toBe("#d4a017"),
    );

    // Simulates the active theme's file being deleted out from under the
    // app — the backend itself is what actually corrects `activeThemeId`
    // (a separate `settings-changed` event, not exercised here), but even
    // before that arrives, the theme-resolving effect's own `?? FALLBACK`
    // must not keep showing a theme that's no longer in the list.
    act(() => {
      emitThemesChange?.({ payload: [] });
    });

    await waitFor(() =>
      expect(document.documentElement.style.getPropertyValue("--color-primary")).toBe("#ff7a29"),
    );
  });

  it("re-fetches the theme list via refreshThemes", async () => {
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.themes).toEqual([YUZU]));

    const CUSTOM_THEME: ThemeFile = { id: "my-theme", name: "My Theme", colors: YUZU.colors };
    invokeMock.mockImplementation((command: string) => {
      if (command === "list_themes") return Promise.resolve([YUZU, CUSTOM_THEME]);
      return Promise.resolve(undefined);
    });

    await act(async () => {
      await result.current.refreshThemes();
    });

    expect(result.current.themes).toEqual([YUZU, CUSTOM_THEME]);
  });
});
