//! Watches Satsuma's themes directory (`~/.config/satsuma/themes/`) for
//! changes made outside the app itself — a user hand-editing, adding, or
//! deleting a theme file in a text editor or file manager — and calls back
//! (debounced) whenever anything in it changes, so the app notices without
//! needing a restart or a manual refresh. See `reconcile_themes_and_notify`
//! in `lib.rs` for what actually happens on that callback (re-listing
//! themes, recovering a deleted Citrus, and falling the active theme back
//! to Citrus if it's the one that disappeared).

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

/// How long to wait after the first event in a burst before acting, so one
/// logical change — most editors save via a temp-file write plus a rename,
/// which `notify` reports as several raw events — is handled once rather
/// than several times in a row.
const DEBOUNCE: Duration = Duration::from_millis(200);

/// Spawns a background thread that watches `dir` (non-recursively — theme
/// files are never nested) and calls `on_change` once per debounced burst
/// of filesystem activity in it. Silently does nothing if `dir` can't be
/// watched (e.g. it doesn't exist yet at startup) — Settings' own
/// theme-loading commands create it independently of this watcher, so a
/// later save/list call still works even if watching itself failed here.
pub fn watch(dir: PathBuf, on_change: impl Fn() + Send + 'static) {
    std::thread::spawn(move || {
        let (tx, rx) = mpsc::channel::<notify::Result<notify::Event>>();
        let Ok(mut watcher) =
            RecommendedWatcher::new(move |result| { let _ = tx.send(result); }, notify::Config::default())
        else {
            return;
        };

        if watcher.watch(&dir, RecursiveMode::NonRecursive).is_err() {
            return;
        }

        loop {
            let Ok(first) = rx.recv() else {
                // The sender half is gone, which only happens if `watcher`
                // itself was dropped — nothing more to watch.
                return;
            };
            // Drain the rest of this burst before acting once.
            while rx.recv_timeout(DEBOUNCE).is_ok() {}
            if first.is_ok() {
                on_change();
            }
        }
    });
}
