//! Resolves the vendored pdfium shared library's directory for
//! `satsuma_core::convert::document`'s PDF-rasterization path (PDF -> JPG/
//! PNG, and the scanned-page image fallback of PDF -> DOCX), which accepts
//! it as a plain `Option<&Path>` parameter rather than looking it up
//! itself, keeping that crate Tauri-free.
//!
//! Unlike the old FFmpeg sidecar (a Tauri `externalBin`), both FFmpeg
//! and pdfium are now `resources` entries (`tauri.conf.json`'s
//! `bundle.resources`), resolved at runtime via
//! [`tauri::path::PathResolver::resource_dir`] rather than sitting
//! next to the executable. Its source directory
//! (`src-tauri/lib/pdfium/`) is gitignored — no checked-in placeholder —
//! and tauri-codegen still validates that the path exists on every build,
//! so `scripts/fetch-pdfium.sh`/`.ps1` (which creates the directory and
//! downloads the real library into it) must run once per checkout before
//! any `cargo build`/`cargo check` of the satsuma crate; see AGENTS.md's
//! Commands section. Until that has happened, [`resolve_pdfium_lib_dir`]
//! resolves to `None` — graceful degradation, fully supported by
//! `convert::document` (everything except PDF rasterization keeps
//! working).
//!
//! Resolution order, first match wins: a user override from Settings
//! ([`Settings::pdfium_path`], surfaced in `SettingsScreen.tsx`'s
//! "External tools" section — a file path, matching how `ffmpeg_path`
//! points at a binary, even though `pdfium-render`'s own API wants a
//! directory; [`resolve_pdfium_lib_dir`] takes the file's parent). Like
//! `ffmpeg.rs`'s override, this takes over *entirely* rather than being
//! tried before the bundled resource — a broken explicit choice surfaces
//! as "not found" rather than silently falling back to a different
//! library than the one the user asked for. Then the bundled resource
//! location, then the dev-time location next to the built executable
//! (matching how `cargo tauri dev`/a raw `cargo build` don't go through
//! the installer's resource-copying step at all), and finally — Linux
//! only — the standard system library directories, so a machine with a
//! system-installed `libpdfium.so` still gets PDF rasterization even when
//! nothing was vendored, mirroring `ffmpeg.rs`'s PATH fallback. Windows
//! has no standard system pdfium (`pdfium.dll` ships with apps that
//! bundle it, never as a system component), so that last fallback is
//! absent there.

use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

const LIBRARY_FILE_NAME: &str = if cfg!(windows) { "pdfium.dll" } else { "libpdfium.so" };

/// The platform pdfium library's filename (`pdfium.dll` on Windows,
/// `libpdfium.so` elsewhere) — exposed so callers that need the library
/// *file* path (not just its directory, which is what
/// [`resolve_pdfium_lib_dir`] returns) don't have to duplicate this
/// platform check.
pub fn library_file_name() -> &'static str {
    LIBRARY_FILE_NAME
}

/// Returns the directory containing the pdfium shared library to use:
/// `override_path`'s parent directory if the user configured one in
/// Settings and it actually points at a file — checked *instead of*, not
/// before, the bundled resource, so a broken override reports "not found"
/// (see this module's doc comment) — otherwise the bundled resource
/// location, then a dev-time fallback next to the built executable, then
/// (Linux only) a system-installed library in the standard library
/// directories. `None` if none has been vendored and no system copy could
/// be found (a dev build that never ran `scripts/fetch-pdfium.sh`/`.ps1`
/// on a machine without a system pdfium).
pub fn resolve_pdfium_lib_dir(app: &AppHandle, override_path: Option<&str>) -> Option<PathBuf> {
    if let Some(path) = override_path {
        return resolve_override(path);
    }

    if let Ok(resource_dir) = app.path().resource_dir() {
        let candidate = resource_dir.join("pdfium");
        if is_real_library_dir(&candidate) {
            return Some(candidate);
        }
    }

    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let dev_candidate = exe_dir.join("pdfium");
    if is_real_library_dir(&dev_candidate) {
        return Some(dev_candidate);
    }

    find_system_pdfium_dir_in(&system_library_dirs())
}

