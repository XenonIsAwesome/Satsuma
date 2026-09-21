#![deny(missing_docs)]
//! Tauri command layer for Satsuma: window/tray/overlay lifecycle, settings
//! and theme persistence, and the platform-integration entry points
//! (`windows_integration` on Windows, `linux_integration` on Linux) that
//! deliver a [`satsuma_core::LaunchRequest`] into the app. Conversion and
//! detection logic itself lives in the platform-independent `satsuma-core`
//! crate, not here.

mod ffmpeg;
#[cfg(target_os = "linux")]
mod linux_integration;
mod pdfium;
mod settings_store;
#[cfg(target_os = "windows")]
mod single_instance_guard;
mod theme_store;
mod theme_watcher;
#[cfg(target_os = "windows")]
mod windows_integration;

use satsuma_core::{
    detect_category, overlay_position, parse_launch_args, validate_hotkeys, FileCategory, LaunchRequest,
    Settings, ThemeFile,
};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::menu::{CheckMenuItem, CheckMenuItemBuilder, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_autostart::ManagerExt;

/// Argv flag the autostart plugin launches Satsuma with at login, so a
/// silent background start doesn't flash the main window open.
const AUTOSTART_FLAG: &str = "--autostart";

const OVERLAY_WINDOW_LABEL: &str = "overlay";
const OVERLAY_SIZE: (i32, i32) = (480, 480);

/// Diagnostic-only: milliseconds since this process's first log line, so
/// the overlay-focus `eprintln!`s scattered across `lib.rs` and
/// `linux_integration.rs` can be correlated by relative timing, not just
/// by the order they happen to interleave in on stderr. Gated behind the
/// `debug-overlay-focus` feature (off by default) — see that feature's
/// doc comment in `Cargo.toml`.
#[cfg(feature = "debug-overlay-focus")]
static LOG_START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
#[cfg(feature = "debug-overlay-focus")]
pub(crate) fn log_ts() -> u128 {
    LOG_START.get_or_init(std::time::Instant::now).elapsed().as_millis()
}

/// Holds a `LaunchRequest` parsed from this process's own argv, if any,
/// until the frontend claims it once on startup via `take_launch_request`.
/// Later invocations (a second context-menu launch on an already-running
/// app) arrive instead as a `launch-request` event via the single-instance
/// plugin, since by then the frontend is already listening.
struct LaunchState(Mutex<Option<LaunchRequest>>);

/// The tray's "Start at Login" checkbox item, kept around so toggling
/// autostart from the Settings screen (`set_autostart_enabled`) can update
/// its checked state too — Phase 1 mirrors this setting into Settings
/// rather than moving it, so both surfaces should stay in sync without a
/// restart.
struct AutostartMenuItem(CheckMenuItem<tauri::Wry>);

/// Satsuma's own config directory — deliberately `~/.config/satsuma` (via
/// the OS's *base* config dir, not Tauri's identifier-derived
/// `app_config_dir`, which would otherwise nest everything under
/// `~/.config/io.github.xenonisawesome.satsuma`) so `settings.json` and the
/// `themes/` directory are somewhere a user would actually find and
/// hand-edit them.
fn config_base_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().config_dir().map(|dir| dir.join("satsuma")).map_err(|error| error.to_string())
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(settings_store::settings_file_path(&config_base_dir(app)?))
}

/// Loads persisted `Settings` without touching the themes directory or
/// correcting a dangling `active_theme_id` — unlike
/// `load_settings_and_themes`, which every Settings-screen-facing command
/// needs, this is for call sites that only care about the external-tool
/// override paths (`convert_file`, `ffmpeg_status`, `pdfium_status`) and
/// shouldn't pay for (or trigger side effects from) theme resolution on
/// every single conversion.
fn load_settings_only(app: &AppHandle) -> Settings {
    settings_path(app).ok().as_deref().map(settings_store::load_settings_from).unwrap_or_default()
}

fn themes_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(theme_store::themes_dir(&config_base_dir(app)?))
}

/// The themes directory's real, resolved absolute path as a display
/// string (e.g. `/home/alice/.config/satsuma/themes` on Linux,
/// `C:\Users\alice\AppData\Roaming\satsuma\themes` on Windows) — so
/// Settings can tell the user exactly where to find/add theme files
/// without the frontend having to guess at platform-specific conventions
/// (which would otherwise show a Linux-shaped path on Windows). `None` only
/// if the OS config directory itself couldn't be resolved.
#[tauri::command]
fn themes_dir_display(app: AppHandle) -> Option<String> {
    themes_path(&app).ok().map(|path| display_path(&path))
}

/// Opens the themes directory in the OS's default file manager, so a user
/// can jump straight to hand-editing or adding theme files from Settings
/// instead of having to copy the path `themes_dir_display` shows and
/// navigate there themselves. Calls the opener plugin's Rust API directly
/// rather than exposing its `opener:allow-open-path` permission to the
/// webview — the path here is always this fixed, server-resolved
/// directory, never something arbitrary from JS.
#[tauri::command]
fn open_themes_dir(app: AppHandle) -> Result<(), String> {
    let dir = themes_path(&app)?;
    tauri_plugin_opener::open_path(dir, None::<&str>).map_err(|error| error.to_string())
}

