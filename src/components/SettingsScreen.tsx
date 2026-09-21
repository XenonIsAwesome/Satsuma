import { useEffect, useState } from "react";
import { HotkeyRecorder } from "./HotkeyRecorder";
import { LinuxIntegrationPanel } from "./LinuxIntegrationPanel";
import {
  DEFAULT_FORMAT_MODIFIERS,
  DEFAULT_TOOLS_MODIFIERS,
  reservedComboWarning,
  validateHotkeys,
} from "../lib/modifiers";
import {
  currentPlatform,
  ffmpegStatus,
  isAutostartEnabled,
  openThemesDir,
  pdfiumStatus,
  pickExecutableFile,
  saveTheme,
  setAutostartEnabled,
  themesDirDisplay,
} from "../lib/tauri";
import { FALLBACK_THEME_COLORS } from "../lib/theme";
import type { CustomColors, HotkeyCombo, Settings, ThemeFile } from "../types";
import "./SettingsScreen.css";

interface SettingsScreenProps {
  settings: Settings;
  themes: ThemeFile[];
  onUpdate: (next: Settings) => Promise<void>;
  onThemesChange: () => Promise<ThemeFile[]>;
  onClose: () => void;
}

/** A sentinel dropdown value — not a real saved theme id — that reveals the
 * color pickers and "save as" row for hand-tweaking colors, per Phase
 * 1.1's design note. Editing a real saved theme still starts from that
 * theme's own colors; nothing is overwritten unless explicitly saved. */
const CUSTOM_OPTION_ID = "custom";

const CUSTOM_COLOR_FIELDS: { field: keyof CustomColors; label: string }[] = [
  { field: "primary", label: "Primary" },
  { field: "secondary", label: "Secondary" },
  { field: "neutral", label: "Neutral (page background)" },
  { field: "surface", label: "Surface (cards)" },
  { field: "outline", label: "Outline" },
];

/**
 * Phase 1's unified Settings screen: hotkey remapping for the format/tools
 * trigger combinations, a data-driven theme picker (every theme — built-in
 * or user-made — is a file under `~/.config/satsuma/themes/`, listed here
 * and editable/saveable as a new file), the Linux file-manager integration
 * toggle, and "Start at Login" — one surface, per Phase 1's design note,
 * rather than settings scattered across ad hoc panels.
 *
 * Hotkey/theme edits apply immediately via `onUpdate` (no separate "Save"
 * step, no restart) — an invalid hotkey combination is kept as local
 * (unsaved) draft state and surfaced as an inline error instead of being
 * persisted, per Phase 1's success criteria. A color tweak similarly
 * applies live as an unsaved override; it only becomes a real, reusable
 * theme file once explicitly named and saved below the color pickers.
 */
