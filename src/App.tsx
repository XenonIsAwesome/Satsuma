import { useCallback, useEffect, useRef, useState } from "react";
import { DropZone } from "./components/DropZone";
import { WedgeMenu } from "./components/WedgeMenu";
import { SettingsScreen } from "./components/SettingsScreen";
import { useModifierKeys } from "./hooks/useModifierKeys";
import { useSettings } from "./hooks/useSettings";
import { formatWedgeOptions, getToolOptions } from "./data/wedgeOptions";
import { comboLabel } from "./lib/modifiers";
import { basename, extensionOf } from "./lib/path";
import { convertFile, extractArchive, listConversionTargets } from "./lib/tauri";
import type { DroppedFile, MenuMode, WedgeOption } from "./types";
import "./App.css";

interface Toast {
  kind: "info" | "success" | "danger";
  message: string;
}

const SUCCESS_TOAST_DURATION_MS = 4000;

/** Satsuma's brand mark — a 7-wedge citrus wheel with one wedge pulled out
 * in `--color-primary-active`, matching the real wedge menu's selected
 * state (see `WedgeMenu.css`). Kept as inline geometry rather than an
 * imported asset so it stays a self-contained component, same as
 * `SettingsGearIcon` below; the master vector lives at
 * `assets/satsuma-logo.svg` for README/docs use. */
function SatsumaLogo() {
  return (
    <svg width="40" height="40" viewBox="0 0 400 400" aria-hidden="true">
      {/* Fixed brand backdrop, not `--color-neutral` — that token matches
          the page background (see App.css), which would make this circle
          disappear against it. */}
      <circle cx="200" cy="200" r="200" fill="#C9B896" />
      <path d="M 343.42 79.23 A 187.50 187.50 0 0 0 205.00 12.57 L 205.00 150.25 A 50.00 50.00 0 0 1 235.78 165.07 Z" fill="var(--color-surface)" stroke="var(--color-outline)" strokeWidth="2" strokeLinejoin="round" />
      <path d="M 383.85 236.83 A 187.50 187.50 0 0 0 349.66 87.05 L 242.01 172.89 A 50.00 50.00 0 0 1 249.61 206.20 Z" fill="var(--color-surface)" stroke="var(--color-outline)" strokeWidth="2" strokeLinejoin="round" />
      <path d="M 123.18 371.04 A 187.50 187.50 0 0 0 276.82 371.04 L 217.08 246.99 A 50.00 50.00 0 0 1 182.92 246.99 Z" fill="var(--color-surface)" stroke="var(--color-outline)" strokeWidth="2" strokeLinejoin="round" />
      <path d="M 18.38 246.58 A 187.50 187.50 0 0 0 114.17 366.70 L 173.91 242.65 A 50.00 50.00 0 0 1 152.61 215.94 Z" fill="var(--color-surface)" stroke="var(--color-outline)" strokeWidth="2" strokeLinejoin="round" />
      <path d="M 50.34 87.05 A 187.50 187.50 0 0 0 16.15 236.83 L 150.39 206.20 A 50.00 50.00 0 0 1 157.99 172.89 Z" fill="var(--color-surface)" stroke="var(--color-outline)" strokeWidth="2" strokeLinejoin="round" />
      <path d="M 195.00 12.57 A 187.50 187.50 0 0 0 56.58 79.23 L 164.22 165.07 A 50.00 50.00 0 0 1 195.00 150.25 Z" fill="var(--color-surface)" stroke="var(--color-outline)" strokeWidth="2" strokeLinejoin="round" />
      <path d="M 284.05 367.61 A 187.50 187.50 0 0 0 382.11 244.64 L 247.99 214.03 A 50.00 50.00 0 0 1 224.36 243.67 Z" fill="var(--color-primary-active)" stroke="var(--color-outline)" strokeWidth="2" strokeLinejoin="round" transform="translate(7.82 6.23)" />
      <circle cx="200" cy="200" r="40" fill="var(--color-surface)" stroke="var(--color-outline)" strokeWidth="1.5" />
    </svg>
  );
}

/** A plain gear glyph for the Settings button — no icon library dependency,
 * inherits its color from the button via `currentColor`. */
function SettingsGearIcon() {
  return (
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" aria-hidden="true">
      <circle cx="12" cy="12" r="3" strokeWidth="2" />
      <path
        strokeWidth="2"
        d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"
      />
    </svg>
  );
}

