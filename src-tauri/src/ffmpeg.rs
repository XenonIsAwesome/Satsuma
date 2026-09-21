//! Resolves the vendored FFmpeg sidecar binary's on-disk path for
//! the video/audio conversion families in `satsuma-core`, which
//! accept it as a plain `&Path` parameter (see
//! `convert::video`/`convert::audio`) rather than looking it up
//! themselves, keeping that crate Tauri-free.
//!
//! Unlike the Phase 0 approach of installing a bundled FFmpeg into
//! `/usr/bin/ffmpeg` (which is what Tauri's `externalBin`
//! bundling does — the binary ends up on the system PATH, e.g. at
//! /usr/bin/ffmpeg on Debian), FFmpeg is now a `resources` entry
//! `bundle.resources`), populated by `scripts/fetch-ffmpeg.sh`/`.ps1`
//! before a real packaged build. Tauri maps `lib/ffmpeg` into the
//! packaged app's resource directory at `ffmpeg`, so the binary
//! lives at `<resource-dir>/ffmpeg/ffmpeg-<triple>` — e.g.
//! `/usr/lib/satsuma/ffmpeg/ffmpeg-x86_64-unknown-linux-gnu` in the
//! Debian package, `C:\Program Files\Satsuma\resources\ffmpeg\...`
//! in the Windows installer, `target/debug/ffmpeg/...` in a dev
//! build — together with its GPL-3 license (fetched by the script,
//! installed as `COPYING`).
//!
//! Resolution order, first match wins: a Settings override (takes
//! over entirely, like `pdfium.rs`'s — a broken explicit choice
//! surfaces as "not found" rather than silently substituting a
//! different binary), then the bundled resource location, then
//! the dev-build location next to the executable (matching how
//! `cargo tauri dev`/a raw `cargo build` don't go through the
//! installer's resource-copying step at all), and finally a
//! system-installed `ffmpeg` on `PATH` as a dev convenience only
//! (production installs always bundle the sidecar). `None` if none
//! was found.
//!
//! A sidecar candidate is only *adopted* when it actually runs
//! (`ffmpeg -version` succeeds), not merely when a file exists at
//! that path: a truncated download, a wrong-architecture binary, or
//! any other file sitting at a candidate location would otherwise
//! be adopted, shadowing the PATH fallback and turning every
//! video/audio conversion (and the Settings status line) into a
//! spawn failure against something that isn't a working FFmpeg.
//!
//! The sidecar itself is gitignored (see `src-tauri/.gitignore`)
//! and created by `scripts/fetch-ffmpeg.sh`/`.ps1`, which must run
//! once per checkout before any `cargo build` of the satsuma crate:
//! tauri-build validates the `resources` path on every build, and a
//! fresh checkout has no file there at all.

use std::path::PathBuf;
use tauri::{AppHandle, Manager};

// Satsuma only ships for these two platforms (see AGENTS.md), each
// built for one target triple in CI/release - matching
// scripts/fetch-ffmpeg.sh's (`x86_64-unknown-linux-gnu`) and
// fetch-ffmpeg.ps1's (`x86_64-pc-windows-msvc`) sidecar naming.
#[cfg(target_os = "windows")]
const TARGET_TRIPLE: &str = "x86_64-pc-windows-msvc";
#[cfg(target_os = "linux")]
const TARGET_TRIPLE: &str = "x86_64-unknown-linux-gnu";
#[cfg(not(any(target_os = "windows", target_os = "linux")))]
const TARGET_TRIPLE: &str = "unknown";