/// A theme file's id alongside its contents, flattened for the frontend
/// (which otherwise has no use for the id/`ThemeFile` split that lets
/// `theme_store` derive an id from a filename without storing it twice).
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ThemeFileDto {
    id: String,
    name: String,
    colors: satsuma_core::CustomColors,
}

impl From<(String, ThemeFile)> for ThemeFileDto {
    fn from((id, theme): (String, ThemeFile)) -> Self {
        Self { id, name: theme.name, colors: theme.colors }
    }
}

/// Loads the persisted `Settings` and the current theme list together,
/// making sure `active_theme_id` still refers to an existing theme file —
/// falling back to Citrus (recreating its file first if that's the one
/// that's missing, see `theme_store::ensure_citrus_seeded`) and persisting
/// the correction if not. Every caller needs both the settings and the
/// theme list anyway (to resolve one against the other), and the "was a
/// correction made" bool lets callers that also broadcast events (the
/// theme-directory watcher) skip a spurious `settings-changed` when
/// nothing about Settings actually changed.
fn load_settings_and_themes(app: &AppHandle) -> (Settings, bool, Vec<ThemeFileDto>) {
    let themes_dir = themes_path(app).ok();
    if let Some(dir) = &themes_dir {
        let _ = theme_store::ensure_seeded(dir);
        let _ = theme_store::ensure_citrus_seeded(dir);
    }
    let listed = themes_dir.as_deref().map(theme_store::list_themes).unwrap_or_default();

    let settings_file = settings_path(app).ok();
    let mut settings =
        settings_file.as_deref().map(settings_store::load_settings_from).unwrap_or_default();

    let mut corrected = false;
    if !theme_store::theme_exists(&listed, &settings.active_theme_id) {
        settings.active_theme_id = satsuma_core::DEFAULT_THEME_ID.to_string();
        settings.color_overrides = None;
        corrected = true;
        if let Some(path) = &settings_file {
            let _ = settings_store::save_settings_to(path, &settings);
        }
    }

    let dtos = listed.into_iter().map(ThemeFileDto::from).collect();
    (settings, corrected, dtos)
}

/// Re-derives settings/themes after the themes directory changed outside
/// the app (see `theme_watcher::watch`) and broadcasts whatever actually
/// changed: the fresh theme list always, and a corrected `Settings` (the
/// active theme having disappeared, per `load_settings_and_themes`) only
/// when that correction actually happened.
fn reconcile_themes_and_notify(app: &AppHandle) {
    let (settings, corrected, themes) = load_settings_and_themes(app);
    let _ = app.emit("themes-changed", &themes);
    if corrected {
        let _ = app.emit("settings-changed", &settings);
    }
}

/// Returns the persisted `Settings`, or `Settings::default()` if none have
/// been saved yet (first run) or the file couldn't be read/parsed —
/// falling back to Citrus if the persisted active theme no longer exists
/// (see `load_settings_and_themes`).
#[tauri::command]
fn get_settings(app: AppHandle) -> Settings {
    load_settings_and_themes(&app).0
}

/// Validates and persists `settings`, then applies them live: broadcasts a
/// `settings-changed` event to every window (so the main window's and the
/// overlay's own theme/modifier state update without a restart) and, on
/// Windows, updates the global keyboard hook's live-tracked combinations.
#[tauri::command]
fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    validate_hotkeys(&settings.format_modifiers, &settings.tools_modifiers)?;

    let path = settings_path(&app)?;
    settings_store::save_settings_to(&path, &settings).map_err(|error| error.to_string())?;

    #[cfg(target_os = "windows")]
    windows_integration::set_modifiers(settings.format_modifiers.clone(), settings.tools_modifiers.clone());

    let _ = app.emit("settings-changed", &settings);
    Ok(())
}

/// Every theme currently saved under `~/.config/satsuma/themes/`, seeding
/// that directory with Satsuma's own built-ins first if it doesn't exist
/// yet, and recovering a deleted Citrus specifically either way (see
/// `load_settings_and_themes`).
#[tauri::command]
fn list_themes(app: AppHandle) -> Vec<ThemeFileDto> {
    load_settings_and_themes(&app).2
}

/// Saves `colors` as a new theme file named `name`, returning the saved
/// theme (including the id it was actually given — see
/// `theme_store::save_theme` for the collision-safe naming rule).
#[tauri::command]
fn save_theme(app: AppHandle, name: String, colors: satsuma_core::CustomColors) -> Result<ThemeFileDto, String> {
    let dir = themes_path(&app)?;
    let theme = ThemeFile { name, colors };
    let id = theme_store::save_theme(&dir, &theme).map_err(|error| error.to_string())?;
    Ok(ThemeFileDto::from((id, theme)))
}