export function SettingsScreen({ settings, themes, onUpdate, onThemesChange, onClose }: SettingsScreenProps) {
  const [draftFormat, setDraftFormat] = useState<HotkeyCombo>(settings.formatModifiers);
  const [draftTools, setDraftTools] = useState<HotkeyCombo>(settings.toolsModifiers);
  const [platform, setPlatform] = useState<string | null>(null);
  const [autostart, setAutostart] = useState(false);
  const [autostartBusy, setAutostartBusy] = useState(false);
  const [newThemeName, setNewThemeName] = useState("");
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [themesDir, setThemesDir] = useState<string | null>(null);
  const [openDirError, setOpenDirError] = useState<string | null>(null);
  const [draftFfmpegPath, setDraftFfmpegPath] = useState(settings.ffmpegPath ?? "");
  const [draftPdfiumPath, setDraftPdfiumPath] = useState(settings.pdfiumPath ?? "");
  const [ffmpegResolved, setFfmpegResolved] = useState<string | null>(null);
  const [pdfiumResolved, setPdfiumResolved] = useState<string | null>(null);

  // Resyncs local draft state whenever settings change from outside (e.g. a
  // `settings-changed` event from another window). Two windows editing
  // hotkeys at the exact same moment isn't a scenario Phase 1 needs to
  // reconcile more carefully than "last write wins."
  useEffect(() => {
    setDraftFormat(settings.formatModifiers);
    setDraftTools(settings.toolsModifiers);
  }, [settings.formatModifiers, settings.toolsModifiers]);

  useEffect(() => {
    setDraftFfmpegPath(settings.ffmpegPath ?? "");
  }, [settings.ffmpegPath]);

  useEffect(() => {
    setDraftPdfiumPath(settings.pdfiumPath ?? "");
  }, [settings.pdfiumPath]);

  useEffect(() => {
    void currentPlatform().then(setPlatform);
    void isAutostartEnabled().then(setAutostart);
    void themesDirDisplay().then(setThemesDir);
  }, []);

  // Re-checks what's actually resolved (bundled sidecar, system fallback,
  // or the user's own override) whenever the saved path changes — not on
  // every keystroke in the draft input, since that would round-trip to
  // the backend on each character typed.
  useEffect(() => {
    void ffmpegStatus().then(setFfmpegResolved);
  }, [settings.ffmpegPath]);

  useEffect(() => {
    void pdfiumStatus().then(setPdfiumResolved);
  }, [settings.pdfiumPath]);

  const validationError = validateHotkeys(draftFormat, draftTools);
  const formatWarning = platform ? reservedComboWarning(draftFormat, platform) : null;
  const toolsWarning = platform ? reservedComboWarning(draftTools, platform) : null;

  function handleFormatChange(next: HotkeyCombo) {
    setDraftFormat(next);
    if (!validateHotkeys(next, draftTools)) {
      void onUpdate({ ...settings, formatModifiers: next });
    }
  }

  function handleToolsChange(next: HotkeyCombo) {
    setDraftTools(next);
    if (!validateHotkeys(draftFormat, next)) {
      void onUpdate({ ...settings, toolsModifiers: next });
    }
  }

  /** Reverts only the hotkey bindings to their built-in defaults
   * (Shift / Shift+Alt) — deliberately leaves `activeThemeId` and
   * `colorOverrides` untouched, since a hotkey reset shouldn't also throw
   * away a theme someone's picked or is mid-tweak on. */
  function handleResetHotkeys() {
    setDraftFormat(DEFAULT_FORMAT_MODIFIERS);
    setDraftTools(DEFAULT_TOOLS_MODIFIERS);
    void onUpdate({
      ...settings,
      formatModifiers: DEFAULT_FORMAT_MODIFIERS,
      toolsModifiers: DEFAULT_TOOLS_MODIFIERS,
    });
  }

  const activeTheme = themes.find((theme) => theme.id === settings.activeThemeId);
  const currentColors = settings.colorOverrides ?? activeTheme?.colors ?? FALLBACK_THEME_COLORS;
  // "Custom" isn't a stored field — it's however the dropdown reflects
  // "there's a colorOverrides in play right now" (see CUSTOM_OPTION_ID).
  const isEditingCustomColors = settings.colorOverrides !== null;
  const themeSelectValue = isEditingCustomColors ? CUSTOM_OPTION_ID : settings.activeThemeId;

  function handleThemeSelect(id: string) {
    if (id === CUSTOM_OPTION_ID) {
      // Seeds the pickers from whatever's currently shown (the active
      // theme's own colors) rather than some arbitrary starting point.
      void onUpdate({ ...settings, colorOverrides: currentColors });
    } else {
      void onUpdate({ ...settings, activeThemeId: id, colorOverrides: null });
    }
  }

  function handleColorChange(field: keyof CustomColors, value: string) {
    setSaveError(null);
    void onUpdate({ ...settings, colorOverrides: { ...currentColors, [field]: value } });
  }

  async function handleSaveTheme() {
    const name = newThemeName.trim();
    if (!name) return;
    setSaving(true);
    setSaveError(null);
    try {
      const saved = await saveTheme(name, currentColors);
      await onThemesChange();
      await onUpdate({ ...settings, activeThemeId: saved.id, colorOverrides: null });
      setNewThemeName("");
    } catch (error) {
      setSaveError(String(error));
    } finally {
      setSaving(false);
    }
  }

  async function handleOpenThemesDir() {
    setOpenDirError(null);
    try {
      await openThemesDir();
    } catch (error) {
      setOpenDirError(String(error));
    }
  }

  async function handleAutostartToggle() {
    setAutostartBusy(true);
    try {
      const next = !autostart;
      await setAutostartEnabled(next);
      setAutostart(next);
    } finally {
      setAutostartBusy(false);
    }
  }

  /** Commits the typed/pasted draft path on blur (rather than on every
   * keystroke) — an empty field means "use the default," saved as `null`
   * rather than an empty string. */
  function handleFfmpegPathBlur() {
    const trimmed = draftFfmpegPath.trim();
    if (trimmed === (settings.ffmpegPath ?? "")) return;
    void onUpdate({ ...settings, ffmpegPath: trimmed || null });
  }

  function handlePdfiumPathBlur() {
    const trimmed = draftPdfiumPath.trim();
    if (trimmed === (settings.pdfiumPath ?? "")) return;
    void onUpdate({ ...settings, pdfiumPath: trimmed || null });
  }

  async function handleBrowseFfmpeg() {
    const picked = await pickExecutableFile("Select the FFmpeg binary");
    if (!picked) return;
    setDraftFfmpegPath(picked);
    void onUpdate({ ...settings, ffmpegPath: picked });
  }

  async function handleBrowsePdfium() {
    const picked = await pickExecutableFile("Select the pdfium library file");
    if (!picked) return;
    setDraftPdfiumPath(picked);
    void onUpdate({ ...settings, pdfiumPath: picked });
  }

  function handleResetFfmpegPath() {
    setDraftFfmpegPath("");
    void onUpdate({ ...settings, ffmpegPath: null });
  }

  function handleResetPdfiumPath() {
    setDraftPdfiumPath("");
    void onUpdate({ ...settings, pdfiumPath: null });
  }

  return (
    <div className="settings-screen" data-testid="settings-screen">
      <div className="settings-screen__header">
        <h2>Settings</h2>
        <button type="button" className="button-ghost" onClick={onClose} data-testid="settings-close">
          Back
        </button>
      </div>

      <section className="settings-card">
        <div className="settings-card__header">
          <h3>Hotkeys</h3>
          <button
            type="button"
            className="button-ghost"
            onClick={handleResetHotkeys}
            data-testid="reset-hotkeys"
          >
            Reset to defaults
          </button>
        </div>
        <p className="settings-card__hint">
          Click a hotkey, then press the combination you want. Hold either combination to open Satsuma's radial
          menu from a drag, the desktop overlay, or your file manager.
        </p>
        {platform === "windows" && (
          <p className="settings-card__hint" data-testid="windows-key-note">
            On Windows, an extra key beyond Ctrl/Alt/Shift/Win works for drag-and-drop within Satsuma and for the
            overlay's tools menu, but not for triggering the menu from your file manager — that path only ever
            watches the modifiers.
          </p>
        )}
        <HotkeyRecorder
          label="Format menu"
          combo={draftFormat}
          onChange={handleFormatChange}
          testId="format-hotkey"
        />
        <HotkeyRecorder label="Tools menu" combo={draftTools} onChange={handleToolsChange} testId="tools-hotkey" />
        {validationError && (
          <p className="settings-card__error" data-testid="hotkey-error">
            {validationError}
          </p>
        )}
        {!validationError && formatWarning && (
          <p className="settings-card__warning" data-testid="hotkey-warning-format">
            {formatWarning}
          </p>
        )}
        {!validationError && toolsWarning && (
          <p className="settings-card__warning" data-testid="hotkey-warning-tools">
            {toolsWarning}
          </p>
        )}
      </section>

      <section className="settings-card">
        <div className="settings-card__header">
          <h3>Theme</h3>
          <button
            type="button"
            className="button-ghost"
            onClick={() => void handleOpenThemesDir()}
            data-testid="open-themes-dir"
          >
            Open theme folder
          </button>
        </div>
        <p className="settings-card__hint">
          {themesDir ? <>Themes are files under <code>{themesDir}</code></> : "Themes are files"} — add your own
          there, or pick <strong>Custom</strong> to tweak the colors below and save them as a new one.
        </p>
        {openDirError && (
          <p className="settings-card__error" data-testid="open-themes-dir-error">
            {openDirError}
          </p>
        )}
        <select
          className="settings-select"
          value={themeSelectValue}
          onChange={(event) => handleThemeSelect(event.target.value)}
          data-testid="theme-select"
        >
          {!activeTheme && !isEditingCustomColors && (
            <option value={settings.activeThemeId}>{settings.activeThemeId}</option>
          )}
          {themes.map((theme) => (
            <option key={theme.id} value={theme.id}>
              {theme.name}
            </option>
          ))}
          <option value={CUSTOM_OPTION_ID}>Custom</option>
        </select>

        {isEditingCustomColors && (
          <>
            <div className="settings-custom-colors" data-testid="theme-colors">
              {CUSTOM_COLOR_FIELDS.map(({ field, label }) => (
                <label key={field} className="settings-custom-colors__field">
                  {label}
                  <input
                    type="color"
                    value={currentColors[field]}
                    onChange={(event) => handleColorChange(field, event.target.value)}
                    data-testid={`theme-color-${field}`}
                  />
                </label>
              ))}
            </div>

            <div className="settings-save-theme">
              <input
                type="text"
                className="settings-save-theme__input"
                placeholder="New theme name"
                value={newThemeName}
                onChange={(event) => setNewThemeName(event.target.value)}
                data-testid="new-theme-name"
              />
              <button
                type="button"
                className="button-ghost"
                onClick={() => void handleSaveTheme()}
                disabled={saving || !newThemeName.trim()}
                data-testid="save-theme"
              >
                Save as new theme
              </button>
            </div>
            {saveError && (
              <p className="settings-card__error" data-testid="save-theme-error">
                {saveError}
              </p>
            )}
          </>
        )}
      </section>

      <section className="settings-card">
        <h3>Startup</h3>
        <label className="settings-toggle">
          <input
            type="checkbox"
            checked={autostart}
            disabled={autostartBusy}
            onChange={() => void handleAutostartToggle()}
            data-testid="autostart-toggle"
          />
          Start at Login
        </label>
      </section>

      <section className="settings-card">
        <h3>External tools</h3>
        <p className="settings-card__hint">
          Satsuma bundles FFmpeg (video/audio) and, once vendored, pdfium (PDF page rendering) — point at your own
          copy instead if you'd rather not use the bundled one.
        </p>

        <div className="settings-tool-row">
          <label className="settings-tool-row__label" htmlFor="ffmpeg-path-input">
            FFmpeg
          </label>
          <div className="settings-tool-row__controls">
            <input
              id="ffmpeg-path-input"
              type="text"
              className="settings-save-theme__input"
              placeholder="Bundled (default)"
              value={draftFfmpegPath}
              onChange={(event) => setDraftFfmpegPath(event.target.value)}
              onBlur={handleFfmpegPathBlur}
              data-testid="ffmpeg-path-input"
            />
            <button type="button" className="button-ghost" onClick={() => void handleBrowseFfmpeg()} data-testid="ffmpeg-browse">
              Browse…
            </button>
            {settings.ffmpegPath && (
              <button type="button" className="button-ghost" onClick={handleResetFfmpegPath} data-testid="ffmpeg-reset">
                Reset
              </button>
            )}
          </div>
          <p className={ffmpegResolved ? "settings-card__hint" : "settings-card__error"} data-testid="ffmpeg-status">
            {ffmpegResolved ? <>Using <code>{ffmpegResolved}</code></> : "Not found — video/audio conversion is unavailable"}
          </p>
        </div>

        <div className="settings-tool-row">
          <label className="settings-tool-row__label" htmlFor="pdfium-path-input">
            pdfium
          </label>
          <div className="settings-tool-row__controls">
            <input
              id="pdfium-path-input"
              type="text"
              className="settings-save-theme__input"
              placeholder="Bundled (default)"
              value={draftPdfiumPath}
              onChange={(event) => setDraftPdfiumPath(event.target.value)}
              onBlur={handlePdfiumPathBlur}
              data-testid="pdfium-path-input"
            />
            <button type="button" className="button-ghost" onClick={() => void handleBrowsePdfium()} data-testid="pdfium-browse">
              Browse…
            </button>
            {settings.pdfiumPath && (
              <button type="button" className="button-ghost" onClick={handleResetPdfiumPath} data-testid="pdfium-reset">
                Reset
              </button>
            )}
          </div>
          <p className={pdfiumResolved ? "settings-card__hint" : "settings-card__error"} data-testid="pdfium-status">
            {pdfiumResolved ? <>Using <code>{pdfiumResolved}</code></> : "Not found — PDF page rendering is unavailable"}
          </p>
        </div>
      </section>

      <LinuxIntegrationPanel />
    </div>
  );
}
