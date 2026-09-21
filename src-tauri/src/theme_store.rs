//! Reads/writes Satsuma's theme files as JSON under
//! `~/.config/satsuma/themes/` (see `config_base_dir` in `lib.rs`) — one
//! file per theme, its filename stem (see `satsuma_core::slugify`) is its
//! id. Kept as plain functions over a `Path`, mirroring `settings_store`'s
//! and `linux_integration`'s split between "where" (Tauri-resolved) and
//! "what" (plain, unit testable against a temp directory).

use satsuma_core::{builtin_themes, slugify, ThemeFile, DEFAULT_THEME_ID};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub fn themes_dir(config_base_dir: &Path) -> PathBuf {
    config_base_dir.join("themes")
}

fn theme_file_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.json"))
}

/// Populates `dir` with Satsuma's built-in themes the first time it's ever
/// needed (i.e. the directory doesn't exist yet) — after that, the
/// directory is entirely the user's: deleting or editing a file (including
/// a built-in one) is never undone by a later run.
pub fn ensure_seeded(dir: &Path) -> io::Result<()> {
    if dir.exists() {
        return Ok(());
    }
    fs::create_dir_all(dir)?;
    for (id, theme) in builtin_themes() {
        write_theme_file(dir, &id, &theme)?;
    }
    Ok(())
}

fn write_theme_file(dir: &Path, id: &str, theme: &ThemeFile) -> io::Result<()> {
    let json = serde_json::to_string_pretty(theme).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    fs::write(theme_file_path(dir, id), json)
}

/// Ensures Citrus specifically exists in `dir`, recreating it from its
/// hardcoded built-in values (see `satsuma_core::builtin_themes`) if its
/// file is missing. Unlike `ensure_seeded` — which only seeds the built-ins
/// once, on a directory that doesn't exist yet, and never undoes a
/// deletion — Citrus is Satsuma's hard fallback (`Settings::active_theme_id`
/// falls back to it whenever the active theme disappears, see
/// `lib.rs`'s `load_settings_and_themes`), so it must always be
/// recoverable even if its own file is deleted. A no-op if it already
/// exists, so a user's own edits to `citrus.json` are never overwritten.
pub fn ensure_citrus_seeded(dir: &Path) -> io::Result<()> {
    if theme_file_path(dir, DEFAULT_THEME_ID).exists() {
        return Ok(());
    }
    let Some((id, citrus)) = builtin_themes().into_iter().find(|(id, _)| id == DEFAULT_THEME_ID) else {
        return Ok(());
    };
    fs::create_dir_all(dir)?;
    write_theme_file(dir, &id, &citrus)
}

/// Whether `id` refers to one of `themes`' own ids — used to decide
/// whether the active theme has disappeared (deleted by hand, or caught by
/// the directory watcher) and Settings should fall back to Citrus.
pub fn theme_exists(themes: &[(String, ThemeFile)], id: &str) -> bool {
    themes.iter().any(|(theme_id, _)| theme_id == id)
}

/// Every theme file in `dir`, as `(id, ThemeFile)` pairs — unparseable or
/// non-`.json` files are silently skipped rather than failing the whole
/// listing, since a user's own stray/malformed file in this directory
/// shouldn't take down Settings' theme picker. Ordered by id for a stable,
/// predictable dropdown.
pub fn list_themes(dir: &Path) -> Vec<(String, ThemeFile)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut themes: Vec<(String, ThemeFile)> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("json"))
        .filter_map(|entry| {
            let id = entry.path().file_stem()?.to_str()?.to_string();
            let raw = fs::read_to_string(entry.path()).ok()?;
            let theme: ThemeFile = serde_json::from_str(&raw).ok()?;
            Some((id, theme))
        })
        .collect();

    themes.sort_by(|a, b| a.0.cmp(&b.0));
    themes
}