/// Whether Satsuma is currently set to launch at login — mirrors the tray
/// menu's own "Start at Login" checkbox so the Settings screen can show and
/// toggle the same underlying state.
#[tauri::command]
fn is_autostart_enabled(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
fn set_autostart_enabled(app: AppHandle, enabled: bool) -> Result<(), String> {
    let manager = app.autolaunch();
    let result = if enabled { manager.enable() } else { manager.disable() };
    result.map_err(|error| error.to_string())?;
    if let Some(item) = app.try_state::<AutostartMenuItem>() {
        let _ = item.0.set_checked(enabled);
    }
    Ok(())
}

/// The actual current held state — the four modifiers, plus at most one
/// other currently-held key from `satsuma_core::keys`' supported set —
/// queried directly from the OS rather than reconstructed from DOM
/// keyboard events. Polled repeatedly by the Settings screen's hotkey
/// recorder while it's recording (see `pollHeldModifiers` in
/// `lib/tauri.ts`).
///
/// This exists because reconstructing state from individual keydown/keyup
/// events in the frontend (tried first) turned out to still be unreliable
/// in practice for a real Shift+Alt press: Alt in particular is a
/// menu-accelerator/mnemonic key that some native window
/// toolkits/webviews can intercept before its own keydown/keyup ever
/// reaches the page's JS at all. Querying the OS directly —
/// `GetAsyncKeyState` on Windows, `XQueryKeymap`'s per-keycode bitmap on
/// Linux/X11 — isn't subject to that interception. `None` when no such
/// query is available (a Linux Wayland session, or any other platform), in
/// which case the frontend falls back to plain DOM keyboard events.
///
/// Only feeds the Settings screen's *recorder* — a recorded binding that
/// includes an extra key still won't trigger Windows' separate
/// Explorer-drag global keyboard hook, which only ever matches on the four
/// modifiers (see `HotkeyCombo`'s own doc comment for why).
#[tauri::command]
fn poll_held_modifiers() -> Option<satsuma_core::HotkeyCombo> {
    #[cfg(target_os = "windows")]
    {
        Some(windows_integration::poll_held_modifiers())
    }
    #[cfg(target_os = "linux")]
    {
        linux_integration::poll_held_modifiers()
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        None
    }
}

/// Classifies a dropped file's path into an image/video/audio/unknown
/// category so the frontend knows which wedge menu options to show.
/// Delegates to `satsuma-core`, kept independent of the Tauri command layer.
#[tauri::command]
fn detect_file_category(path: String) -> FileCategory {
    detect_category(&path)
}

/// Returns (and clears) the `LaunchRequest` this process was started with,
/// e.g. from a Linux file-manager "Convert with Satsuma" context-menu entry.
/// Returns `None` for a normal launch with no associated file.
#[tauri::command]
fn take_launch_request(state: tauri::State<LaunchState>) -> Option<LaunchRequest> {
    state.0.lock().unwrap().take()
}

/// The current OS, so the frontend can show OS-specific UI (e.g. the Linux
/// file-manager integration toggle only makes sense on Linux).
#[tauri::command]
fn current_platform() -> &'static str {
    std::env::consts::OS
}

/// Hides the borderless overlay window. Called by the overlay's own
/// frontend once a wedge is selected or the menu is cancelled, so the
/// window is ready (but hidden) for the next trigger rather than destroyed
/// and recreated each time.
#[tauri::command]
fn hide_overlay(app: AppHandle) {
    #[cfg(feature = "debug-overlay-focus")]
    eprintln!("[satsuma +{}ms] hide_overlay invoked", log_ts());
    #[cfg(target_os = "linux")]
    linux_integration::mark_overlay_visible(false);
    if let Some(overlay) = app.get_webview_window(OVERLAY_WINDOW_LABEL) {
        let _ = overlay.hide();
    }
}

/// Makes the (already-created) overlay window visible at the current cursor
/// again — the frontend-facing counterpart to `hide_overlay`, called by the
/// overlay page itself once a `LaunchRequest` has actually opened the wedge
/// menu. This enforces "menu open ⇒ window visible" from the side that
/// knows the page is definitively mounted, because the Rust-side show that
/// accompanies each trigger (see `show_overlay`) is unreliable on Windows
/// for a *forwarded* request: the single-instance plugin delivers it inside
/// a WM_COPYDATA wndproc on the main thread, so `show_overlay`'s
/// `set_position`/`show` run re-entrantly inside that wndproc where the
/// WebView2 first-show can silently no-op (e2e-windows runs saw the overlay
/// HWND exist but stay hidden after a forward, at both its creation-cascade
/// and cursor-derived positions). An `invoke` from the page rides the normal
/// IPC path instead, which is only processed once the wndproc has returned
/// — a dependable spot to re-assert visibility. Idempotent: a direct
/// launch's setup-time show has already positioned/revealed the window by
/// the time the page mounts, so this is a harmless re-assertion there.
#[tauri::command]
fn show_overlay_window(app: AppHandle) {
    show_overlay(&app);
}

