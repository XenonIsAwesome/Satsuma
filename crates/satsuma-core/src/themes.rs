//! Theme files: every Satsuma theme — built-in or user-made — is just a
//! name plus the same five color tokens (`CustomColors`), saved as one JSON
//! file under `~/.config/satsuma/themes/`. Phase 1.1 replaces the earlier
//! fixed `ThemePreset` enum with this so a theme is data anyone can add to
//! that directory, not a hardcoded list — see `src-tauri/src/theme_store.rs`
//! for the actual directory I/O this module's types and pure helpers back.

use crate::settings::CustomColors;
use serde::{Deserialize, Serialize};

/// One theme's saved contents — the file's name (sans extension) is its id,
/// resolved by the Tauri-side `theme_store` module, not stored redundantly
/// inside the file itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeFile {
    /// User-facing display name (not necessarily the same as the id derived
    /// from it via [`slugify`]).
    pub name: String,
    /// The theme's five color tokens.
    pub colors: CustomColors,
}

/// Turns a user-provided theme name into a safe, filesystem- and URL-safe
/// id: lowercased, non-alphanumeric runs collapsed to a single `-`, leading/
/// trailing `-` trimmed. Falls back to `"theme"` if that leaves nothing
/// (e.g. a name that's entirely punctuation/emoji).
pub fn slugify(name: &str) -> String {
    let mut slug = String::with_capacity(name.len());
    let mut last_was_dash = false;
    for ch in name.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash && !slug.is_empty() {
            slug.push('-');
            last_was_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "theme".to_string()
    } else {
        slug
    }
}

/// The four themes Satsuma seeds `~/.config/satsuma/themes/` with on first
/// run (see `theme_store::ensure_seeded`), straight out of DESIGN.md's
/// theme table — id, display name, and the five user-settable tokens each
/// literally uses (their own `primary-active` isn't one of these five, but
/// every theme's `primary-active` is derived the same way at run time
/// frontend-side regardless of whether it's a seeded default or
/// user-made, so the built-ins don't need to special-case it here).
pub fn builtin_themes() -> Vec<(String, ThemeFile)> {
    vec![
        (
            "citrus".to_string(),
            ThemeFile {
                name: "Citrus".to_string(),
                colors: CustomColors {
                    primary: "#ff7a29".to_string(),
                    secondary: "#2b2620".to_string(),
                    neutral: "#f6f3ee".to_string(),
                    surface: "#ffffff".to_string(),
                    outline: "#eae4d9".to_string(),
                },
            },
        ),
        (
            "midnight-citrus".to_string(),
            ThemeFile {
                name: "Midnight Citrus".to_string(),
                colors: CustomColors {
                    primary: "#ff9452".to_string(),
                    secondary: "#f5efe6".to_string(),
                    neutral: "#1b1815".to_string(),
                    surface: "#262220".to_string(),
                    outline: "#3a342e".to_string(),
                },
            },
        ),
        (
            "yuzu".to_string(),
            ThemeFile {
                name: "Yuzu".to_string(),
                colors: CustomColors {
                    primary: "#d4a017".to_string(),
                    secondary: "#26241c".to_string(),
                    neutral: "#f7f5ec".to_string(),
                    surface: "#ffffff".to_string(),
                    outline: "#e8e2d0".to_string(),
                },
            },
        ),
        (
            "blood-orange".to_string(),
            ThemeFile {
                name: "Blood Orange".to_string(),
                colors: CustomColors {
                    primary: "#c4432a".to_string(),
                    secondary: "#241914".to_string(),
                    neutral: "#f5eeea".to_string(),
                    surface: "#ffffff".to_string(),
                    outline: "#e6d9d2".to_string(),
                },
            },
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugifies_a_simple_name() {
        assert_eq!(slugify("My Theme"), "my-theme");
    }

    #[test]
    fn collapses_punctuation_runs_and_trims_edges() {
        assert_eq!(slugify("  Wow!! Cool -- Theme??  "), "wow-cool-theme");
    }

    #[test]
    fn falls_back_to_theme_for_an_all_punctuation_name() {
        assert_eq!(slugify("!!!"), "theme");
        assert_eq!(slugify(""), "theme");
    }

    #[test]
    fn builtin_themes_have_unique_ids() {
        let ids: Vec<String> = builtin_themes().into_iter().map(|(id, _)| id).collect();
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(ids.len(), unique.len());
    }

    #[test]
    fn builtin_theme_ids_are_already_valid_slugs() {
        for (id, _) in builtin_themes() {
            assert_eq!(slugify(&id), id);
        }
    }
}
