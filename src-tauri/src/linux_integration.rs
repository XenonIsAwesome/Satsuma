//! Installs Satsuma's Linux file-manager integration: a standard `.desktop`
//! MIME association (so Satsuma shows up in every file manager's "Open
//! With" menu), plus direct context-menu entries for the file managers that
//! support them (Nautilus scripts, Nemo actions, KDE Dolphin service
//! menus). There's no cross-file-manager API for a native top-level
//! context-menu entry, so "Open With" is the guaranteed-to-work fallback
//! for everything else (Thunar, PCManFM, ...).
use satsuma_core::HotkeyCombo;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    change_property, change_window_attributes, get_property, intern_atom, query_keymap, query_pointer,
    send_event, AtomEnum, ChangeWindowAttributesAux, ClientMessageData, ClientMessageEvent, EventMask, PropMode,
    MOTION_NOTIFY_EVENT,
};
use x11rb::protocol::xtest::fake_input;
use x11rb::protocol::Event;

const DESKTOP_ENTRY: &str = include_str!("../resources/linux/applications/satsuma.desktop");
const NAUTILUS_CONVERT_SCRIPT: &str =
    include_str!("../resources/linux/nautilus-scripts/Convert with Satsuma");
const NAUTILUS_TOOLS_SCRIPT: &str = include_str!("../resources/linux/nautilus-scripts/Satsuma Tools");
const NEMO_CONVERT_ACTION: &str =
    include_str!("../resources/linux/nemo-actions/satsuma-convert.nemo_action");
const NEMO_TOOLS_ACTION: &str = include_str!("../resources/linux/nemo-actions/satsuma-tools.nemo_action");
const KDE_SERVICE_MENU: &str = include_str!("../resources/linux/dolphin-servicemenus/satsuma.desktop");

/// Placeholder substituted with the running binary's absolute path at
/// install time (see `install`). Using an absolute path rather than relying
/// on `satsuma` resolving correctly via `$PATH` avoids a real bug found in
/// practice: a stale/different `satsuma` earlier on `$PATH` (e.g. a system
/// package, or another build) silently intercepting the context-menu
/// trigger instead of whichever build the user actually installed the
/// integration from.
const EXE_PLACEHOLDER: &str = "@SATSUMA_EXE@";

const DESKTOP_FILE_NAME: &str = "satsuma.desktop";
const NAUTILUS_CONVERT_SCRIPT_NAME: &str = "Convert with Satsuma";
const NAUTILUS_TOOLS_SCRIPT_NAME: &str = "Satsuma Tools";
const NEMO_CONVERT_ACTION_NAME: &str = "satsuma-convert.nemo_action";
const NEMO_TOOLS_ACTION_NAME: &str = "satsuma-tools.nemo_action";

/// The XDG directories Satsuma's integration files get installed into,
/// rooted at a data-home directory (normally `$XDG_DATA_HOME` or
/// `~/.local/share`). Parameterized so install/uninstall/is_installed can
/// be unit tested against a temp directory instead of the real home dir.
pub struct IntegrationPaths {
    pub applications_dir: PathBuf,
    pub nautilus_scripts_dir: PathBuf,
    pub nemo_actions_dir: PathBuf,
    pub kde_servicemenus_dir: PathBuf,
}

impl IntegrationPaths {
    pub fn under_data_home(data_home: &Path) -> Self {
        Self {
            applications_dir: data_home.join("applications"),
            nautilus_scripts_dir: data_home.join("nautilus/scripts"),
            nemo_actions_dir: data_home.join("nemo/actions"),
            kde_servicemenus_dir: data_home.join("kio/servicemenus"),
        }
    }

    fn desktop_file(&self) -> PathBuf {
        self.applications_dir.join(DESKTOP_FILE_NAME)
    }

    fn nautilus_convert_script(&self) -> PathBuf {
        self.nautilus_scripts_dir.join(NAUTILUS_CONVERT_SCRIPT_NAME)
    }

    fn nautilus_tools_script(&self) -> PathBuf {
        self.nautilus_scripts_dir.join(NAUTILUS_TOOLS_SCRIPT_NAME)
    }