/// Returns the FFmpeg binary to use: `override_path` if the user
/// configured one in Settings — checked *instead of*, not before,
/// the usual fallbacks, so a broken override reports "not found"
/// rather than silently using a different binary (see this
/// module's doc comment) — otherwise the bundled copy in the
/// resource directory, then the dev-build location next to the
/// executable, or a system `ffmpeg` on `PATH` as a dev-only
/// fallback. `None` if nothing usable was found at all.
pub fn resolve_ffmpeg_bin(app: &AppHandle, override_path: Option<&str>) -> Option<PathBuf> {
    if let Some(path) = override_path {
        return resolve_override(path);
    }

    let exe_suffix = if cfg!(windows) { ".exe" } else { "" };

    // Packaged app: <resource-dir>/ffmpeg/
    if let Ok(resource_dir) = app.path().resource_dir() {
        let ffmpeg_dir = resource_dir.join("ffmpeg");
        let candidates = [
            ffmpeg_dir.join(format!("ffmpeg{exe_suffix}")),
            ffmpeg_dir.join(format!("ffmpeg-{TARGET_TRIPLE}{exe_suffix}")),
        ];
        if let Some(found) = first_runnable(&candidates) {
            return Some(found);
        }
    }

    // Dev build: resources copied next to the executable by
    // tauri-build (target/debug/ffmpeg/...). Kept for convenience
    // but not the primary path — a manual copy sitting here is
    // indistinguishable from a truncated download until probed.
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let dev_candidates = [
        exe_dir.join(format!("ffmpeg{exe_suffix}")),
        exe_dir.join(format!("ffmpeg-{TARGET_TRIPLE}{exe_suffix}")),
    ];
    if let Some(found) = first_runnable(&dev_candidates) {
        return Some(found);
    }

    which_on_path("ffmpeg")
}

/// The override-path half of [`resolve_ffmpeg_bin`], pulled out as a plain
/// function (no `AppHandle` needed, mirroring `pdfium.rs`'s
/// `resolve_override`) so it's unit-testable on its own without going
/// through Tauri's `mock_app()`: `path` is adopted only when it is actually
/// runnable — `is_file()` alone would adopt an inert or wrong-arch file,
/// and then the whole surface downstream (the Settings status line, every
/// video/audio conversion) would fail at spawn time with no indication the
/// *override itself* is broken. The same probe every other candidate gets:
/// `ffmpeg -version`.
fn resolve_override(path: &str) -> Option<PathBuf> {
    let path = PathBuf::from(path);
    is_runnable_ffmpeg(&path).then_some(path)
}

/// The first candidate that is an actually-runnable FFmpeg, or `None`.
/// Candidates are only adopted when they execute — plain file
/// existence would accept a broken file sitting at a candidate
/// location (a truncated fetch, a wrong-arch binary), which would
/// then be used for conversions and Settings status instead of the
/// working system `ffmpeg` the PATH fallback provides.
fn first_runnable(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|path| is_runnable_ffmpeg(path)).cloned()
}

