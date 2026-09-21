//! App-wide, user-configurable preferences: the modifier-key combinations
//! that trigger the format/tools wedge menus, and the active color theme.
//! Kept independent of the Tauri command layer so the validation rules are
//! unit testable on their own — see `src-tauri/src/settings_store.rs` for
//! how this gets loaded from / saved to disk, and `src-tauri/src/lib.rs`'s
//! `save_settings` command for where `validate_hotkeys` is enforced
//! before anything is persisted or applied.

use crate::keys::is_supported_key;
use serde::{Deserialize, Serialize};

/// A hotkey binding: the four modifier keys Windows/Linux desktop
/// environments expose (mirroring Tangerine's Control/Option/Shift/Command
/// on macOS), plus optionally one additional non-modifier key (see
/// `crate::keys` for the supported set) — any nonempty combination is a
/// valid trigger per the design vault's "Interaction" doc, "Modifier keys" section in the
/// design vault, generalized in Phase 1.2 from modifiers-only to "any
/// combination or singular key on the keyboard."
///
/// **Windows' Explorer-drag global keyboard hook only ever matches on the
/// four modifiers** — `key` is ignored there (see
/// `windows_integration::matched_mode`) — because that hook runs
/// system-wide, before Satsuma's own window has focus, on code that can
/// only be type-checked, never run, in this project's dev environment.
/// Extending *that specific path* to arbitrary keys was deliberately left
/// out as a follow-up rather than risking system-wide keyboard-hook
/// behavior nobody could verify. A binding with `key` set still works
/// everywhere else: in-app drag-and-drop, the desktop overlay's tools
/// live-toggle, and Linux (which never had a system-wide hook to begin
/// with, so there's no equivalent restriction there).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyCombo {
    /// Control key held.
    pub ctrl: bool,
    /// Alt (Option on macOS) key held.
    pub alt: bool,
    /// Shift key held.
    pub shift: bool,
    /// The Windows key on Windows/Linux (Command on macOS, not a target
    /// platform for Satsuma, but kept generically named to match the
    /// frontend's use of the DOM's "Meta" key name).
    pub meta: bool,
    /// An additional non-modifier key — e.g. `"KeyP"`, `"F5"`, `"Digit3"`,
    /// using the same physical-position identifiers as the DOM's
    /// `KeyboardEvent.code` (see `crate::keys`) — or `None` for a
    /// modifiers-only binding (Phase 1's original, still fully supported,
    /// shape). A binding can be a single standalone key (all four
    /// modifiers false, `key` set) or any modifier combination plus one
    /// more key.
    pub key: Option<String>,
}

impl HotkeyCombo {
    /// Builds a modifiers-only combo (`key` left unset).
    pub const fn new(ctrl: bool, alt: bool, shift: bool, meta: bool) -> Self {
        Self { ctrl, alt, shift, meta, key: None }
    }

    /// Returns a copy of this combo with `key` set — `key` must be one of
    /// `crate::keys`' supported codes, or this is a no-op (returns `self`
    /// unchanged), since an unrecognized key could never actually be
    /// detected by either platform's polling.
    pub fn with_key(mut self, key: impl Into<String>) -> Self {
        let key = key.into();
        if is_supported_key(&key) {
            self.key = Some(key);
        }
        self
    }

    /// True if no modifier is held and no additional key is set — a combo
    /// that could never actually trigger anything.
    pub fn is_empty(&self) -> bool {
        !self.ctrl && !self.alt && !self.shift && !self.meta && self.key.is_none()
    }
}

/// Shift alone opens the format menu — matches Tangerine's Shift default
/// and Phase 0's previously-hardcoded behavior.
pub const DEFAULT_FORMAT_MODIFIERS: HotkeyCombo = HotkeyCombo::new(false, false, true, false);
/// Shift+Alt opens the tools menu — matches Tangerine's Shift+Option
/// default and Phase 0's previously-hardcoded behavior.
pub const DEFAULT_TOOLS_MODIFIERS: HotkeyCombo = HotkeyCombo::new(false, true, true, false);

