//! Reads/writes Satsuma's `Settings` (see `satsuma_core::settings`) as JSON
//! on disk. Kept as plain functions over a `Path` rather than an `AppHandle`
//! so the actual read/write/parse logic is unit testable against a temp
//! directory without needing a running Tauri app — mirroring
//! `linux_integration`'s `IntegrationPaths`/`install`/`uninstall` split
//! between "where" (Tauri-resolved) and "what" (plain, testable).

use satsuma_core::Settings;
use std::io;
use std::path::{Path, PathBuf};

const SETTINGS_FILE_NAME: &str = "settings.json";

pub fn settings_file_path(config_dir: &Path) -> PathBuf {
    config_dir.join(SETTINGS_FILE_NAME)
}

/// Loads `Settings` from `path`, falling back to `Settings::default()` if
/// the file doesn't exist yet (first run) or fails to parse (e.g. a future
/// version wrote a shape this build doesn't understand) — a corrupt or
/// missing settings file should never prevent the app from starting.
pub fn load_settings_from(path: &Path) -> Settings {
    fs_read_to_string(path).and_then(|raw| serde_json::from_str(&raw).ok()).unwrap_or_default()
}

fn fs_read_to_string(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// Writes `settings` as JSON to `path`, creating the parent directory (the
/// app config dir) if it doesn't exist yet.
pub fn save_settings_to(path: &Path, settings: &Settings) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(settings)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    std::fs::write(path, json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use satsuma_core::HotkeyCombo;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let id = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir()
                .join(format!("satsuma-settings-store-test-{}-{}", std::process::id(), id));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn load_returns_defaults_when_no_file_exists_yet() {
        let dir = TempDir::new();
        let path = settings_file_path(&dir.0);
        assert_eq!(load_settings_from(&path), Settings::default());
    }

    #[test]
    fn load_returns_defaults_for_unparseable_content() {
        let dir = TempDir::new();
        let path = settings_file_path(&dir.0);
        std::fs::write(&path, "not json").unwrap();
        assert_eq!(load_settings_from(&path), Settings::default());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = TempDir::new();
        let path = settings_file_path(&dir.0);
        let settings = Settings {
            format_modifiers: HotkeyCombo::new(true, false, false, false),
            tools_modifiers: HotkeyCombo::new(true, false, true, false),
            active_theme_id: "yuzu".to_string(),
            color_overrides: None,
            ffmpeg_path: Some("/usr/local/bin/ffmpeg".to_string()),
            pdfium_path: Some("/usr/local/lib/libpdfium.so".to_string()),
        };

        save_settings_to(&path, &settings).unwrap();

        assert_eq!(load_settings_from(&path), settings);
    }

    #[test]
    fn save_creates_the_config_directory_if_missing() {
        let dir = TempDir::new();
        let nested = dir.0.join("nested/config/dir");
        let path = settings_file_path(&nested);

        save_settings_to(&path, &Settings::default()).unwrap();

        assert!(path.exists());
    }
}