/// `true` if `candidate` actually starts and exits successfully. Probing
/// `-version` (the same check `satsuma-core`'s conversion test helpers
/// use) rather than trusting `is_file()`: an existing file can still be a
/// broken stub — wrong architecture, missing DLL, or a truncated
/// download — indistinguishable from a working binary until you run it.
fn is_runnable_ffmpeg(candidate: &std::path::Path) -> bool {
    let mut cmd = std::process::Command::new(candidate);
    // CREATE_NO_WINDOW — same as satsuma-core's build_ffmpeg_command, so
    // the `-version` probe that happens during resolution (and the PATH
    // fallback in `which_on_path`) doesn't flash a console window on
    // Windows even for a wrong-arch/broken stub that exits instantly.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd.arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// A minimal `which`-alike: searches `PATH` for an executable named
/// `name` (with `.exe` appended on Windows), so dev builds without a
/// fetched sidecar can still exercise real conversions against a
/// system-installed FFmpeg. Not used for anything else, so a tiny local
/// implementation is simpler than pulling in the `which` crate for one
/// call site. Like the sidecar candidates, only a *runnable* PATH
/// binary is accepted, so a broken `ffmpeg` earlier on `PATH` can't be
/// selected over nothing.
fn which_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    let exe_name = if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
    std::env::split_paths(&path_var).map(|dir| dir.join(&exe_name)).find(|candidate| is_runnable_ffmpeg(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_path_pointing_at_an_inert_file_is_rejected() {
        let dir = std::env::temp_dir().join(format!("satsuma-ffmpeg-override-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fake_bin = dir.join("my-ffmpeg");
        std::fs::write(&fake_bin, b"fake").unwrap();

        // An override that exists but isn't runnable is indistinguishable
        // from a missing one — both resolve to `None` (see this module's
        // doc comment), never a silently-adopted inert file.
        assert_eq!(resolve_override(fake_bin.to_str().unwrap()), None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn override_path_is_adopted_when_it_points_at_a_runnable_ffmpeg() {
        use std::os::unix::fs::PermissionsExt;

        // Needs a real ffmpeg to build a runnable stand-in; skip cleanly on
        // a machine without one (same pattern as the satsuma-core suites).
        let real = PathBuf::from("/usr/bin/ffmpeg");
        if !real.is_file() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("satsuma-ffmpeg-override-runnable-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // A shell shim that execs the real binary: exists AND runs, so the
        // probe adopts it.
        let working = dir.join("my-ffmpeg");
        std::fs::write(&working, format!("#!/bin/sh\nexec \"{}\" \"$@\"\n", real.display())).unwrap();
        std::fs::set_permissions(&working, std::fs::Permissions::from_mode(0o755)).unwrap();

        assert_eq!(resolve_override(working.to_str().unwrap()), Some(working));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn override_path_that_does_not_exist_returns_none_without_falling_back() {
        // Deliberately doesn't fall back to the bundled/system lookup even
        // though one might exist on this machine — a broken explicit
        // override should surface as "not found," not silently substitute
        // a different binary (see this module's doc comment).
        assert_eq!(resolve_override("/no/such/ffmpeg-binary-at-all"), None);
    }

    #[test]
    fn override_path_pointing_at_a_directory_is_rejected() {
        let dir = std::env::temp_dir().join(format!("satsuma-ffmpeg-override-dir-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        assert_eq!(resolve_override(dir.to_str().unwrap()), None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn placeholder_text_file_is_not_adopted_as_a_sidecar_candidate() {
        // The dev-build trap the runnability probe exists for: a
        // non-executable stub sitting at a candidate location (what a
        // truncated fetch or a wrong-arch binary looks like)
        // must not be adopted as the FFmpeg sidecar.
        let dir = std::env::temp_dir().join(format!("satsuma-ffmpeg-placeholder-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let placeholder = dir.join("ffmpeg");
        std::fs::write(&placeholder, b"inert placeholder").unwrap();
        // Give it the exec bit on unix so the rejection is about
        // *runnability*, not just a missing exec permission.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&placeholder, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        assert_eq!(first_runnable(std::slice::from_ref(&placeholder)), None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn first_runnable_candidate_wins_over_a_dead_placeholder() {
        use std::os::unix::fs::PermissionsExt;

        // Needs a real ffmpeg to build a runnable stand-in; skip cleanly on
        // a machine without one (same pattern as the satsuma-core suites).
        let real = PathBuf::from("/usr/bin/ffmpeg");
        if !real.is_file() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("satsuma-ffmpeg-skip-placeholder-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let placeholder = dir.join("dead-ffmpeg");
        std::fs::write(&placeholder, b"inert placeholder").unwrap();
        // A shell shim that execs the real binary: exists AND runs, so the
        // probe adopts it while skipping the earlier dead candidate.
        let working = dir.join("working-ffmpeg");
        std::fs::write(&working, format!("#!/bin/sh\nexec \"{}\" \"$@\"\n", real.display())).unwrap();
        std::fs::set_permissions(&placeholder, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::set_permissions(&working, std::fs::Permissions::from_mode(0o755)).unwrap();

        let candidates = [placeholder.clone(), working.clone()];
        assert_eq!(first_runnable(&candidates), Some(working));

        let _ = std::fs::remove_dir_all(&dir);
    }
}