/// The five user-set tokens every theme file defines — every other token
/// (`primary-active`, the `on-*` pairings) is derived from these at run
/// time, frontend-side, per DESIGN.md's "Custom theme behavior" (which
/// Phase 1.1 generalizes from a Custom-only mechanism to how *every* theme,
/// built-in or user-made, is defined — see `crate::themes`). Stored as
/// `#rrggbb` hex strings; Rust only persists these opaquely and never
/// interprets them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomColors {
    /// Primary brand/accent color, as `#rrggbb`.
    pub primary: String,
    /// Secondary accent color, as `#rrggbb`.
    pub secondary: String,
    /// Neutral (text/background-adjacent) color, as `#rrggbb`.
    pub neutral: String,
    /// Surface (card/panel background) color, as `#rrggbb`.
    pub surface: String,
    /// Outline/border color, as `#rrggbb`.
    pub outline: String,
}

/// The id (theme filename stem, see `crate::themes::slugify`) Satsuma seeds
/// `~/.config/satsuma/themes/` with for its own default look.
pub const DEFAULT_THEME_ID: &str = "citrus";

/// All of Satsuma's app-wide, user-configurable preferences — a single
/// personalization surface per Phase 1's "one unified Settings screen"
/// requirement, rather than settings scattered across ad hoc panels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Combination that opens the format wedge menu.
    pub format_modifiers: HotkeyCombo,
    /// Combination that opens the advanced-tools wedge menu.
    pub tools_modifiers: HotkeyCombo,
    /// The id of the saved theme file (under `~/.config/satsuma/themes/`)
    /// currently applied.
    pub active_theme_id: String,
    /// An in-progress, not-yet-saved color tweak layered on top of
    /// `active_theme_id`'s own colors — cleared once the tweak is either
    /// saved as a new theme file (which becomes the new `active_theme_id`)
    /// or abandoned by picking a different theme.
    pub color_overrides: Option<CustomColors>,
    /// User-configured override path to the FFmpeg binary (video/audio
    /// conversion — see `crate::convert::video`/`audio`). `None` (the
    /// default) resolves to the bundled sidecar, falling back to a
    /// system-installed `ffmpeg` on `PATH` — see `src-tauri/src/ffmpeg.rs`.
    /// An override that no longer points at a real file is *not* silently
    /// ignored in favor of that fallback — see `ffmpeg.rs`'s doc comment
    /// for why a broken explicit choice should surface as a clear error
    /// instead.
    pub ffmpeg_path: Option<String>,
    /// User-configured override path to the pdfium shared library file
    /// (PDF -> JPG/PNG rasterization — see `crate::convert::document`).
    /// `None` (the default) resolves to the vendored resource, if any —
    /// see `src-tauri/src/pdfium.rs`. Same non-silent-fallback behavior as
    /// `ffmpeg_path` above for a broken override.
    pub pdfium_path: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            format_modifiers: DEFAULT_FORMAT_MODIFIERS,
            tools_modifiers: DEFAULT_TOOLS_MODIFIERS,
            active_theme_id: DEFAULT_THEME_ID.to_string(),
            color_overrides: None,
            ffmpeg_path: None,
            pdfium_path: None,
        }
    }
}

