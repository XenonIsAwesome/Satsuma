export type FileCategory = "image" | "video" | "audio" | "document" | "archive" | "unknown";

export interface DroppedFile {
  path: string;
  name: string;
  category: FileCategory;
}

export interface WedgeOption {
  id: string;
  label: string;
  icon: string;
}

export type MenuMode = "formats" | "tools";

/** A hotkey binding — mirrors the Rust `HotkeyCombo` struct field-for-field
 * (serde `camelCase`): the four modifiers, tracked via the DOM's own
 * "Control"/"Alt"/"Shift"/"Meta" `KeyboardEvent.key` names, plus an
 * optional additional key using the DOM's `KeyboardEvent.code` naming
 * (e.g. `"KeyP"`, `"F5"`, `"Digit3"` — see `lib/keys.ts` for the supported
 * set). `null` for a modifiers-only binding, Phase 1's original shape. */
export interface HotkeyCombo {
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  meta: boolean;
  key: string | null;
}

/** The five user-set tokens every theme file defines, as `#rrggbb` strings
 * — see DESIGN.md's "Custom theme behavior", generalized from a
 * Custom-only mechanism to how every theme (built-in or user-made) is
 * defined once themes became data-driven files. */
export interface CustomColors {
  primary: string;
  secondary: string;
  neutral: string;
  surface: string;
  outline: string;
}

/** One theme file under `~/.config/satsuma/themes/`, as the
 * `list_themes`/`save_theme` Tauri commands return it — `id` is the
 * filename stem, derived server-side from `name` (see
 * `satsuma_core::slugify`). */
export interface ThemeFile {
  id: string;
  name: string;
  colors: CustomColors;
}

/** Mirrors the Rust `Settings` struct (serde `camelCase`) persisted via the
 * `get_settings`/`save_settings` Tauri commands. */
export interface Settings {
  formatModifiers: HotkeyCombo;
  toolsModifiers: HotkeyCombo;
  activeThemeId: string;
  colorOverrides: CustomColors | null;
  /** User-configured override path to the FFmpeg binary, or `null` to use
   * the bundled sidecar (falling back to a system `ffmpeg` on PATH) — see
   * `ffmpegStatus` for what's actually in effect right now. */
  ffmpegPath: string | null;
  /** User-configured override path to the pdfium shared library file, or
   * `null` to use the bundled resource if one was vendored — see
   * `pdfiumStatus` for what's actually in effect right now. */
  pdfiumPath: string | null;
}