/// Payload for the `conversion-progress` event `convert_file` emits while
/// it runs. Per Interaction.md's progress/completion contract, a real
/// percentage is only ever emitted when the engine can report one (FFmpeg
/// encode timestamps against known duration) — the frontend treats "no
/// event yet for this path" as an indeterminate spinner rather than 0%.
#[derive(Clone, serde::Serialize)]
struct ConversionProgress {
    /// The source file path this progress update is for (so a frontend
    /// converting several files at once can tell them apart).
    path: String,
    /// `0.0..=100.0`.
    percent: f32,
}

/// The target extensions every one of `paths` can be converted to — backs
/// the wedge menu's format list (`data/wedgeOptions.ts` on the frontend),
/// replacing Phase 0's hardcoded table. Empty for a directory, a
/// nonexistent path, an unrecognized format, an empty `paths`, or (for
/// more than one path) a selection with no common targets — the frontend
/// shows no menu in all of those cases, per Interaction.md's "unsupported
/// inputs are rejected outright" / "mixed unrelated families show
/// nothing" rules.
#[tauri::command]
fn list_conversion_targets(paths: Vec<String>) -> Vec<String> {
    if paths.is_empty() || paths.iter().any(|path| std::path::Path::new(path).is_dir()) {
        return Vec::new();
    }
    satsuma_core::supported_targets_for_selection(&paths)
        .into_iter()
        .map(str::to_string)
        .collect()
}

/// The actual conversion work behind `convert_file`, taking a plain
/// progress callback rather than an `AppHandle` so it's unit-testable
/// without a running Tauri app — the command below is a thin shim wiring
/// `AppHandle::emit` into `on_progress`.
fn convert_file_blocking(
    path: &str,
    target_extension: &str,
    ffmpeg_bin: Option<&std::path::Path>,
    pdfium_lib_dir: Option<&std::path::Path>,
    on_progress: &mut dyn FnMut(f32),
) -> Result<String, String> {
    let source = std::path::Path::new(path);
    satsuma_core::convert(source, target_extension, ffmpeg_bin, pdfium_lib_dir, on_progress)
        .map(|destination| destination.to_string_lossy().into_owned())
        .map_err(|error| format!("{path}: {error}"))
}