/// Enforces Phase 1's two hard rules for the hotkey bindings: neither may
/// be empty (a trigger has to actually require holding/pressing something —
/// a modifier, a plain key, or both), and the two bindings may not be
/// identical (otherwise nothing would ever distinguish the format menu from
/// the tools menu). Soft heuristic warnings (e.g. a combination that
/// collides with a well-known OS shortcut) are surfaced frontend-side
/// instead, since Rust has no portable way to query "is this already a
/// global shortcut for something else" across Windows/Linux desktop
/// environments.
pub fn validate_hotkeys(format: &HotkeyCombo, tools: &HotkeyCombo) -> Result<(), String> {
    if format.is_empty() {
        return Err("the format-menu combination can't be empty".to_string());
    }
    if tools.is_empty() {
        return Err("the tools-menu combination can't be empty".to_string());
    }
    if format == tools {
        return Err("the format-menu and tools-menu combinations can't be identical".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_phase_0s_previously_hardcoded_shift_and_shift_alt() {
        let settings = Settings::default();
        assert_eq!(settings.format_modifiers, HotkeyCombo::new(false, false, true, false));
        assert_eq!(settings.tools_modifiers, HotkeyCombo::new(false, true, true, false));
        assert_eq!(settings.active_theme_id, DEFAULT_THEME_ID);
        assert_eq!(settings.color_overrides, None);
    }

    #[test]
    fn empty_combo_is_empty() {
        assert!(HotkeyCombo::new(false, false, false, false).is_empty());
        assert!(!HotkeyCombo::new(false, false, false, true).is_empty());
    }

    #[test]
    fn default_bindings_are_valid() {
        assert!(validate_hotkeys(&DEFAULT_FORMAT_MODIFIERS, &DEFAULT_TOOLS_MODIFIERS).is_ok());
    }

    #[test]
    fn rejects_an_empty_format_combo() {
        let empty = HotkeyCombo::new(false, false, false, false);
        let error = validate_hotkeys(&empty, &DEFAULT_TOOLS_MODIFIERS).unwrap_err();
        assert!(error.contains("format"));
    }

    #[test]
    fn rejects_an_empty_tools_combo() {
        let empty = HotkeyCombo::new(false, false, false, false);
        let error = validate_hotkeys(&DEFAULT_FORMAT_MODIFIERS, &empty).unwrap_err();
        assert!(error.contains("tools"));
    }

    #[test]
    fn rejects_identical_bindings() {
        let combo = HotkeyCombo::new(true, false, false, false);
        let error = validate_hotkeys(&combo, &combo).unwrap_err();
        assert!(error.contains("identical"));
    }

    #[test]
    fn allows_a_binding_that_is_a_superset_of_the_other() {
        // Shift vs. Shift+Alt: different exact combinations, even though one
        // physically holds the other's keys too — matching is done by exact
        // pressed-set equality, not subset containment (see useModifierKeys
        // frontend-side), so this must be accepted.
        let shift = HotkeyCombo::new(false, false, true, false);
        let shift_alt = HotkeyCombo::new(false, true, true, false);
        assert!(validate_hotkeys(&shift, &shift_alt).is_ok());
    }

    #[test]
    fn with_key_sets_a_supported_key() {
        let combo = HotkeyCombo::new(true, false, false, false).with_key("KeyP");
        assert_eq!(combo.key.as_deref(), Some("KeyP"));
        assert!(!combo.is_empty());
    }

    #[test]
    fn with_key_ignores_an_unrecognized_key() {
        let combo = HotkeyCombo::new(true, false, false, false).with_key("NumpadEnter");
        assert_eq!(combo.key, None);
    }

    #[test]
    fn a_lone_key_with_no_modifiers_is_not_empty() {
        let combo = HotkeyCombo::new(false, false, false, false).with_key("KeyP");
        assert!(!combo.is_empty());
    }

    #[test]
    fn two_combos_with_the_same_modifiers_but_different_keys_are_not_identical() {
        let p = HotkeyCombo::new(true, false, false, false).with_key("KeyP");
        let o = HotkeyCombo::new(true, false, false, false).with_key("KeyO");
        assert!(validate_hotkeys(&p, &o).is_ok());
    }

    #[test]
    fn settings_round_trip_through_json() {
        let settings = Settings {
            format_modifiers: HotkeyCombo::new(true, false, false, false).with_key("KeyP"),
            tools_modifiers: HotkeyCombo::new(true, false, true, false),
            active_theme_id: "my-theme".to_string(),
            color_overrides: Some(CustomColors {
                primary: "#123456".to_string(),
                secondary: "#654321".to_string(),
                neutral: "#ffffff".to_string(),
                surface: "#eeeeee".to_string(),
                outline: "#dddddd".to_string(),
            }),
            ffmpeg_path: Some("/opt/ffmpeg/bin/ffmpeg".to_string()),
            pdfium_path: None,
        };
        let json = serde_json::to_string(&settings).unwrap();
        let round_tripped: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(settings, round_tripped);
    }
}