    fn nemo_convert_action(&self) -> PathBuf {
        self.nemo_actions_dir.join(NEMO_CONVERT_ACTION_NAME)
    }

    fn nemo_tools_action(&self) -> PathBuf {
        self.nemo_actions_dir.join(NEMO_TOOLS_ACTION_NAME)
    }

    fn kde_service_menu(&self) -> PathBuf {
        self.kde_servicemenus_dir.join(DESKTOP_FILE_NAME)
    }
}

/// Resolves the real `IntegrationPaths` for the current user, honoring
/// `XDG_DATA_HOME` and falling back to `~/.local/share`. Returns `None` if
/// neither `XDG_DATA_HOME` nor `HOME` is set.
pub fn default_paths() -> Option<IntegrationPaths> {
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))?;
    Some(IntegrationPaths::under_data_home(&data_home))
}

/// Installs the integration files with `exe_path` (normally
/// `std::env::current_exe()`) baked in as the command each trigger runs —
/// see `EXE_PLACEHOLDER` for why this matters.
pub fn install(paths: &IntegrationPaths, exe_path: &Path) -> io::Result<()> {
    let exe = exe_path.to_string_lossy();
    let desktop_exec = desktop_quote(&exe);
    let shell_exec = shell_quote(&exe);

    write_file(&paths.desktop_file(), &DESKTOP_ENTRY.replace(EXE_PLACEHOLDER, &desktop_exec), false)?;
    write_file(
        &paths.nautilus_convert_script(),
        &NAUTILUS_CONVERT_SCRIPT.replace(EXE_PLACEHOLDER, &shell_exec),
        true,
    )?;
    write_file(
        &paths.nautilus_tools_script(),
        &NAUTILUS_TOOLS_SCRIPT.replace(EXE_PLACEHOLDER, &shell_exec),
        true,
    )?;
    write_file(
        &paths.nemo_convert_action(),
        &NEMO_CONVERT_ACTION.replace(EXE_PLACEHOLDER, &desktop_exec),
        false,
    )?;
    write_file(
        &paths.nemo_tools_action(),
        &NEMO_TOOLS_ACTION.replace(EXE_PLACEHOLDER, &desktop_exec),
        false,
    )?;
    write_file(&paths.kde_service_menu(), &KDE_SERVICE_MENU.replace(EXE_PLACEHOLDER, &desktop_exec), false)?;
    Ok(())
}

/// Quotes a value for a Desktop Entry `Exec=` field (always safe to quote,
/// per the spec, even when not strictly required) — escapes the characters
/// the spec requires escaping inside a double-quoted value.
fn desktop_quote(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"").replace('$', "\\$").replace('`', "\\`");
    format!("\"{escaped}\"")
}