/// Converts `path` to `target_extension`, writing a collision-safe sibling
/// file (or, for a multi-page PDF export, a sibling folder — see
/// `satsuma_core::convert::document`) and emitting `conversion-progress`
/// events as the engine reports real progress. Resolves with the path
/// actually written. Replaces Phase 0's `stub_convert_file` byte-copy with
/// a real encode/transcode, dispatched through `satsuma_core::convert`.
/// FFmpeg/pdfium paths honor the user's Settings overrides, if any — see
/// `ffmpeg::resolve_ffmpeg_bin`/`pdfium::resolve_pdfium_lib_dir`.
#[tauri::command]
async fn convert_file(app: AppHandle, path: String, target_extension: String) -> Result<String, String> {
    let settings = load_settings_only(&app);
    let ffmpeg_bin = ffmpeg::resolve_ffmpeg_bin(&app, settings.ffmpeg_path.as_deref());
    let pdfium_lib_dir = pdfium::resolve_pdfium_lib_dir(&app, settings.pdfium_path.as_deref());
    tauri::async_runtime::spawn_blocking(move || {
        convert_file_blocking(&path, &target_extension, ffmpeg_bin.as_deref(), pdfium_lib_dir.as_deref(), &mut |fraction| {
            let _ = app.emit(
                "conversion-progress",
                ConversionProgress {
                    path: path.clone(),
                    percent: fraction * 100.0,
                },
            );
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Renders `path` for the UI without Windows' `\\?\` verbatim-path
/// prefix. On Windows some resolved paths (resource-directory
/// paths in particular) come back from the OS with that prefix;
/// it is valid as a filesystem path but is noisy and ugly in the
/// Settings → External tools status lines. This strips it for
/// display only — it is never passed to a spawn, so the prefix's
/// long-path semantics are preserved where they actually matter.
/// UNC paths are restored to the canonical `\\server\share` shape
/// rather than the bare `server\share` that a naive prefix strip
/// would leave.
fn display_path(path: &Path) -> String {
    let displayed = path.to_string_lossy();
    match displayed.strip_prefix(r"\\?\") {
        Some(rest) if rest.starts_with("UNC\\") => {
            format!(r"\\{}", &rest["UNC\\".len()..])
        }
        Some(rest) => rest.to_string(),
        None => displayed.into_owned(),
    }
}

/// The FFmpeg binary path Satsuma would actually use right now (honoring
/// `Settings::ffmpeg_path` if set), or `None` if nothing usable was found
/// — backs the "External tools" Settings section's live status line, so a
/// broken custom path (or a dev build with no sidecar and no system
/// `ffmpeg`) is visible before it ever causes a conversion to fail.
#[tauri::command]
fn ffmpeg_status(app: AppHandle) -> Option<String> {
    let settings = load_settings_only(&app);
    ffmpeg::resolve_ffmpeg_bin(&app, settings.ffmpeg_path.as_deref()).map(|path| display_path(&path))
}

/// The pdfium library file path Satsuma would actually use right now
/// (honoring `Settings::pdfium_path` if set), or `None` if nothing usable
/// was found — backs the "External tools" Settings section's live status
/// line, same as `ffmpeg_status`. Returns the library *file* path (not
/// just its directory, which is what `resolve_pdfium_lib_dir` returns for
/// `satsuma_core::convert`'s own use) so the displayed status mirrors
/// exactly what the user typed/picked for `ffmpeg_status` consistency.
#[tauri::command]
fn pdfium_status(app: AppHandle) -> Option<String> {
    let settings = load_settings_only(&app);
    let lib_dir = pdfium::resolve_pdfium_lib_dir(&app, settings.pdfium_path.as_deref())?;
    Some(display_path(&lib_dir.join(pdfium::library_file_name())))
}

/// Extracts an archive (`path` a ZIP/TAR/GZIP/RAR file) into a sibling
/// folder, returning that folder's path. Tools.md's "Extract Archive" is a
/// single action with no format target to pick, so it's a separate
/// command rather than going through `convert_file`. Runs on a blocking
/// thread-pool like `convert_file` — extraction reads + decompresses +
/// writes every entry, which must not freeze the main window, tray, or the
/// borderless overlay (whose Escape handler stays responsive) on a slow
/// archive.
#[tauri::command]
async fn extract_archive(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || extract_archive_blocking(&path))
        .await
        .map_err(|error| error.to_string())?
}

/// The actual extraction work behind `extract_archive`, taking a plain
/// path string so it's unit-testable without a running Tauri app — the
/// command above is a thin shim offloading it from the main thread.
fn extract_archive_blocking(path: &str) -> Result<String, String> {
    let source = std::path::Path::new(path);
    satsuma_core::convert::archive::extract(source)
        .map(|destination| destination.to_string_lossy().into_owned())
        .map_err(|error| format!("{path}: {error}"))
}

/// Installs the Linux file-manager context-menu integration (Nautilus
/// scripts, Nemo actions, KDE Dolphin service menu, `.desktop` MIME
/// association) under the current user's XDG data dirs — backs the
/// in-app "file-manager integration" Settings toggle. A no-op error on
/// every other OS (Windows uses the global Shift-hook instead, see
/// `windows_integration.rs`), and fails if the XDG data home itself
/// can't be resolved rather than guessing a path.
#[tauri::command]
fn install_linux_file_manager_integration() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let paths = linux_integration::default_paths()
            .ok_or_else(|| "could not determine the XDG data home directory".to_string())?;
        let exe = std::env::current_exe().map_err(|error| error.to_string())?;
        linux_integration::install(&paths, &exe).map_err(|error| error.to_string())
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err("file-manager integration is only available on Linux".to_string())
    }
}

/// Reverses [`install_linux_file_manager_integration`] — removes every
/// file it installed. Also a no-op error on non-Linux OSes.
#[tauri::command]
fn uninstall_linux_file_manager_integration() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let paths = linux_integration::default_paths()
            .ok_or_else(|| "could not determine the XDG data home directory".to_string())?;
        linux_integration::uninstall(&paths).map_err(|error| error.to_string())
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err("file-manager integration is only available on Linux".to_string())
    }
}

/// Whether the Linux file-manager integration is currently installed —
/// backs the Settings toggle's initial state. Always `false` on non-Linux
/// OSes and if the XDG data home can't be resolved (rather than erroring,
/// since this is a status query, not an action).
#[tauri::command]
fn is_linux_file_manager_integration_installed() -> bool {
    #[cfg(target_os = "linux")]
    {
        linux_integration::default_paths()
            .map(|paths| linux_integration::is_installed(&paths))
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Best-effort current cursor position, used to place the overlay window
/// near where the user actually is. `None` falls back to monitor-center
/// placement (e.g. on a Wayland session, where a global pointer query
/// generally isn't available to ordinary clients).
fn query_cursor_position() -> Option<(i32, i32)> {
    #[cfg(target_os = "windows")]
    {
        windows_integration::cursor_position()
    }
    #[cfg(target_os = "linux")]
    {
        linux_integration::cursor_position()
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        None
    }
}

/// The center of the primary monitor, in screen coordinates — the fallback
/// placement for the overlay window when the real cursor position isn't
/// available.
fn primary_monitor_center(app: &AppHandle) -> (i32, i32) {
    app.get_webview_window(OVERLAY_WINDOW_LABEL)
        .and_then(|window| window.primary_monitor().ok().flatten())
        .map(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            (position.x + size.width as i32 / 2, position.y + size.height as i32 / 2)
        })
        .unwrap_or((0, 0))
}

/// Positions the (already-created, hidden) overlay window near the current
/// cursor and shows it, ready for the frontend to render the wedge menu for
/// whatever `LaunchRequest` accompanies this trigger.
pub(crate) fn show_overlay(app: &AppHandle) {
    #[cfg(feature = "debug-overlay-focus")]
    eprintln!("[satsuma +{}ms] show_overlay: called", log_ts());
    let Some(overlay) = app.get_webview_window(OVERLAY_WINDOW_LABEL) else { return };
    let cursor = query_cursor_position().unwrap_or_else(|| primary_monitor_center(app));
    let (x, y) = overlay_position(None, cursor, OVERLAY_SIZE);
    let _ = overlay.set_position(tauri::PhysicalPosition::new(x, y));
    let _ = overlay.show();

    #[cfg(target_os = "linux")]
    linux_integration::mark_overlay_visible(true);

    // On Linux, `set_focus()`'s own untimestamped activation request and
    // `force_focus_overlay_linux`'s properly-timestamped one are two
    // separate, independent attempts to activate the very same window —
    // firing both back-to-back turned out to sometimes cause a spurious
    // focus-then-immediate-defocus (a `Focused(true)` followed right away
    // by an unprompted `Focused(false)`, with no `hide_overlay` call in
    // between, seen on real hardware), most likely from confusing the
    // window manager's focus-stealing-prevention bookkeeping. Use exactly
    // one activation path per platform instead of layering them.
    #[cfg(target_os = "linux")]
    force_focus_overlay_linux(&overlay);
    #[cfg(not(target_os = "linux"))]
    let _ = overlay.set_focus();
}

/// Extracts a window's raw X11 window ID from its `raw-window-handle`, or
/// `None` if the handle isn't an X11 (Xlib) one for any reason (e.g. a
/// Wayland session, where raw-window-handle would report a different
/// variant entirely).
#[cfg(target_os = "linux")]
fn x11_window_id<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) -> Option<u32> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = window.window_handle().ok()?;
    let RawWindowHandle::Xlib(xlib) = handle.as_raw() else { return None };
    Some(xlib.window as u32)
}

