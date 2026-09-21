import { invoke } from "@tauri-apps/api/core";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import type { CustomColors, FileCategory, HotkeyCombo, Settings, ThemeFile } from "../types";

/** Thin wrapper around the `detect_file_category` Tauri command, isolated so
 * components/hooks can be tested by mocking this module instead of the raw
 * Tauri API. */
export async function detectFileCategory(path: string): Promise<FileCategory> {
  return invoke<FileCategory>("detect_file_category", { path });
}

/** The OS this build is running on ("linux", "windows", "macos", ...). */
export async function currentPlatform(): Promise<string> {
  return invoke<string>("current_platform");
}

export async function isLinuxFileManagerIntegrationInstalled(): Promise<boolean> {
  return invoke<boolean>("is_linux_file_manager_integration_installed");
}

export async function installLinuxFileManagerIntegration(): Promise<void> {
  return invoke<void>("install_linux_file_manager_integration");
}

export async function uninstallLinuxFileManagerIntegration(): Promise<void> {
  return invoke<void>("uninstall_linux_file_manager_integration");
}

/** Hides the borderless overlay window (see OverlayApp), leaving it ready
 * for the next hotkey/context-menu trigger rather than closing it. */
export async function hideOverlay(): Promise<void> {
  return invoke<void>("hide_overlay");
}

/** Re-asserts the overlay window's visibility at the current cursor — the
 * page-side counterpart to `hideOverlay`, invoked by the overlay itself
 * once a `LaunchRequest` has actually opened the wedge menu. This is the
 * fix for the Windows single-instance forward race: the Rust-side show
 * that accompanies each trigger runs re-entrantly inside the WM_COPYDATA
 * wndproc for a forwarded request, where WebView2's first-show can
 * silently no-op and leave the window existing-but-hidden. An `invoke`
 * rides the normal IPC path instead, which is only processed once the
 * wndproc has returned — a dependable spot to re-assert "menu open ⇒
 * window visible". Idempotent: a direct launch's setup-time show has
 * already positioned/revealed the window by the time the page mounts, so
 * this is a harmless re-assertion there. See `show_overlay_window` in
 * `src-tauri/src/lib.rs`. */
export async function showOverlayWindow(): Promise<void> {
  return invoke<void>("show_overlay_window");
}

/** The target extensions every one of `paths` can be converted to —
 * backs the wedge menu's format list as of Phase 2, replacing the old
 * hardcoded `wedgeOptions` table. Empty for a directory, an unrecognized
 * format, or a selection with no common targets (see
 * `list_conversion_targets` in `src-tauri/src/lib.rs` for the exact
 * rules, mirroring Interaction.md's menu-content rules). */
export async function listConversionTargets(paths: string[]): Promise<string[]> {
  return invoke<string[]>("list_conversion_targets", { paths });
}

/** Converts `path` to `targetExtension`, writing a collision-safe sibling
 * file (or, for a multi-page PDF export, a sibling folder). Resolves with
 * the path actually written; rejects with a filename + error detail on
 * failure. The backend emits `conversion-progress` events (a `0..=100`
 * percent for the source path) while it runs — a progress UI will consume
 * them in a later phase. Replaces Phase 0's `stubConvertFile` byte-copy
 * with a real encode/transcode. */
export async function convertFile(path: string, targetExtension: string): Promise<string> {
  return invoke<string>("convert_file", { path, targetExtension });
}

/** Extracts an archive (`path` a ZIP/TAR/GZIP/RAR file) into a sibling
 * folder, resolving with that folder's path. Tools.md's "Extract Archive"
 * — a single action, not a format conversion. */
export async function extractArchive(path: string): Promise<string> {
  return invoke<string>("extract_archive", { path });
}

/** The persisted `Settings` (or the built-in defaults, on first run). */
export async function getSettings(): Promise<Settings> {
  return invoke<Settings>("get_settings");
}

/** Validates and persists `settings`, applying them live across every
 * window (main + overlay) via a `settings-changed` event — see
 * `save_settings` in `src-tauri/src/lib.rs`. Rejects (via the returned
 * promise) an invalid combination, e.g. an empty or colliding hotkey pair. */