/// `true` if `candidate` actually contains the platform pdfium library
/// file, as opposed to just being a directory that exists — the fetch
/// scripts' `mkdir` could have created the directory without the download
/// ever completing, and that must not be treated as a usable library.
fn is_real_library_dir(candidate: &Path) -> bool {
    candidate.join(LIBRARY_FILE_NAME).is_file()
}

/// The standard system library directories searched for a
/// system-installed pdfium by [`find_system_pdfium_dir_in`] — the last
/// fallback of [`resolve_pdfium_lib_dir`], mirroring `ffmpeg.rs`'s PATH
/// fallback. Linux only: Windows has no standard system pdfium
/// (`pdfium.dll` ships with apps that bundle it, never as a system
/// component), so the list is empty there. Covers the multiarch dir
/// (`/usr/lib/x86_64-linux-gnu`), the 64-bit dir (`/usr/lib64`), and the
/// plain `/usr/lib` plus `/usr/local/lib`.
fn system_library_dirs() -> Vec<PathBuf> {
    if cfg!(target_os = "linux") {
        ["/usr/local/lib", "/usr/lib/x86_64-linux-gnu", "/usr/lib64", "/usr/lib"]
            .iter()
            .map(PathBuf::from)
            .collect()
    } else {
        Vec::new()
    }
}

/// The first of `dirs` that actually contains the platform pdfium library
/// file — kept as a plain function (no `AppHandle` needed) so it's
/// unit-testable on its own with a temp directory standing in for the
/// system locations.
fn find_system_pdfium_dir_in(dirs: &[PathBuf]) -> Option<PathBuf> {
    dirs.iter().find(|dir| is_real_library_dir(dir)).cloned()
}

/// The override-path half of [`resolve_pdfium_lib_dir`], pulled out as a
/// plain function (no `AppHandle` needed) so it's unit-testable on its
/// own: `path` must point at a real file, and its parent directory is
/// returned as the library dir.
fn resolve_override(path: &str) -> Option<PathBuf> {
    let path = Path::new(path);
    if !path.is_file() {
        return None;
    }
    path.parent().map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_path_resolves_to_its_parent_directory() {
        let dir = std::env::temp_dir().join(format!("satsuma-pdfium-override-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fake_lib = dir.join("libpdfium.so");
        std::fs::write(&fake_lib, b"fake").unwrap();

        assert_eq!(resolve_override(fake_lib.to_str().unwrap()), Some(dir.clone()));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn override_path_that_does_not_exist_returns_none_without_falling_back() {
        assert_eq!(resolve_override("/no/such/libpdfium.so"), None);
    }

    #[test]
    fn override_path_pointing_at_a_directory_is_rejected() {
        let dir = std::env::temp_dir().join(format!("satsuma-pdfium-override-dir-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        assert_eq!(resolve_override(dir.to_str().unwrap()), None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn system_library_dirs_are_nonempty_only_on_linux() {
        let dirs = system_library_dirs();
        if cfg!(target_os = "linux") {
            assert!(dirs.contains(&PathBuf::from("/usr/lib")));
        } else {
            assert!(dirs.is_empty());
        }
    }

    #[test]
    fn find_system_pdfium_dir_in_picks_the_directory_containing_the_library() {
        let with_lib = std::env::temp_dir().join(format!("satsuma-pdfium-system-lib-test-{}", std::process::id()));
        std::fs::create_dir_all(&with_lib).unwrap();
        std::fs::write(with_lib.join(LIBRARY_FILE_NAME), b"fake").unwrap();

        let without_lib = std::env::temp_dir().join(format!("satsuma-pdfium-system-empty-test-{}", std::process::id()));
        std::fs::create_dir_all(&without_lib).unwrap();

        let dirs = [without_lib.clone(), with_lib.clone()];
        assert_eq!(find_system_pdfium_dir_in(&dirs), Some(with_lib.clone()));

        // A dir that merely exists (no library file in it - e.g. the fetch
        // scripts' mkdir without a completed download) is not a match.
        assert_eq!(find_system_pdfium_dir_in(std::slice::from_ref(&without_lib)), None);

        let _ = std::fs::remove_dir_all(&with_lib);
        let _ = std::fs::remove_dir_all(&without_lib);
    }
}