/// Asks the window manager to actually activate the overlay, working
/// around `set_focus()`'s unreliable untimestamped focus request on Linux.
/// A no-op (silently) if the raw handle isn't an X11 one for any reason.
#[cfg(target_os = "linux")]
fn force_focus_overlay_linux<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) {
    let Some(xid) = x11_window_id(window) else { return };

    // `force_activate_window` does a blocking X11 round-trip (to obtain a
    // real timestamp for the activation request) — this function runs on
    // Tauri's main GTK event-loop thread, and blocking that thread would
    // freeze the whole app until the round-trip completes (or, worse,
    // indefinitely if it never does), the exact class of bug
    // `windows_integration.rs`'s `spawn_shift_handler` exists to avoid on
    // Windows. `xid` is a plain `u32`, so it's cheap and safe to hand to a
    // dedicated thread rather than sending the window handle itself across.
    #[cfg(feature = "debug-overlay-focus")]
    eprintln!("[satsuma +{}ms] force_focus_overlay_linux: spawning activation thread (xid={xid})", log_ts());
    std::thread::spawn(move || {
        linux_integration::force_activate_window(xid);
    });
}

/// Shows the overlay for a `LaunchRequest`'s category, or hides it (in case
/// a previous trigger left it open) when the category is `Unknown` — an
/// unsupported file type has no wedge options to show at all, so the
/// overlay must never become a blank, click-catching window for it. See the
/// doc comment on `LaunchRequest::category` in `satsuma-core` for why this
/// category is computed Rust-side rather than left to an async frontend call.
fn show_overlay_for_request(app: &AppHandle, request: &LaunchRequest) {
    if request.category == FileCategory::Unknown {
        if let Some(overlay) = app.get_webview_window(OVERLAY_WINDOW_LABEL) {
            let _ = overlay.hide();
        }
    } else {
        show_overlay(app);
    }
}

