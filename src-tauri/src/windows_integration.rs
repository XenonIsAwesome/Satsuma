//! Windows-only: lets the user select a file in Explorer and hold their
//! configured format/tools modifier combination (Shift/Shift+Alt by
//! default, remappable in Settings — see `satsuma_core::settings`) to open
//! Satsuma's wedge menu for it, no drag-and-drop required, mirroring how
//! Tangerine works on macOS (there via the Accessibility APIs; here via
//! Explorer's COM automation object model plus a low-level keyboard hook).
//!
//! Verified: this module has been built with the MSVC toolchain and
//! manually exercised end-to-end in a real Windows VM — the hook/overlay/
//! COM path actually works, not just type-checks. See `docs/phase0.md`'s
//! "Windows integration" note for exactly what that manual pass covered
//! (and what's still *not* yet verified on real hardware: the tray icon's
//! actual rendering/menu interaction, and the autostart entry surviving a
//! real reboot/login).
//!
//! This file can still only be *type-checked*, not built or linked, from
//! this project's own Linux dev environment
//! (`cargo check --target x86_64-pc-windows-gnu` — see the Commands
//! section of AGENTS.md/CLAUDE.md): linking a Tauri app with the mingw GNU
//! linker fails here with "export ordinal too large" (a mingw
//! PE-export-table limitation unrelated to this module's own code). The
//! verification above happened on an actual Windows machine/VM with the
//! MSVC toolchain (`x86_64-pc-windows-msvc`), which is what a real build
//! for this target needs.
//!
//! The hook only does anything while this process is running, which is why
//! `run()` (in `lib.rs`) makes Satsuma tray-resident with an optional
//! autostart-at-login entry — otherwise the trigger combination would
//! silently do nothing whenever the app hadn't been manually launched
//! first. Once the currently-held modifier keys exactly match the
//! configured format or tools combination, this module hands off to
//! `crate::show_overlay`, which positions a borderless overlay window at
//! the current cursor position (not the file icon's exact bounding rect —
//! see the design doc referenced from `docs/phase0.md` for why) and shows
//! it, rather than bringing the normal titled app window forward.
//!
//! `low_level_keyboard_proc` itself never does the COM work directly —
//! `spawn_trigger_handler` hands off to a fresh thread instead. A
//! `WH_KEYBOARD_LL` callback that doesn't return quickly gets silently
//! disabled by Windows, and `Shell.Application` COM automation (cross-
//! process, potentially over several Explorer windows) is exactly the kind
//! of thing that can be slow enough to trip that — which would otherwise
//! present as "the trigger combo did something once, then stopped
//! responding at all."
//!
//! **This hook only ever matches on the four modifier keys** — a binding
//! that also includes a non-modifier key (`HotkeyCombo::key`, added when
//! bindings were generalized to "any combination or singular key on the
//! keyboard") never triggers this Explorer-drag path, even though it works
//! everywhere else (in-app drag-and-drop, the desktop overlay). This is
//! deliberate, not an oversight: `currently_held()` below only ever tracks
//! the four modifiers and never populates `key`, so it can never equal a
//! configured combo that has one set — extending a system-wide, low-level
//! keyboard hook to arbitrary keys was judged not worth the risk, independent
//! of this module's own (now-verified — see above) build/run status.

use satsuma_core::{detect_category, HotkeyCombo, LaunchRequest, MenuMode};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter};
use windows::core::Interface;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_RCONTROL, VK_RMENU,
    VK_RSHIFT, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::Shell::{IShellFolderViewDual, IShellWindows, IWebBrowserApp, ShellWindows};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetCursorPos, GetForegroundWindow, GetMessageW, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP,
    WM_SYSKEYDOWN, WM_SYSKEYUP,
};

static CTRL_HELD: AtomicBool = AtomicBool::new(false);
static ALT_HELD: AtomicBool = AtomicBool::new(false);
static SHIFT_HELD: AtomicBool = AtomicBool::new(false);
static META_HELD: AtomicBool = AtomicBool::new(false);
static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();
/// Guards against re-firing while the matched combination stays held (the
/// OS re-fires `WM_KEYDOWN` via key-repeat for whichever key is currently
/// being auto-repeated) — reset back to `false` once any key of the
/// currently-matched combination is released, so the *next* time the user
/// assembles a matching combination it fires again. See `spawn_trigger_handler`.
static ARMED: AtomicBool = AtomicBool::new(false);