export async function saveSettings(settings: Settings): Promise<void> {
  return invoke<void>("save_settings", { settings });
}

export async function isAutostartEnabled(): Promise<boolean> {
  return invoke<boolean>("is_autostart_enabled");
}

export async function setAutostartEnabled(enabled: boolean): Promise<void> {
  return invoke<void>("set_autostart_enabled", { enabled });
}

/** Every theme currently saved under `~/.config/satsuma/themes/` — the
 * backend seeds that directory with Satsuma's own built-ins the first time
 * it's asked for this list, so it's never empty on a fresh install. */
export async function listThemes(): Promise<ThemeFile[]> {
  return invoke<ThemeFile[]>("list_themes");
}

/** Saves `colors` as a new theme file named `name`, returning the theme as
 * actually saved (its `id` may differ from a plain slugification of `name`
 * if that id was already taken — see `save_theme` in `src-tauri/src/lib.rs`). */
export async function saveTheme(name: string, colors: CustomColors): Promise<ThemeFile> {
  return invoke<ThemeFile>("save_theme", { name, colors });
}

/** The themes directory's real, resolved absolute path (e.g.
 * `/home/alice/.config/satsuma/themes` on Linux,
 * `C:\Users\alice\AppData\Roaming\satsuma\themes` on Windows) — shown in
 * Settings so the path is always correct for whatever OS this build is
 * actually running on, rather than a hardcoded Linux-shaped guess. */
export async function themesDirDisplay(): Promise<string | null> {
  return invoke<string | null>("themes_dir_display");
}

/** Opens the themes directory in the OS's default file manager (Explorer,
 * Nautilus, Dolphin, ...) — see `open_themes_dir` in `src-tauri/src/lib.rs`.
 * Rejects (via the returned promise) if the directory couldn't be
 * resolved or the OS refused to open it. */
export async function openThemesDir(): Promise<void> {
  return invoke<void>("open_themes_dir");
}

/** The four modifier keys' actual current held state, plus at most one
 * other currently-held key from `lib/keys.ts`'s supported set, queried
 * directly from the OS — `null` when no such query is available on this
 * platform (a Linux Wayland session, or any other platform without one
 * implemented). Used by `HotkeyRecorder` while it's recording, polled
 * repeatedly on an interval; see `poll_held_modifiers` in
 * `src-tauri/src/lib.rs` for why a DOM-event reconstruction alone wasn't
 * reliable enough for this. Resolves to `null` (rather than rejecting) if
 * the command itself fails for any reason — e.g. no Tauri backend at all,
 * as in a plain browser during development — so callers can treat
 * "unavailable" uniformly. */
export async function pollHeldModifiers(): Promise<HotkeyCombo | null> {
  try {
    return (await invoke<HotkeyCombo | null>("poll_held_modifiers")) ?? null;
  } catch {
    return null;
  }
}

/** The FFmpeg binary path Satsuma would actually use right now, honoring
 * `Settings.ffmpegPath` if set — `null` if nothing usable was found (a
 * broken custom path, or a dev build with no bundled sidecar and no
 * system `ffmpeg`). Backs the "External tools" Settings section's live
 * status line — see `ffmpeg_status` in `src-tauri/src/lib.rs`. */
export async function ffmpegStatus(): Promise<string | null> {
  return invoke<string | null>("ffmpeg_status");
}

/** The pdfium library path Satsuma would actually use right now, honoring
 * `Settings.pdfiumPath` if set — `null` if nothing usable was found (most
 * builds today, until pdfium is vendored — see `pdfium_status` in
 * `src-tauri/src/lib.rs`). */
export async function pdfiumStatus(): Promise<string | null> {
  return invoke<string | null>("pdfium_status");
}

/** Opens the OS's native "pick a file" dialog (titled `title`), resolving
 * with the chosen file's absolute path, or `null` if the dialog was
 * cancelled — used by the "External tools" Settings section's "Browse…"
 * buttons instead of asking the user to type/paste a path by hand. */
export async function pickExecutableFile(title: string): Promise<string | null> {
  const selection = await openFileDialog({ title, multiple: false, directory: false });
  return selection ?? null;
}