/// Quotes a value for interpolation into a POSIX `sh` script (single-quoted,
/// with embedded single quotes escaped the standard `'\''` way).
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub fn uninstall(paths: &IntegrationPaths) -> io::Result<()> {
    for path in [
        paths.desktop_file(),
        paths.nautilus_convert_script(),
        paths.nautilus_tools_script(),
        paths.nemo_convert_action(),
        paths.nemo_tools_action(),
        paths.kde_service_menu(),
    ] {
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

/// Whether the integration is currently installed. Only checks the
/// `.desktop` file (the piece every install path shares) rather than
/// requiring every optional per-file-manager file to be present.
pub fn is_installed(paths: &IntegrationPaths) -> bool {
    paths.desktop_file().exists()
}

/// Best-effort current cursor position via `XQueryPointer` on the root
/// window of the default X11 display. Returns `None` on a Wayland session
/// (detected via `XDG_SESSION_TYPE`, since a plain X11 connection attempt
/// can still spuriously succeed under XWayland but won't reflect the real
/// compositor-wide pointer) or on any connection/protocol failure — the
/// overlay window falls back to monitor-center placement in that case.
pub fn cursor_position() -> Option<(i32, i32)> {
    if std::env::var("XDG_SESSION_TYPE").as_deref() == Ok("wayland") {
        return None;
    }

    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots.get(screen_num)?.root;
    let reply = x11rb::protocol::xproto::query_pointer(&conn, root).ok()?.reply().ok()?;
    Some((reply.root_x as i32, reply.root_y as i32))
}

/// Standard "evdev" XKB-ruleset keycodes (X11 keycode = Linux kernel evdev
/// scancode + 8) for each modifier's left/right variant — confirmed
/// against a real `xmodmap -pm` listing during development, where every
/// one of these matched exactly. See `satsuma_core::keys` for the same
/// convention applied to the rest of the keyboard.
const KEYCODE_CONTROL_L: u8 = 37;
const KEYCODE_CONTROL_R: u8 = 105;
const KEYCODE_ALT_L: u8 = 64;
const KEYCODE_ALT_R: u8 = 108;
const KEYCODE_SHIFT_L: u8 = 50;
const KEYCODE_SHIFT_R: u8 = 62;
const KEYCODE_SUPER_L: u8 = 133;
const KEYCODE_SUPER_R: u8 = 134;

/// Whether keycode `keycode` is currently pressed, per `XQueryKeymap`'s
/// 256-bit-as-32-bytes reply: bit `keycode % 8` of byte `keycode / 8`.
fn key_pressed(keys: &[u8; 32], keycode: u8) -> bool {
    keys[(keycode / 8) as usize] & (1 << (keycode % 8)) != 0
}

/// Best-effort snapshot of the actual current held state — the four
/// modifiers, plus at most one other currently-held key from
/// `satsuma_core::keys`' supported set — queried directly from the X
/// server rather than reconstructed from individual keydown/keyup DOM
/// events in the frontend.
///
/// This exists because that DOM reconstruction (tried first, in
/// `HotkeyRecorder.tsx`/`useHeldModifiers.ts`) turned out to still be
/// unreliable in practice for a real Shift+Alt press: WebKitGTK's own
/// mnemonic/menu-accelerator key handling can intercept a modifier key's
/// keydown/keyup before the webview's JS ever sees it, regardless of
/// whether the app actually shows a menu bar — no amount of `keydown`/
/// `keyup` listening in the page can see an event that was never
/// delivered to it.
///
/// Uses `XQueryKeymap` (the X server's raw per-keycode "is this physical
/// key currently down" bitmap) rather than `XQueryPointer`'s aggregated
/// modifier mask, which an earlier version of this function used and which
/// turned out to *also* be unreliable in practice: it under-reported Alt
/// specifically while Shift was already held, even on a system whose own
/// `xmodmap -pm` output confirmed the standard `Mod1` = Alt mapping —
/// something about how the X server folds physical keys into that
/// aggregated mask evidently isn't as dependable as reading a specific
/// keycode's own bit directly. Checking fixed, standard "evdev" keycodes
/// (see the `KEYCODE_*` constants above) for the four modifiers sidesteps
/// that aggregation entirely, and doubles as the exact mechanism needed to
/// also detect an arbitrary extra key (`satsuma_core::keys::x11_keycodes`)
/// — there's no equivalent "aggregated mask" abstraction to fall back on
/// for keys that aren't modifiers, so this was the more general approach
/// either way.
///
/// Returns `None` on a Wayland session (no equivalent global keyboard-state
/// query exists for an ordinary client there) or on any connection/protocol
/// failure — callers fall back to plain DOM keyboard events in that case.
pub fn poll_held_modifiers() -> Option<HotkeyCombo> {
    if std::env::var("XDG_SESSION_TYPE").as_deref() == Ok("wayland") {
        return None;
    }

    let (conn, _screen_num) = x11rb::connect(None).ok()?;
    let reply = query_keymap(&conn).ok()?.reply().ok()?;
    let keys = reply.keys;

    let mut combo = HotkeyCombo::new(
        key_pressed(&keys, KEYCODE_CONTROL_L) || key_pressed(&keys, KEYCODE_CONTROL_R),
        key_pressed(&keys, KEYCODE_ALT_L) || key_pressed(&keys, KEYCODE_ALT_R),
        key_pressed(&keys, KEYCODE_SHIFT_L) || key_pressed(&keys, KEYCODE_SHIFT_R),
        key_pressed(&keys, KEYCODE_SUPER_L) || key_pressed(&keys, KEYCODE_SUPER_R),
    );

    if let Some((code, _)) =
        satsuma_core::x11_keycodes().find(|(_, keycode)| key_pressed(&keys, *keycode))
    {
        combo.key = Some(code.to_string());
    }

    Some(combo)
}

/// Best-effort: actually activates `xid` (the overlay's own X11 window) by
/// sending the window manager a properly-timestamped `_NET_ACTIVE_WINDOW`
/// client message — the same EWMH mechanism tools like `wmctrl -a` use.
///
/// This exists because Tauri's own `WebviewWindow::set_focus()` asks GTK to
/// `gtk_window_present_with_time` using `GDK_CURRENT_TIME` (i.e. no real
/// timestamp) under the hood. Focus-stealing-prevention-aware window
/// managers — confirmed against real Linux Mint/Cinnamon (Muffin) hardware
/// — treat an untimestamped request as low-confidence and silently ignore
/// it for anything but a freshly-launched process's very first window;
/// every later overlay trigger in an already-running process was
/// consistently ignored until the user physically clicked the window
/// (which carries a real timestamp and always succeeds via the normal
/// click-to-focus path). A genuine, current server timestamp — obtained via
/// the standard X11 "self-inflicted `PropertyNotify`" trick in
/// `current_server_time` below — makes the window manager treat the
/// request as legitimate instead.
///
/// Guards against overlapping `force_activate_window` calls — e.g. a rapid
/// second overlay trigger while an earlier call is still mid-flight — from
/// racing each other over the same window's activation state.
static ACTIVATING: AtomicBool = AtomicBool::new(false);

/// Returns `false` (a no-op) on a Wayland session, where none of this EWMH
/// machinery applies, on any connection/protocol failure, or if another
/// call is already in flight — the overlay then relies on whatever
/// `set_focus()` alone still manages.
pub fn force_activate_window(xid: u32) -> bool {
    #[cfg(feature = "debug-overlay-focus")]
    eprintln!("[satsuma +{}ms] force_activate_window: called (xid={xid})", crate::log_ts());
    if std::env::var("XDG_SESSION_TYPE").as_deref() == Ok("wayland") {
        #[cfg(feature = "debug-overlay-focus")]
        eprintln!("[satsuma +{}ms] force_activate_window: skipped (Wayland session)", crate::log_ts());
        return false;
    }
    if ACTIVATING.swap(true, Ordering::SeqCst) {
        #[cfg(feature = "debug-overlay-focus")]
        eprintln!(
            "[satsuma +{}ms] force_activate_window: skipped (another call already in flight)",
            crate::log_ts()
        );
        return false;
    }

    let result = (|| -> Option<()> {
        let (conn, screen_num) = x11rb::connect(None).ok()?;
        let root = conn.setup().roots.get(screen_num)?.root;
        #[cfg(feature = "debug-overlay-focus")]
        eprintln!("[satsuma +{}ms] force_activate_window: connected", crate::log_ts());

        let timestamp = current_server_time(&conn, xid)?;
        #[cfg(feature = "debug-overlay-focus")]
        eprintln!("[satsuma +{}ms] force_activate_window: got timestamp {timestamp}", crate::log_ts());
        let net_active_window = intern_atom(&conn, false, b"_NET_ACTIVE_WINDOW").ok()?.reply().ok()?.atom;

        nudge_input_activity(&conn, root);

        // Per the EWMH spec: data.l[0] = 1 (source indication: a normal
        // application, not a pager), l[1] = timestamp, l[2] = 0 (the
        // currently active window, left unspecified).
        let event = ClientMessageEvent::new(
            32,
            xid,
            net_active_window,
            ClientMessageData::from([1, timestamp, 0, 0, 0]),
        );
        let event_mask = EventMask::SUBSTRUCTURE_NOTIFY | EventMask::SUBSTRUCTURE_REDIRECT;
        send_event(&conn, false, root, event_mask, event).ok()?;
        conn.flush().ok()?;
        Some(())
    })();

    ACTIVATING.store(false, Ordering::SeqCst);
    #[cfg(feature = "debug-overlay-focus")]
    eprintln!(
        "[satsuma +{}ms] force_activate_window: {}",
        crate::log_ts(),
        if result.is_some() { "sent" } else { "failed" }
    );
    result.is_some()
}

/// Synthesizes a zero-displacement mouse move (via the XTest extension) at
/// the cursor's own current position, immediately before sending the
/// `_NET_ACTIVE_WINDOW` request.
///
/// This exists because a correct timestamp alone turned out not to be
/// enough: real-hardware testing showed `force_activate_window` sending a
/// well-formed, correctly-timestamped request that the window manager
/// still didn't act on for anywhere from milliseconds to well over a
/// minute — until the user happened to move the mouse or click something,
/// at which point it took effect immediately. That points to Mutter/Muffin
/// deferring an unsolicited activation request until it separately
/// observes what it considers recent genuine input activity, rather than
/// evaluating the request's timestamp alone. A synthetic input event is an
/// explicit attempt to satisfy that, since we have no legitimate one to
/// offer — the cursor doesn't actually move (the target position is
/// exactly where it already is), so nothing is visibly different on
/// screen. Best-effort: failures are silently ignored, since this is a
/// heuristic nudge, not something the rest of the flow depends on.
fn nudge_input_activity(conn: &impl Connection, root: u32) {
    let Some(pointer) = query_pointer(conn, root).ok().and_then(|c| c.reply().ok()) else { return };
    // detail=0: absolute position, per the XTest spec; time=0: CurrentTime;
    // deviceid=0: the default core pointer device.
    let _ = fake_input(conn, MOTION_NOTIFY_EVENT, 0, 0, root, pointer.root_x, pointer.root_y, 0);
    let _ = conn.flush();
}

/// Whether the overlay is currently supposed to be visible — set by
/// `show_overlay`/`hide_overlay` (via `mark_overlay_visible`) so
/// `spawn_active_window_watcher`'s background thread can tell a real
/// "user switched away from the overlay" apart from ordinary
/// `_NET_ACTIVE_WINDOW` churn happening while the overlay isn't even open
/// (which happens constantly during normal desktop use and must be
/// ignored).
static OVERLAY_VISIBLE: AtomicBool = AtomicBool::new(false);

pub fn mark_overlay_visible(visible: bool) {
    OVERLAY_VISIBLE.store(visible, Ordering::SeqCst);
}

/// Spawns a single, persistent background thread (call once, at startup)
/// that watches the root window's `_NET_ACTIVE_WINDOW` property and hides
/// the overlay whenever it changes to something other than the overlay's
/// own window while the overlay is visible — i.e. whenever the user
/// switches to a different window.
///
/// This exists as a more reliable alternative to relying on the overlay
/// itself receiving a `Focused(false)` event: `force_activate_window`
/// above makes the overlay *become* active, but Mutter/Muffin's
/// focus-stealing-prevention can — confirmed on real hardware — silently
/// decline even a well-formed, correctly-timestamped activation request
/// for reasons not exposed to the client, meaning the overlay sometimes
/// never becomes "active" at all and can then never be observed *losing*
/// that status either. Watching what the window manager itself considers
/// active sidesteps that entirely: it only depends on Mutter correctly
/// reporting what changed, not on us winning any focus-stealing heuristic.
///
/// A no-op on a Wayland session, where `_NET_ACTIVE_WINDOW` root-window
/// watching doesn't apply.
pub fn spawn_active_window_watcher(app: tauri::AppHandle, overlay_xid: u32) {
    if std::env::var("XDG_SESSION_TYPE").as_deref() == Ok("wayland") {
        return;
    }

    std::thread::spawn(move || {
        if watch_active_window(&app, overlay_xid).is_none() {
            #[cfg(feature = "debug-overlay-focus")]
            eprintln!("[satsuma +{}ms] active-window watcher: failed to start, giving up", crate::log_ts());
        }
    });
}

/// The actual watch loop, split out from `spawn_active_window_watcher` so
/// early failures can use `?` and return `None` instead of nested
/// `let...else` blocks. Blocks indefinitely on `wait_for_event` — fine
/// here, unlike in `current_server_time`, because this is the one
/// persistent watcher thread for the process's whole lifetime, not
/// something spawned fresh per overlay trigger; there's nothing to leak by
/// design, since it's meant to run forever.
fn watch_active_window(app: &tauri::AppHandle, overlay_xid: u32) -> Option<()> {
    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots.get(screen_num)?.root;
    let net_active_window = intern_atom(&conn, false, b"_NET_ACTIVE_WINDOW").ok()?.reply().ok()?.atom;

    change_window_attributes(&conn, root, &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE))
        .ok()?
        .check()
        .ok()?;

    #[cfg(feature = "debug-overlay-focus")]
    eprintln!("[satsuma +{}ms] active-window watcher: started", crate::log_ts());

    loop {
        let Event::PropertyNotify(notify) = conn.wait_for_event().ok()? else { continue };
        if notify.window != root || notify.atom != net_active_window {
            continue;
        }
        if !OVERLAY_VISIBLE.load(Ordering::SeqCst) {
            continue;
        }

        let active = get_property(&conn, false, root, net_active_window, AtomEnum::WINDOW, 0, 1)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .and_then(|reply| reply.value32().and_then(|mut values| values.next()));

        if active != Some(overlay_xid) {
            #[cfg(feature = "debug-overlay-focus")]
            eprintln!(
                "[satsuma +{}ms] active-window watcher: active window changed to {active:?} (overlay is {overlay_xid}) — hiding",
                crate::log_ts()
            );
            OVERLAY_VISIBLE.store(false, Ordering::SeqCst);
            crate::hide_overlay(app.clone());
        }
    }
}

/// How long `current_server_time` polls for its `PropertyNotify` before
/// giving up. Generous relative to how fast a self-inflicted, same-machine
/// round-trip normally resolves (sub-millisecond in practice), but still a
/// hard, real wall-clock bound — see the doc comment below for why that
/// matters.
const TIMESTAMP_PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(500);

/// Obtains a real, current X11 server timestamp by changing a dummy
/// property on `window` (after selecting `PROPERTY_CHANGE` events on it)
/// and reading the resulting `PropertyNotify`'s own `time` field — the
/// standard idiom for this, since X11 has no direct "give me the current
/// time" request.
///
/// Uses non-blocking `poll_for_event` in a loop bounded by a real
/// wall-clock deadline (`TIMESTAMP_PROBE_TIMEOUT`), not `wait_for_event`
/// (which blocks with no timeout at all) or an event-count bound (which
/// still blocks forever on the very first call if no event ever arrives —
/// this function runs on its own dedicated thread per `force_activate_window`
/// call, so a wait that never returns leaks that thread and its X11
/// connection/file descriptor forever, silently, on every overlay trigger
/// that hits it; a real deadline is the only way to guarantee this function
/// always returns.
fn current_server_time(conn: &impl Connection, window: u32) -> Option<u32> {
    change_window_attributes(conn, window, &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE))
        .ok()?
        .check()
        .ok()?;

    let atom = intern_atom(conn, false, b"SATSUMA_TIMESTAMP_PROBE").ok()?.reply().ok()?.atom;
    change_property(conn, PropMode::REPLACE, window, atom, AtomEnum::STRING, 8, 0, &[]).ok()?;
    conn.flush().ok()?;

    let deadline = std::time::Instant::now() + TIMESTAMP_PROBE_TIMEOUT;
    while std::time::Instant::now() < deadline {
        match conn.poll_for_event() {
            Ok(Some(Event::PropertyNotify(notify))) if notify.window == window && notify.atom == atom => {
                return Some(notify.time);
            }
            Ok(Some(_)) => {} // an unrelated event — keep polling
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(5)),
            #[cfg_attr(not(feature = "debug-overlay-focus"), allow(unused_variables))]
            Err(error) => {
                #[cfg(feature = "debug-overlay-focus")]
                eprintln!("[satsuma +{}ms] current_server_time: connection error: {error:?}", crate::log_ts());
                return None;
            }
        }
    }
    #[cfg(feature = "debug-overlay-focus")]
    eprintln!("[satsuma +{}ms] current_server_time: timed out waiting for PropertyNotify", crate::log_ts());
    None
}