/// The user's configured format/tools modifier combinations, updated live
/// (no restart required) whenever Settings are saved — see `set_modifiers`,
/// called from `save_settings` in `lib.rs`.
static FORMAT_COMBO: Mutex<HotkeyCombo> = Mutex::new(satsuma_core::DEFAULT_FORMAT_MODIFIERS);
static TOOLS_COMBO: Mutex<HotkeyCombo> = Mutex::new(satsuma_core::DEFAULT_TOOLS_MODIFIERS);

/// Installs a global low-level keyboard hook on a dedicated thread with its
/// own Win32 message loop (required by `WH_KEYBOARD_LL`). Whenever the
/// currently-held modifier keys come to exactly match the configured format
/// or tools combination, it queries the foreground Explorer window's
/// current selection via COM automation and, if any files are selected,
/// surfaces the wedge menu for them in the matched mode.
pub fn spawn_shift_watcher(app: AppHandle, format: HotkeyCombo, tools: HotkeyCombo) {
    let _ = APP_HANDLE.set(app);
    set_modifiers(format, tools);
    std::thread::spawn(|| unsafe {
        if CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_err() {
            return;
        }

        let Ok(hook) = SetWindowsHookExW(WH_KEYBOARD_LL, Some(low_level_keyboard_proc), None, 0) else {
            return;
        };

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        let _ = UnhookWindowsHookEx(hook);
    });
}

/// Updates the live-tracked format/tools combinations. Called from
/// `save_settings` in `lib.rs` so a remap in Settings takes effect
/// immediately, with no restart, per Phase 1's success criteria.
pub fn set_modifiers(format: HotkeyCombo, tools: HotkeyCombo) {
    *FORMAT_COMBO.lock().unwrap() = format;
    *TOOLS_COMBO.lock().unwrap() = tools;
}

fn currently_held() -> HotkeyCombo {
    HotkeyCombo::new(
        CTRL_HELD.load(Ordering::SeqCst),
        ALT_HELD.load(Ordering::SeqCst),
        SHIFT_HELD.load(Ordering::SeqCst),
        META_HELD.load(Ordering::SeqCst),
    )
}

/// Pure decision of which mode (if any) the currently-held keys trigger —
/// exact-match against each configured combination, never subset/superset
/// matching, so e.g. a format binding of plain Shift and a tools binding of
/// Shift+Alt never both "match" while Shift+Alt is held. Kept as a plain
/// function, independent of the atomics above, so it's unit testable on any
/// platform even though the hook that calls it only ever runs on Windows.
fn matched_mode(held: HotkeyCombo, format: HotkeyCombo, tools: HotkeyCombo) -> Option<MenuMode> {
    if held == tools {
        Some(MenuMode::Tools)
    } else if held == format {
        Some(MenuMode::Formats)
    } else {
        None
    }
}

unsafe extern "system" fn low_level_keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let data = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        let vk = data.vkCode as u16;
        let is_ctrl = vk == VK_CONTROL.0 || vk == VK_LCONTROL.0 || vk == VK_RCONTROL.0;
        let is_alt = vk == VK_MENU.0 || vk == VK_LMENU.0 || vk == VK_RMENU.0;
        let is_shift = vk == VK_SHIFT.0 || vk == VK_LSHIFT.0 || vk == VK_RSHIFT.0;
        let is_meta = vk == VK_LWIN.0 || vk == VK_RWIN.0;

        match wparam.0 as u32 {
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                if is_ctrl {
                    CTRL_HELD.store(true, Ordering::SeqCst);
                } else if is_alt {
                    ALT_HELD.store(true, Ordering::SeqCst);
                } else if is_shift {
                    SHIFT_HELD.store(true, Ordering::SeqCst);
                } else if is_meta {
                    META_HELD.store(true, Ordering::SeqCst);
                }

                if is_ctrl || is_alt || is_shift || is_meta {
                    let held = currently_held();
                    let format = FORMAT_COMBO.lock().unwrap().clone();
                    let tools = TOOLS_COMBO.lock().unwrap().clone();
                    if let Some(mode) = matched_mode(held, format, tools) {
                        if !ARMED.swap(true, Ordering::SeqCst) {
                            spawn_trigger_handler(mode);
                        }
                    }
                }
            }
            WM_KEYUP | WM_SYSKEYUP => {
                if is_ctrl {
                    CTRL_HELD.store(false, Ordering::SeqCst);
                } else if is_alt {
                    ALT_HELD.store(false, Ordering::SeqCst);
                } else if is_shift {
                    SHIFT_HELD.store(false, Ordering::SeqCst);
                } else if is_meta {
                    META_HELD.store(false, Ordering::SeqCst);
                }

                if is_ctrl || is_alt || is_shift || is_meta {
                    let held = currently_held();
                    let format = FORMAT_COMBO.lock().unwrap().clone();
                    let tools = TOOLS_COMBO.lock().unwrap().clone();
                    if matched_mode(held, format, tools).is_none() {
                        ARMED.store(false, Ordering::SeqCst);
                    }
                }
            }
            _ => {}
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