/// Builds the tray icon and its menu (Show Satsuma / Start at Login / Quit)
/// so the app remains reachable once its window is closed — closing the
/// window hides it rather than exiting, see the `CloseRequested` handler in
/// `run()` below.
fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let show_item = MenuItem::with_id(app, "show", "Show Satsuma", true, None::<&str>)?;
    let autostart_enabled = app.autolaunch().is_enabled().unwrap_or(false);
    let autostart_item =
        CheckMenuItemBuilder::with_id("autostart", "Start at Login").checked(autostart_enabled).build(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&show_item, &autostart_item, &PredefinedMenuItem::separator(app)?, &quit_item],
    )?;

    app.manage(AutostartMenuItem(autostart_item));

    let mut tray = TrayIconBuilder::new().menu(&menu).show_menu_on_left_click(true);
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.on_menu_event(|app, event| match event.id().as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "autostart" => {
                let manager = app.autolaunch();
                let enabled = manager.is_enabled().unwrap_or(false);
                let _ = set_autostart_enabled(app.clone(), !enabled);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

/// App entry point: builds and runs the Tauri application (window/tray
/// setup, plugin registration, command handlers, single-instance argv
/// forwarding). Called from `main.rs`.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let context = tauri::generate_context!();

    // The single-instance plugin's Windows setup has an unfixed upstream
    // race (plugins-workspace #3587) that lets a second process boot as a
    // full second instance when its `FindWindowW` misses the primary's
    // IPC window; this guard must run before *anything* Tauri does.
    #[cfg(target_os = "windows")]
    single_instance_guard::enforce(&context.config().identifier);

    let args: Vec<String> = std::env::args().collect();
    let launched_via_autostart = args.iter().any(|arg| arg == AUTOSTART_FLAG);
    let initial_request = parse_launch_args(args.into_iter().skip(1).filter(|arg| arg != AUTOSTART_FLAG));

    let mut builder = tauri::Builder::default();

    // Must be registered before other plugins/setup: it re-exercises this
    // same `run()` in a fresh process only long enough to forward argv to
    // the already-running instance, then exits.
    builder = builder.plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
        let request = parse_launch_args(argv.into_iter().skip(1).filter(|arg| arg != AUTOSTART_FLAG));
        if let Some(request) = request {
            // Store it in LaunchState *before* emitting, not just the
            // event alone: the "overlay" window is created once at
            // startup with `visible: false` and (at least on Windows,
            // where WebView2 defers its own content loading until a
            // hidden window is first shown) its page may not have
            // mounted `useLaunchRequest`'s event listener yet by the time
            // `show_overlay_for_request` below makes this the window's
            // first-ever `show()` — an event emitted into a page that
            // hasn't registered its listener yet is simply lost, with no
            // replay. Populating LaunchState means the frontend's other,
            // pull-based path (`take_launch_request`, called once on
            // mount) still finds this request once the page does mount,
            // exactly like a direct-launch's `initial_request` already
            // works, regardless of which of the two arrives first.
            *app.state::<LaunchState>().0.lock().unwrap() = Some(request.clone());
            show_overlay_for_request(app, &request);
            let _ = app.emit("launch-request", request);
        } else if let Some(window) = app.get_webview_window("main") {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }));

    builder = builder.plugin(tauri_plugin_autostart::init(
        tauri_plugin_autostart::MacosLauncher::LaunchAgent,
        Some(vec![AUTOSTART_FLAG]),
    ));

    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(LaunchState(Mutex::new(initial_request.clone())))
        .invoke_handler(tauri::generate_handler![
            detect_file_category,
            take_launch_request,
            current_platform,
            hide_overlay,
            show_overlay_window,
            list_conversion_targets,
            convert_file,
            extract_archive,
            ffmpeg_status,
            pdfium_status,
            install_linux_file_manager_integration,
            uninstall_linux_file_manager_integration,
            is_linux_file_manager_integration_installed,
            get_settings,
            save_settings,
            is_autostart_enabled,
            set_autostart_enabled,
            list_themes,
            save_theme,
            themes_dir_display,
            open_themes_dir,
            poll_held_modifiers,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            setup_tray(&handle)?;

            // Loaded once up front so the Windows hook (below) starts out
            // watching whatever combinations were last saved, rather than
            // always the hardcoded defaults until the frontend happens to
            // call `save_settings` again.
            let initial_settings = get_settings(handle.clone());

            #[cfg(target_os = "windows")]
            windows_integration::spawn_shift_watcher(
                handle.clone(),
                initial_settings.format_modifiers,
                initial_settings.tools_modifiers,
            );
            #[cfg(not(target_os = "windows"))]
            let _ = initial_settings;

            // Watches the themes directory so an external edit/add/delete
            // (a user hand-editing a theme file, or deleting the active
            // one) is picked up live — see `reconcile_themes_and_notify`.
            // `get_settings` above already ensured this directory exists.
            if let Ok(dir) = themes_path(&handle) {
                let watcher_app = handle.clone();
                theme_watcher::watch(dir, move || {
                    reconcile_themes_and_notify(&watcher_app);
                });
            }

            if let Some(main_window) = app.get_webview_window("main") {
                let main_for_close = main_window.clone();
                main_window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = main_for_close.hide();
                    }
                });
            }

            // `overlay_window` is only actually used under some cfg
            // combinations (the `debug-overlay-focus` feature and/or
            // `target_os = "linux"`) — harmlessly unused otherwise, e.g. a
            // plain Windows build without that feature.
            #[allow(unused_variables)]
            if let Some(overlay_window) = app.get_webview_window(OVERLAY_WINDOW_LABEL) {
                // Purely diagnostic — see the doc comment on
                // `linux_integration::spawn_active_window_watcher` for why
                // hiding on `Focused(false)` was dropped in favor of that:
                // Mutter/Muffin's focus-stealing-prevention can silently
                // decline our own activation request, so this window
                // sometimes never has "focus" to lose in the first place,
                // and — separately — the reverse also happened on real
                // hardware (an unprompted `Focused(false)` with no user
                // action at all), making it an unreliable signal either way.
                #[cfg(feature = "debug-overlay-focus")]
                overlay_window.on_window_event(move |event| {
                    eprintln!("[satsuma +{}ms] overlay window event: {event:?}", log_ts());
                });

                #[cfg(target_os = "linux")]
                if let Some(xid) = x11_window_id(&overlay_window) {
                    linux_integration::spawn_active_window_watcher(handle.clone(), xid);
                }
            }

            // A launch request at process start (a Linux context-menu
            // invocation, or Windows argv forwarding) is handled by the
            // overlay, not the main window — its frontend claims the
            // request itself via `take_launch_request` once mounted, so we
            // only need to position and reveal the window here.
            if let Some(request) = &initial_request {
                show_overlay_for_request(&handle, request);
            } else if !launched_via_autostart {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                }
            }

            Ok(())
        })
        .run(context)
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_file_category_delegates_to_core() {
        assert_eq!(detect_file_category("photo.png".to_string()), FileCategory::Image);
        assert_eq!(detect_file_category("clip.mp4".to_string()), FileCategory::Video);
        assert_eq!(detect_file_category("song.flac".to_string()), FileCategory::Audio);
        assert_eq!(detect_file_category("archive.zip".to_string()), FileCategory::Archive);
        assert_eq!(detect_file_category("report.pdf".to_string()), FileCategory::Document);
        assert_eq!(detect_file_category("presentation.pptx".to_string()), FileCategory::Unknown);
    }

    #[test]
    fn current_platform_matches_the_build_target() {
        assert_eq!(current_platform(), std::env::consts::OS);
    }

    #[test]
    fn list_conversion_targets_is_empty_for_a_directory() {
        let dir = std::env::temp_dir().join(format!("satsuma-lib-test-dir-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        assert_eq!(list_conversion_targets(vec![dir.to_str().unwrap().to_string()]), Vec::<String>::new());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn list_conversion_targets_is_empty_for_an_empty_selection() {
        assert_eq!(list_conversion_targets(vec![]), Vec::<String>::new());
    }

    #[test]
    fn list_conversion_targets_excludes_the_sources_own_format() {
        let targets = list_conversion_targets(vec!["song.mp3".to_string()]);
        assert!(!targets.contains(&"mp3".to_string()));
        assert!(targets.contains(&"wav".to_string()));
    }

    #[test]
    fn list_conversion_targets_intersects_a_mixed_selection() {
        // jpg and png share every raster target plus pdf/docx, minus each
        // other (each is excluded from its own list) - the intersection
        // should still be non-empty and contain neither jpg nor png.
        let targets = list_conversion_targets(vec!["a.jpg".to_string(), "b.png".to_string()]);
        assert!(!targets.is_empty());
        assert!(!targets.contains(&"jpg".to_string()));
        assert!(!targets.contains(&"png".to_string()));
    }

    #[test]
    fn list_conversion_targets_is_empty_for_unrelated_families() {
        let targets = list_conversion_targets(vec!["a.jpg".to_string(), "b.mp4".to_string()]);
        assert_eq!(targets, Vec::<String>::new());
    }

    #[test]
    fn extract_archive_rejects_a_missing_source() {
        let result = extract_archive_blocking("/no/such/file.zip");
        assert!(result.is_err());
    }

    #[test]
    fn convert_file_writes_a_real_output_and_reports_progress() {
        let dir = std::env::temp_dir().join(format!("satsuma-lib-convert-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("pixel.bmp");
        // A minimal valid 1x1 BMP, small enough to inline: the `image`
        // crate (used by satsuma-core's image family) can decode/encode
        // this without needing a real photo fixture.
        let bmp: &[u8] = &[
            0x42, 0x4D, 0x3A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x36, 0x00, 0x00, 0x00, 0x28, 0x00, 0x00,
            0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x04, 0x00, 0x00, 0x00, 0x13, 0x0B, 0x00, 0x00, 0x13, 0x0B, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00,
        ];
        std::fs::write(&source, bmp).unwrap();

        let mut progress_calls = Vec::new();
        let result =
            convert_file_blocking(source.to_str().unwrap(), "png", None, None, &mut |fraction| progress_calls.push(fraction));

        let destination = result.expect("bmp -> png should succeed");
        assert_eq!(destination, dir.join("pixel.png").to_str().unwrap());
        assert!(std::path::Path::new(&destination).is_file());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn convert_file_rejects_a_missing_source() {
        let result = convert_file_blocking("/no/such/file.jpg", "png", None, None, &mut |_| {});
        assert!(result.is_err());
    }

    #[test]
    fn display_path_strips_windows_verbatim_prefix() {
        assert_eq!(
            display_path(std::path::Path::new(r"\\?\C:\Program Files\Satsuma\resources\ffmpeg\ffmpeg.exe")),
            r"C:\Program Files\Satsuma\resources\ffmpeg\ffmpeg.exe"
        );
    }

    #[test]
    fn display_path_normalizes_verbatim_unc_paths() {
        assert_eq!(
            display_path(std::path::Path::new(r"\\?\UNC\fileserver\share\ffmpeg.exe")),
            r"\\fileserver\share\ffmpeg.exe"
        );
    }

    #[test]
    fn display_path_leaves_a_plain_path_untouched() {
        assert_eq!(
            display_path(std::path::Path::new("/usr/lib/satsuma/ffmpeg/ffmpeg-x86_64-unknown-linux-gnu")),
            "/usr/lib/satsuma/ffmpeg/ffmpeg-x86_64-unknown-linux-gnu"
        );
    }
}