fn write_file(path: &Path, contents: &str, executable: bool) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)?;
    if executable {
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    /// A scratch directory unique per test, cleaned up on drop.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let id = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!(
                "satsuma-linux-integration-test-{}-{}",
                std::process::id(),
                id
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn is_installed_is_false_before_install() {
        let dir = TempDir::new();
        let paths = IntegrationPaths::under_data_home(&dir.0);
        assert!(!is_installed(&paths));
    }

    const TEST_EXE: &str = "/opt/satsuma/satsuma";

    #[test]
    fn install_writes_all_six_files_with_the_exe_path_substituted() {
        let dir = TempDir::new();
        let paths = IntegrationPaths::under_data_home(&dir.0);

        install(&paths, Path::new(TEST_EXE)).unwrap();

        assert!(is_installed(&paths));
        assert_eq!(
            fs::read_to_string(paths.desktop_file()).unwrap(),
            DESKTOP_ENTRY.replace(EXE_PLACEHOLDER, "\"/opt/satsuma/satsuma\"")
        );
        assert_eq!(
            fs::read_to_string(paths.nautilus_convert_script()).unwrap(),
            NAUTILUS_CONVERT_SCRIPT.replace(EXE_PLACEHOLDER, "'/opt/satsuma/satsuma'")
        );
        assert_eq!(
            fs::read_to_string(paths.nautilus_tools_script()).unwrap(),
            NAUTILUS_TOOLS_SCRIPT.replace(EXE_PLACEHOLDER, "'/opt/satsuma/satsuma'")
        );
        assert_eq!(
            fs::read_to_string(paths.nemo_convert_action()).unwrap(),
            NEMO_CONVERT_ACTION.replace(EXE_PLACEHOLDER, "\"/opt/satsuma/satsuma\"")
        );
        assert_eq!(
            fs::read_to_string(paths.nemo_tools_action()).unwrap(),
            NEMO_TOOLS_ACTION.replace(EXE_PLACEHOLDER, "\"/opt/satsuma/satsuma\"")
        );
        assert_eq!(
            fs::read_to_string(paths.kde_service_menu()).unwrap(),
            KDE_SERVICE_MENU.replace(EXE_PLACEHOLDER, "\"/opt/satsuma/satsuma\"")
        );
    }

    #[test]
    fn install_quotes_a_path_containing_a_space_and_a_single_quote_for_the_shell_scripts() {
        let dir = TempDir::new();
        let paths = IntegrationPaths::under_data_home(&dir.0);

        install(&paths, Path::new("/home/o'brien/my apps/satsuma")).unwrap();

        let script = fs::read_to_string(paths.nautilus_convert_script()).unwrap();
        assert!(script.contains("exec '/home/o'\\''brien/my apps/satsuma' --mode=formats \"$@\""));
    }

    #[test]
    fn install_makes_nautilus_scripts_executable() {
        let dir = TempDir::new();
        let paths = IntegrationPaths::under_data_home(&dir.0);

        install(&paths, Path::new(TEST_EXE)).unwrap();

        let mode = fs::metadata(paths.nautilus_convert_script()).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111, "script should be executable by all");
    }

    #[test]
    fn uninstall_removes_everything_install_created() {
        let dir = TempDir::new();
        let paths = IntegrationPaths::under_data_home(&dir.0);

        install(&paths, Path::new(TEST_EXE)).unwrap();
        uninstall(&paths).unwrap();

        assert!(!is_installed(&paths));
        assert!(!paths.nautilus_convert_script().exists());
        assert!(!paths.nautilus_tools_script().exists());
        assert!(!paths.nemo_convert_action().exists());
        assert!(!paths.nemo_tools_action().exists());
        assert!(!paths.kde_service_menu().exists());
    }

    #[test]
    fn uninstall_without_a_prior_install_is_not_an_error() {
        let dir = TempDir::new();
        let paths = IntegrationPaths::under_data_home(&dir.0);
        assert!(uninstall(&paths).is_ok());
    }

    #[test]
    fn cursor_position_is_none_on_a_wayland_session() {
        // SAFETY: test-only mutation of process env, restored immediately after.
        let previous = std::env::var_os("XDG_SESSION_TYPE");
        unsafe {
            std::env::set_var("XDG_SESSION_TYPE", "wayland");
        }
        let result = cursor_position();
        match previous {
            Some(value) => unsafe { std::env::set_var("XDG_SESSION_TYPE", value) },
            None => unsafe { std::env::remove_var("XDG_SESSION_TYPE") },
        }
        assert_eq!(result, None, "a Wayland session should skip the X11 query entirely");
    }

    #[test]
    fn key_pressed_reads_the_correct_bit_regardless_of_byte_boundary() {
        let mut keys = [0u8; 32];
        // Keycode 37 (Control_L) is bit 5 of byte 4.
        keys[4] = 1 << 5;
        assert!(key_pressed(&keys, KEYCODE_CONTROL_L));
        assert!(!key_pressed(&keys, KEYCODE_CONTROL_R));
        assert!(!key_pressed(&keys, KEYCODE_ALT_L));
    }

    #[test]
    fn key_pressed_detects_a_keycode_at_the_very_start_and_end_of_the_bitmap() {
        let mut keys = [0u8; 32];
        keys[0] = 1; // keycode 0
        keys[31] = 0b1000_0000; // keycode 255
        assert!(key_pressed(&keys, 0));
        assert!(key_pressed(&keys, 255));
        assert!(!key_pressed(&keys, 1));
    }

    #[test]
    fn poll_held_modifiers_is_none_on_a_wayland_session() {
        // SAFETY: test-only mutation of process env, restored immediately after.
        let previous = std::env::var_os("XDG_SESSION_TYPE");
        unsafe {
            std::env::set_var("XDG_SESSION_TYPE", "wayland");
        }
        let result = poll_held_modifiers();
        match previous {
            Some(value) => unsafe { std::env::set_var("XDG_SESSION_TYPE", value) },
            None => unsafe { std::env::remove_var("XDG_SESSION_TYPE") },
        }
        assert_eq!(result, None, "a Wayland session should skip the X11 query entirely");
    }

    #[test]
    fn force_activate_window_is_a_no_op_on_a_wayland_session() {
        // SAFETY: test-only mutation of process env, restored immediately after.
        let previous = std::env::var_os("XDG_SESSION_TYPE");
        unsafe {
            std::env::set_var("XDG_SESSION_TYPE", "wayland");
        }
        let result = force_activate_window(0);
        match previous {
            Some(value) => unsafe { std::env::set_var("XDG_SESSION_TYPE", value) },
            None => unsafe { std::env::remove_var("XDG_SESSION_TYPE") },
        }
        assert!(!result, "a Wayland session should skip the EWMH activation request entirely");
    }

    #[test]
    fn default_paths_prefers_xdg_data_home() {
        // SAFETY: test-only mutation of process env, restored immediately after.
        let previous = std::env::var_os("XDG_DATA_HOME");
        unsafe {
            std::env::set_var("XDG_DATA_HOME", "/tmp/custom-xdg");
        }
        let paths = default_paths().unwrap();
        match previous {
            Some(value) => unsafe { std::env::set_var("XDG_DATA_HOME", value) },
            None => unsafe { std::env::remove_var("XDG_DATA_HOME") },
        }
        assert_eq!(paths.applications_dir, PathBuf::from("/tmp/custom-xdg/applications"));
    }
}