// Context-menu/hotkey launch requests no longer open the menu in this
// window at all — they open the separate borderless overlay window
// (see OverlayApp.tsx) instead, so this window only ever deals with the
// drag-and-drop trigger.
function App() {
  const [view, setView] = useState<"home" | "settings">("home");
  const { settings, updateSettings, themes, refreshThemes } = useSettings();
  const { formatMatched, toolsMatched } = useModifierKeys(settings.formatModifiers, settings.toolsModifiers);
  const [droppedFile, setDroppedFile] = useState<DroppedFile | null>(null);
  const [menuOpen, setMenuOpen] = useState(false);
  const [menuMode, setMenuMode] = useState<MenuMode>("formats");
  const [toast, setToast] = useState<Toast | null>(null);
  // Backend-queried as of Phase 2 (list_conversion_targets), replacing
  // Phase 0's hardcoded per-category format table.
  const [formatOptions, setFormatOptions] = useState<WedgeOption[]>([]);
  const dismissTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  // The tools combination can live-toggle the mode for as long as a
  // modifier-triggered menu stays open (see the effect below); it's null
  // before the menu has ever been opened via that path.
  const [menuTrigger, setMenuTrigger] = useState<"modifier" | null>(null);

  const showToast = useCallback((next: Toast) => {
    clearTimeout(dismissTimer.current);
    setToast(next);
    // Auto-dismiss on success; a failure stays until the next action so the
    // filename + error detail stay readable, per Interaction.md.
    if (next.kind === "success") {
      dismissTimer.current = setTimeout(() => setToast(null), SUCCESS_TOAST_DURATION_MS);
    }
  }, []);

  useEffect(() => () => clearTimeout(dismissTimer.current), []);

  const handleFileDropped = useCallback((file: DroppedFile) => {
    setDroppedFile(file);
    setToast(null);
  }, []);

  // Queries the real backend-driven format list whenever the dropped file
  // changes — see `list_conversion_targets` in src-tauri/src/lib.rs.
  useEffect(() => {
    if (!droppedFile) {
      setFormatOptions([]);
      return;
    }
    let cancelled = false;
    listConversionTargets([droppedFile.path])
      .then((targets) => {
        if (!cancelled) setFormatOptions(formatWedgeOptions(targets));
      })
      .catch(() => {
        if (!cancelled) setFormatOptions([]);
      });
    return () => {
      cancelled = true;
    };
  }, [droppedFile]);

  // Holding the configured format/tools combination while dragging (or
  // right after drop) opens the menu.
  useEffect(() => {
    if (droppedFile && (formatMatched || toolsMatched) && !menuOpen) {
      setMenuTrigger("modifier");
      setMenuMode(toolsMatched ? "tools" : "formats");
      setMenuOpen(true);
    }
  }, [droppedFile, formatMatched, toolsMatched, menuOpen]);

  // Switches between the format menu and the advanced-tools menu, live,
  // for as long as a modifier-triggered menu stays open.
  useEffect(() => {
    if (menuOpen && menuTrigger === "modifier") {
      if (toolsMatched) setMenuMode("tools");
      else if (formatMatched) setMenuMode("formats");
    }
  }, [formatMatched, toolsMatched, menuOpen, menuTrigger]);

  // Keyboard-only path: Enter opens the menu when a file is present.
  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Enter" && droppedFile && !menuOpen) {
        setMenuOpen(true);
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [droppedFile, menuOpen]);

  const handleSelect = useCallback(
    (option: WedgeOption) => {
      setMenuOpen(false);
      const file = droppedFile;
      if (!file) return;

      if (menuMode === "formats") {
        // Phase 2: a real conversion runs, dispatched through
        // satsuma_core::convert — see `convert_file` in
        // src-tauri/src/lib.rs.
        convertFile(file.path, option.id)
          .then((outputPath) => {
            showToast({
              kind: "success",
              message: `Convert to ${option.label} — wrote ${basename(outputPath)}`,
            });
          })
          .catch((error: unknown) => {
            showToast({
              kind: "danger",
              message: `Convert to ${option.label} failed for ${file.name}: ${String(error)}`,
            });
          });
      } else if (option.id === "extract") {
        // Extract Archive is a real, single action (no settings screen),
        // per Tools.md — unlike every other tool wedge, which stays a
        // Phase 0-style fake selection until its own dedicated GUI lands
        // in Phase 3+.
        extractArchive(file.path)
          .then((outputPath) => {
            showToast({ kind: "success", message: `Extracted to ${basename(outputPath)}` });
          })
          .catch((error: unknown) => {
            showToast({ kind: "danger", message: `Extract failed for ${file.name}: ${String(error)}` });
          });
      } else {
        // Advanced-tool wedges stay fake until their own dedicated GUI
        // lands in Phase 3+: no file is written.
        showToast({ kind: "info", message: `${option.label} — ${file.name}` });
      }
    },
    [menuMode, droppedFile, showToast],
  );

  const handleCancel = useCallback(() => {
    setMenuOpen(false);
  }, []);

  const wedgeOptions = menuMode === "formats" ? formatOptions : getToolOptions(droppedFile?.category ?? "unknown");
  const hubLabel = droppedFile ? (extensionOf(droppedFile.path) ?? droppedFile.category).toUpperCase() : undefined;

  if (view === "settings") {
    return (
      <main className="app">
        <SettingsScreen
          settings={settings}
          themes={themes}
          onUpdate={updateSettings}
          onThemesChange={refreshThemes}
          onClose={() => setView("home")}
        />
      </main>
    );
  }

  return (
    <main className="app">
      <button
        type="button"
        className="settings-gear-button"
        onClick={() => setView("settings")}
        aria-label="Settings"
        data-testid="open-settings"
      >
        <SettingsGearIcon />
      </button>
      <div className="app-brand">
        <SatsumaLogo />
        <h1>Satsuma</h1>
      </div>
      <DropZone droppedFile={droppedFile} onFileDropped={handleFileDropped} />
      <WedgeMenu
        open={menuOpen}
        mode={menuMode}
        options={wedgeOptions}
        onSelect={handleSelect}
        onCancel={handleCancel}
        hubLabel={hubLabel}
        toolsHint={comboLabel(settings.toolsModifiers)}
        unimplementedTools={
          // Join tool IDs that are currently in Phase 0 fake mode, excluding the real "extract" action.
          menuMode === "tools" && droppedFile
            ? ["compress", "crop", "trim", "split", "merge"].filter((id) => getToolOptions(droppedFile.category).some((o) => o.id === id)).join(", ") || undefined
            : undefined
        }
      />
      {toast && (
        <p className={`toast toast--${toast.kind}`} data-testid="last-action">
          {toast.message}
        </p>
      )}
    </main>
  );
}

export default App;