/// Spawns a dedicated thread to actually handle the trigger, so the
/// `WH_KEYBOARD_LL` callback itself (`low_level_keyboard_proc`) returns
/// immediately rather than blocking on `handle_trigger`'s COM automation.
/// This matters a lot in practice: Windows silently disables a low-level
/// keyboard hook that takes too long to return from a single invocation,
/// which would otherwise turn "COM was slow this one time" into "the
/// trigger stops doing anything for the rest of the session."
fn spawn_trigger_handler(mode: MenuMode) {
    std::thread::spawn(move || {
        if unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok() {
            handle_trigger(mode);
            unsafe { CoUninitialize() };
        }
    });
}

/// Does the actual Explorer-selection COM query and shows the overlay in
/// `mode`. Must run on a thread with its own COM apartment initialized —
/// see `spawn_trigger_handler`, the only caller.
fn handle_trigger(mode: MenuMode) {
    let Some(app) = APP_HANDLE.get() else { return };

    let paths = match unsafe { foreground_explorer_selection() } {
        Some(paths) if !paths.is_empty() => paths,
        _ => return,
    };

    let category = detect_category(&paths[0]);
    let request = LaunchRequest { mode, paths, category };

    // An unsupported file type has no wedge options to show at all — skip
    // opening the overlay for it entirely rather than showing a blank,
    // click-catching window (see the doc comment on `LaunchRequest::category`).
    if category == satsuma_core::FileCategory::Unknown {
        return;
    }

    let _ = app.emit("launch-request", request);
    crate::show_overlay(app);
}

/// Best-effort current cursor position via `GetCursorPos`. `None` only on
/// the (essentially theoretical) Win32 failure case; the overlay falls back
/// to monitor-center placement when that happens.
pub(crate) fn cursor_position() -> Option<(i32, i32)> {
    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point) }.ok()?;
    Some((point.x, point.y))
}

/// The actual current held state — the four modifiers, plus at most one
/// other currently-held key from `satsuma_core::keys`' supported set —
/// queried directly via `GetAsyncKeyState` rather than reconstructed from
/// individual keydown/keyup DOM events in the frontend. Feeds only the
/// Settings screen's recorder (see the module doc comment for why a
/// recorded extra key still won't trigger *this* module's own Explorer-drag
/// hook).
///
/// This exists because that DOM reconstruction (tried first, in
/// `HotkeyRecorder.tsx`/`useHeldModifiers.ts`) turned out to still be
/// unreliable in practice for a real Shift+Alt press: Alt in particular is
/// a menu-accelerator/mnemonic key that some native window
/// toolkits/webviews can intercept before its own keydown/keyup ever
/// reaches the page's JS — no amount of listening in the page can see an
/// event that was never delivered to it. `GetAsyncKeyState` queries the
/// OS's own live keyboard state directly (as of the most recent message
/// pump, regardless of which window has focus or would otherwise receive
/// the key message), so it isn't subject to that interception at all.
/// Always succeeds in practice; there's no Windows-specific unavailability
/// case the way there is for X11 on a Wayland session.
pub(crate) fn poll_held_modifiers() -> HotkeyCombo {
    fn is_down(vk: u16) -> bool {
        (unsafe { GetAsyncKeyState(vk as i32) } as u16) & 0x8000 != 0
    }

    let mut combo = HotkeyCombo::new(
        is_down(VK_CONTROL.0),
        is_down(VK_MENU.0),
        is_down(VK_SHIFT.0),
        is_down(VK_LWIN.0) || is_down(VK_RWIN.0),
    );

    // At most 74 quick `GetAsyncKeyState` calls, only while the Settings
    // screen's recorder is actively polling (a few seconds at most) — not
    // worth pre-filtering to "likely" keys first.
    if let Some((code, _)) = satsuma_core::windows_vks().find(|(_, vk)| is_down(*vk)) {
        combo.key = Some(code.to_string());
    }

    combo
}