/// Saves `theme` under a new id derived from its own name, made unique
/// against whatever's already in `dir` (appending `-2`, `-3`, ... on
/// collision, the same collision-safe convention `stub_conversion` uses for
/// output filenames). Returns the id it was actually saved under.
pub fn save_theme(dir: &Path, theme: &ThemeFile) -> io::Result<String> {
    fs::create_dir_all(dir)?;
    let base_id = slugify(&theme.name);
    let mut id = base_id.clone();
    let mut suffix = 2;
    while theme_file_path(dir, &id).exists() {
        id = format!("{base_id}-{suffix}");
        suffix += 1;
    }
    write_theme_file(dir, &id, theme)?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use satsuma_core::CustomColors;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let id = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path =
                std::env::temp_dir().join(format!("satsuma-theme-store-test-{}-{}", std::process::id(), id));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn sample_theme(name: &str) -> ThemeFile {
        ThemeFile {
            name: name.to_string(),
            colors: CustomColors {
                primary: "#123456".to_string(),
                secondary: "#654321".to_string(),
                neutral: "#ffffff".to_string(),
                surface: "#eeeeee".to_string(),
                outline: "#dddddd".to_string(),
            },
        }
    }

    #[test]
    fn ensure_seeded_populates_an_empty_directory_with_the_built_ins() {
        let dir = TempDir::new();
        let themes_dir = dir.0.join("themes");

        ensure_seeded(&themes_dir).unwrap();

        let listed = list_themes(&themes_dir);
        assert_eq!(listed.len(), builtin_themes().len());
        assert!(listed.iter().any(|(id, _)| id == "citrus"));
    }

    #[test]
    fn ensure_seeded_is_a_no_op_if_the_directory_already_exists() {
        let dir = TempDir::new();
        let themes_dir = dir.0.join("themes");
        fs::create_dir_all(&themes_dir).unwrap();
        // Simulates a user who deleted every seeded file but kept the
        // directory itself — re-seeding must not undo that.

        ensure_seeded(&themes_dir).unwrap();

        assert!(list_themes(&themes_dir).is_empty());
    }

    #[test]
    fn list_themes_returns_empty_for_a_missing_directory() {
        let dir = TempDir::new();
        assert_eq!(list_themes(&dir.0.join("does-not-exist")), Vec::new());
    }

    #[test]
    fn list_themes_skips_unparseable_files() {
        let dir = TempDir::new();
        fs::write(dir.0.join("broken.json"), "not json").unwrap();
        fs::write(dir.0.join("notes.txt"), "ignore me").unwrap();
        assert_eq!(list_themes(&dir.0), Vec::new());
    }

    #[test]
    fn save_theme_writes_a_slugified_file_and_round_trips() {
        let dir = TempDir::new();
        let theme = sample_theme("My Cool Theme");

        let id = save_theme(&dir.0, &theme).unwrap();

        assert_eq!(id, "my-cool-theme");
        let listed = list_themes(&dir.0);
        assert_eq!(listed, vec![(id, theme)]);
    }

    #[test]
    fn save_theme_disambiguates_a_name_collision() {
        let dir = TempDir::new();
        let first = sample_theme("Dup");
        let second = sample_theme("Dup");

        let first_id = save_theme(&dir.0, &first).unwrap();
        let second_id = save_theme(&dir.0, &second).unwrap();

        assert_eq!(first_id, "dup");
        assert_eq!(second_id, "dup-2");
    }

    #[test]
    fn save_theme_creates_the_directory_if_missing() {
        let dir = TempDir::new();
        let themes_dir = dir.0.join("nested/themes");

        save_theme(&themes_dir, &sample_theme("Fresh")).unwrap();

        assert!(themes_dir.exists());
    }

    #[test]
    fn ensure_citrus_seeded_recreates_a_deleted_citrus_file() {
        let dir = TempDir::new();
        let themes_dir = dir.0.join("themes");
        ensure_seeded(&themes_dir).unwrap();
        fs::remove_file(themes_dir.join("citrus.json")).unwrap();
        assert!(!theme_exists(&list_themes(&themes_dir), "citrus"));

        ensure_citrus_seeded(&themes_dir).unwrap();

        let listed = list_themes(&themes_dir);
        assert!(theme_exists(&listed, "citrus"));
        // The other three built-ins the user didn't touch stay deleted-or-not
        // exactly as they were — only Citrus is force-recovered.
        assert_eq!(listed.len(), builtin_themes().len());
    }

    #[test]
    fn ensure_citrus_seeded_never_overwrites_an_existing_citrus_file() {
        let dir = TempDir::new();
        let customized = sample_theme("Citrus");
        save_theme(&dir.0, &customized).unwrap(); // writes citrus.json (slugified from "Citrus")

        ensure_citrus_seeded(&dir.0).unwrap();

        let listed = list_themes(&dir.0);
        assert_eq!(listed, vec![("citrus".to_string(), customized)]);
    }

    #[test]
    fn ensure_citrus_seeded_creates_the_directory_if_missing() {
        let dir = TempDir::new();
        let themes_dir = dir.0.join("nested/themes");

        ensure_citrus_seeded(&themes_dir).unwrap();

        assert!(theme_exists(&list_themes(&themes_dir), "citrus"));
    }

    #[test]
    fn theme_exists_checks_by_id() {
        let themes = vec![("citrus".to_string(), sample_theme("Citrus"))];
        assert!(theme_exists(&themes, "citrus"));
        assert!(!theme_exists(&themes, "yuzu"));
    }
}
