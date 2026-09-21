//! Windows-only: fail-closed guard for the single-instance guarantee.
//!
//! `tauri-plugin-single-instance` 2.4.4 (the version this app depends on)
//! has a known, upstream-unfixed race on Windows
//! (<https://github.com/tauri-apps/plugins-workspace/issues/3587>, fix
//! proposed in <https://github.com/tauri-apps/plugins-workspace/pull/3495>):
//! its plugin setup creates a named mutex (`{identifier}-sim`) *before*
//! the WM_COPYDATA event-target window (`{identifier}-sic` /
//! `{identifier}-siw`) that receives forwarded invocations. A second
//! process whose `FindWindowW` happens to run before the primary has
//! registered that window misses it, falls through the plugin's
//! already-exists branch without forwarding or exiting, and boots as a
//! full second instance — which then fights the primary over WebView2
//! resources (or silently becomes "the" instance if the primary died
//! first, as it did on a real windows-latest CI run where a no-arg launch
//! crashed ~1s after start): the single-instance guarantee silently
//! breaks.
//!
//! This module closes that hole *before* the Tauri runtime boots, using
//! the workaround the upstream issue thread recommends: a second named
//! mutex only this app owns. Whoever creates it first is the one true
//! primary; every later process sees `ERROR_ALREADY_EXISTS` and waits a
//! bounded time for the primary's plugin IPC window to appear rather than
//! just proceeding — if the window shows up the process proceeds into the
//! normal Tauri bootstrap, where the plugin (now racing nothing) finds the
//! window, forwards its argv, and exits; if the primary dies, the mutex
//! wait reports it and the waiting process takes over as the new primary;
//! and if neither happens inside the deadline, the process exits rather
//! than ever booting an unguarded second instance (fail closed).
//!
//! See `src-tauri/src/lib.rs`'s `run()` for the call site. Linux has no
//! equivalent logic: its plugin implementation doesn't share this race
//! window (and the whole module is `cfg`-gated off there anyway).

use std::time::{Duration, Instant};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    GetLastError, ERROR_ALREADY_EXISTS, WAIT_ABANDONED, WAIT_OBJECT_0,
};
use windows::Win32::System::Threading::{CreateMutexW, WaitForSingleObject};
use windows::Win32::UI::WindowsAndMessaging::FindWindowW;

/// How long a secondary process waits for the primary's single-instance
/// IPC window to appear before giving up and exiting without booting.
///
/// Deliberately longer than the e2e-windows harness's original 5s
/// second-process exit wait, but short enough that a genuinely-broken
/// handshake surfaces quickly instead of hanging the process forever.
const GUARD_DEADLINE: Duration = Duration::from_secs(5);

/// Length of each `WaitForSingleObject` probe between `FindWindowW`
/// attempts: the primary's guard mutex stays owned for its whole lifetime,
/// so this is both a bounded sleep while it boots and a liveness check —
/// a `WAIT_OBJECT_0`/`WAIT_ABANDONED` return (which also transfers mutex
/// ownership to us) means the primary has died and we should take over.
const GUARD_POLL_MS: u32 = 200;

/// Encodes `text` as a NUL-terminated UTF-16 buffer for the Win32 `*W`
/// calls below. The identifier comes from `tauri.conf.json` (ASCII), but
/// the suffixing/encoding is identical to how `tauri-plugin-single-instance`
/// builds its own names, so the two can never disagree on the actual
/// named objects.
fn encode_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Enforces the single-instance guarantee *before* the Tauri runtime
/// builds its plugins or windows. Called at the very top of `run()` on
/// Windows, with the app's config `identifier` (the same string the
/// single-instance plugin derives its own mutex/window names from).
///
/// # Behavior
///
/// - **Primary** (we created the guard mutex): returns immediately; the
///   handle is kept open (never `CloseHandle`d) for this process's whole
///   lifetime so the mutex object survives and any later process knows a
///   primary exists.
/// - **Secondary** (mutex already exists): waits up to
///   [`GUARD_DEADLINE`] for the primary's plugin IPC window to appear,
///   then returns so the normal Tauri bootstrap can forward our argv via
///   the plugin and exit — or, if the primary dies during the wait, takes
///   over as the new primary — or, on timeout, prints a diagnostic and
///   exits with status 0 rather than ever running unguarded. The function
///   only ever *returns* in the first two cases.
pub fn enforce(identifier: &str) {
    let guard_name = encode_wide(&format!("{identifier}-sim-satsuma-guard"));
    let class_name = encode_wide(&format!("{identifier}-sic"));
    let window_name = encode_wide(&format!("{identifier}-siw"));

    let handle = unsafe {
        CreateMutexW(None, true, PCWSTR::from_raw(guard_name.as_ptr()))
            .expect("CreateMutexW failed for the single-instance guard mutex")
    };
    // CreateMutexW returns a valid handle even when the named mutex already
    // exists (a handle *to* the existing object) — it is not an error
    // return, so the existence signal is GetLastError, exactly the pattern
    // tauri-plugin-single-instance's own platform_impl/windows.rs relies on.
    let already_exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;

    if !already_exists {
        // We created the guard mutex: this is the primary instance. The
        // returned `HANDLE` is a plain Copy wrapper in the windows crate —
        // dropping it closes nothing; the kernel mutex object simply stays
        // alive (and owned by this thread) for as long as this process
        // runs, which is exactly the lifetime we want, so intentionally
        // keep it unclosed forever rather than storing it somewhere.
        let _ = handle;
        return;
    }

    let deadline = Instant::now() + GUARD_DEADLINE;
    loop {
        // The primary's single-instance plugin registers its WM_COPYDATA
        // event-target window (registered class `{id}-sic`, window name
        // `{id}-siw`) during Tauri's plugin setup, shortly after its own
        // `{id}-sim` mutex already exists. FindWindowW finding it proves
        // the primary is far enough along to receive our forwarded argv,
        // so proceeding into the Tauri bootstrap is safe: the plugin will
        // see ERROR_ALREADY_EXISTS on its own mutex, find this same
        // window, send our argv via WM_COPYDATA, and exit(0) — the
        // healthy handshake.
        if let Ok(_hwnd) = unsafe {
            FindWindowW(
                PCWSTR::from_raw(class_name.as_ptr()),
                PCWSTR::from_raw(window_name.as_ptr()),
            )
        } {
            return;
        }

        // Bounded sleep + primary-liveness probe in one: the primary holds
        // the guard mutex for its whole lifetime, so while it lives this
        // waits `GUARD_POLL_MS` and returns WAIT_TIMEOUT (loop and probe
        // FindWindowW again); when it dies the wait returns
        // WAIT_OBJECT_0/WAIT_ABANDONED, transferring ownership to us —
        // take over as the new primary. WAIT_FAILED (invalid handle,
        // should be impossible) falls through to the deadline like any
        // timeout, which is the fail-closed direction.
        let outcome = unsafe { WaitForSingleObject(handle, GUARD_POLL_MS) };
        if outcome == WAIT_OBJECT_0 || outcome == WAIT_ABANDONED {
            let _ = handle;
            return;
        }

        if Instant::now() >= deadline {
            eprintln!(
                "satsuma: another instance exists but never registered its single-instance \
                 IPC window within {GUARD_DEADLINE:?}; refusing to boot a second copy \
                 (fail-closed single-instance guard, upstream bug: \
                 https://github.com/tauri-apps/plugins-workspace/issues/3587)"
            );
            std::process::exit(0);
        }
    }
}