/// Best-effort query of the currently selected file paths in whichever
/// Explorer window is in the foreground, via Shell.Application automation
/// (`IShellWindows` -> matching `IWebBrowserApp` by HWND -> its
/// `IShellFolderViewDual` document -> `SelectedItems`). Returns `None` if
/// the foreground window isn't an Explorer window, or on any COM failure.
unsafe fn foreground_explorer_selection() -> Option<Vec<String>> {
    let foreground = GetForegroundWindow();
    if foreground.0.is_null() {
        return None;
    }

    let shell_windows: IShellWindows = CoCreateInstance(&ShellWindows, None, CLSCTX_LOCAL_SERVER).ok()?;
    let count = shell_windows.Count().ok()?;

    for index in 0..count {
        let Ok(dispatch) = shell_windows.Item(&VARIANT::from(index)) else {
            continue;
        };
        let Ok(browser) = dispatch.cast::<IWebBrowserApp>() else {
            continue;
        };
        let Ok(hwnd_value) = browser.HWND() else {
            continue;
        };
        if HWND(hwnd_value.0 as *mut _) != foreground {
            continue;
        }

        let Ok(document) = browser.Document() else {
            continue;
        };
        let Ok(view) = document.cast::<IShellFolderViewDual>() else {
            continue;
        };
        let Ok(selected_items) = view.SelectedItems() else {
            continue;
        };
        let item_count = selected_items.Count().ok()?;
        if item_count == 0 {
            return None;
        }

        let mut paths = Vec::with_capacity(item_count as usize);
        for item_index in 0..item_count {
            if let Ok(item) = selected_items.Item(&VARIANT::from(item_index)) {
                if let Ok(path) = item.Path() {
                    paths.push(path.to_string());
                }
            }
        }
        return Some(paths);
    }

    None
}

/// Exercises the pure argument-parsing and modifier-matching paths this
/// module ultimately builds a `LaunchRequest` through, so at least those
/// slices are covered by a test that runs wherever this crate is
/// checked/tested (the COM/hook glue above cannot be: it requires a live
/// Explorer process and a real message loop).
#[cfg(test)]
mod tests {
    use super::*;
    use satsuma_core::parse_launch_args;

    #[test]
    fn mode_flag_and_paths_round_trip_into_a_launch_request() {
        let request = parse_launch_args(["--mode=tools", "C:\\Users\\me\\clip.mp4"]).unwrap();
        assert_eq!(request.mode, MenuMode::Tools);
        assert_eq!(request.paths, vec!["C:\\Users\\me\\clip.mp4".to_string()]);
    }

    #[test]
    fn matches_the_tools_combo_over_the_format_combo_when_both_are_technically_close() {
        let format = HotkeyCombo::new(false, false, true, false); // Shift
        let tools = HotkeyCombo::new(false, true, true, false); // Shift+Alt
        let shift_alt_held = HotkeyCombo::new(false, true, true, false);
        assert_eq!(matched_mode(shift_alt_held, format, tools), Some(MenuMode::Tools));
    }

    #[test]
    fn matches_the_format_combo_when_only_its_exact_keys_are_held() {
        let format = HotkeyCombo::new(false, false, true, false); // Shift
        let tools = HotkeyCombo::new(false, true, true, false); // Shift+Alt
        let shift_only_held = HotkeyCombo::new(false, false, true, false);
        assert_eq!(matched_mode(shift_only_held, format, tools), Some(MenuMode::Formats));
    }

    #[test]
    fn matches_nothing_for_an_unrelated_held_combination() {
        let format = HotkeyCombo::new(false, false, true, false);
        let tools = HotkeyCombo::new(false, true, true, false);
        let ctrl_only_held = HotkeyCombo::new(true, false, false, false);
        assert_eq!(matched_mode(ctrl_only_held, format, tools), None);
    }

    #[test]
    fn matches_a_remapped_ctrl_plus_win_combination() {
        let format = HotkeyCombo::new(true, false, false, true); // Ctrl+Win
        let tools = HotkeyCombo::new(true, true, false, true); // Ctrl+Alt+Win
        let held = HotkeyCombo::new(true, false, false, true);
        assert_eq!(matched_mode(held, format, tools), Some(MenuMode::Formats));
    }

    #[test]
    fn never_matches_a_binding_that_includes_an_extra_key() {
        // This hook only ever tracks the four modifiers (see
        // `currently_held`) — a binding with `key` set can never be
        // reproduced by anything this hook observes, by construction, so it
        // can never trigger here (see the module doc comment for why that's
        // deliberate rather than a bug).
        let format_with_key = HotkeyCombo::new(false, false, true, false).with_key("KeyP");
        let tools = HotkeyCombo::new(false, true, true, false);
        let shift_only_held = HotkeyCombo::new(false, false, true, false);
        assert_eq!(matched_mode(shift_only_held, format_with_key, tools), None);
    }
}
