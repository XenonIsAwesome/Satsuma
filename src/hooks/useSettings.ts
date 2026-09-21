import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getSettings, listThemes, saveSettings } from "../lib/tauri";
import { DEFAULT_FORMAT_MODIFIERS, DEFAULT_TOOLS_MODIFIERS } from "../lib/modifiers";
import { applyThemeTokens, FALLBACK_THEME_COLORS, resolveThemeTokens } from "../lib/theme";
import type { Settings, ThemeFile } from "../types";

export const DEFAULT_SETTINGS: Settings = {
  formatModifiers: DEFAULT_FORMAT_MODIFIERS,
  toolsModifiers: DEFAULT_TOOLS_MODIFIERS,
  activeThemeId: "citrus",
  colorOverrides: null,
  ffmpegPath: null,
  pdfiumPath: null,
};

/**
 * Loads Satsuma's persisted `Settings` and saved theme files on mount,
 * applies the resolved active theme to this window's own `document` (each
 * window — the main app, the borderless overlay — has its own, so this
 * must run in both), and stays live-updated via two broadcast events: a
 * `settings-changed` event whenever any window saves a change (see
 * `save_settings` in `src-tauri/src/lib.rs`), and a `themes-changed` event
 * whenever the themes directory changes on disk — including from outside
 * the app entirely (a user hand-editing/adding/deleting a theme file),
 * caught by a filesystem watcher on the backend (see
 * `theme_watcher::watch`). If the currently active theme's file
 * disappears, the backend itself falls `activeThemeId` back to Citrus
 * (recreating Citrus's own file first if that's the one that vanished)
 * and persists it — this hook just reflects whatever it's told, it never
 * has to work out that fallback itself. Starts from `DEFAULT_SETTINGS`
 * (matching Phase 0's previously-hardcoded Shift/Shift+Alt/Citrus
 * behavior) so the app is immediately usable before the async load
 * resolves.
 */
export function useSettings() {
  const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS);
  const [themes, setThemes] = useState<ThemeFile[]>([]);

  const refreshThemes = useCallback(async () => {
    // Guards against a missing/malformed response at the IPC boundary (see
    // the equivalent `getSettings` guard below) — falls back to an empty
    // list, which `useSettings`' theme-resolving effect already treats the
    // same as "active theme not found" (falling back to Citrus's colors).
    const list = (await listThemes()) ?? [];
    setThemes(list);
    return list;
  }, []);

  useEffect(() => {
    const activeTheme = themes.find((theme) => theme.id === settings.activeThemeId);
    const colors = settings.colorOverrides ?? activeTheme?.colors ?? FALLBACK_THEME_COLORS;
    applyThemeTokens(resolveThemeTokens(colors));
  }, [settings.activeThemeId, settings.colorOverrides, themes]);

  useEffect(() => {
    if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) {
      return;
    }

    let cancelled = false;
    void getSettings().then((loaded) => {
      // Guards against a missing/malformed response at the IPC boundary
      // (e.g. an older backend, or a test double that doesn't stub this
      // command) — falls back to keeping `DEFAULT_SETTINGS` rather than
      // setting `settings` to something that isn't a real `Settings`.
      if (!cancelled && loaded) setSettings(loaded);
    });
    void refreshThemes();

    let unlistenSettings: (() => void) | undefined;
    listen<Settings>("settings-changed", (event) => {
      setSettings(event.payload);
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlistenSettings = fn;
      }
    });

    let unlistenThemes: (() => void) | undefined;
    listen<ThemeFile[]>("themes-changed", (event) => {
      setThemes(event.payload);
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlistenThemes = fn;
      }
    });

    return () => {
      cancelled = true;
      unlistenSettings?.();
      unlistenThemes?.();
    };
  }, [refreshThemes]);

  const updateSettings = useCallback(async (next: Settings) => {
    await saveSettings(next);
    setSettings(next);
  }, []);

  return { settings, updateSettings, themes, refreshThemes };
}
