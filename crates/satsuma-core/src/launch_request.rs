//! Parsing of `--mode=formats|tools <path>...` launch arguments into a
//! [`LaunchRequest`], shared by the Linux context-menu launcher and the
//! Windows Explorer-selection overlay path.

use crate::file_type::{detect_category, FileCategory};
use serde::{Deserialize, Serialize};

/// Which wedge menu to open: convertible formats, or advanced tools.
/// Mirrors the frontend's `MenuMode` type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MenuMode {
    /// Convertible output formats for the dropped/selected file(s).
    Formats,
    /// Advanced tools (compress, crop, trim, split, merge).
    Tools,
}

/// A request to open the wedge menu for a set of files, arriving from
/// outside the normal drag-and-drop flow: a file-manager context-menu
/// invocation (Linux) passes this via CLI args, and the Windows
/// Explorer-selection overlay passes it directly in-process.
///
/// `category` is computed here, Rust-side, from `paths[0]` rather than left
/// for the frontend to determine via an async `detect_file_category` call.
/// It used to be: the overlay window is shown immediately on the Rust side
/// (see `show_overlay` in `src-tauri/src/lib.rs`), and only afterwards did
/// the frontend asynchronously resolve the category and render the wedge
/// menu — leaving a real window, however brief, where the overlay was
/// visible, focused, and catching clicks, but rendering nothing. Computing
/// the category up front (a plain extension check, no I/O) closes that gap
/// entirely, and also lets the caller skip showing the overlay at all for a
/// genuinely unsupported file type instead of showing a permanently-blank
/// one — see the `category` check around `show_overlay`'s call sites.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchRequest {
    /// Which wedge menu to open.
    pub mode: MenuMode,
    /// The file path(s) the request was made for, in the order they were
    /// given.
    pub paths: Vec<String>,
    /// Category of `paths[0]`, precomputed so the frontend doesn't need an
    /// async round-trip before it can render the wedge menu.
    pub category: FileCategory,
}

/// Parses `--mode=formats|tools` plus one or more trailing file paths out of
/// process arguments (excluding `argv[0]`, the program name). Returns `None`
/// if no paths were given, since that's just a normal launch with no
/// associated file (e.g. the user double-clicked the app icon).
///
/// `--mode` defaults to `formats` when omitted, and may appear anywhere
/// among the arguments (context-menu launchers put flags before or after
/// paths depending on the OS/file manager).
pub fn parse_launch_args<I, S>(args: I) -> Option<LaunchRequest>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut mode = MenuMode::Formats;
    let mut paths = Vec::new();

    for arg in args {
        let arg = arg.as_ref();
        match arg.strip_prefix("--mode=") {
            Some("tools") => mode = MenuMode::Tools,
            Some("formats") => mode = MenuMode::Formats,
            Some(_) => {} // unrecognized mode value: ignore, keep default
            None => paths.push(arg.to_string()),
        }
    }

    if paths.is_empty() {
        None
    } else {
        let category = detect_category(&paths[0]);
        Some(LaunchRequest { mode, paths, category })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_none_for_no_paths() {
        assert_eq!(parse_launch_args(Vec::<&str>::new()), None);
        assert_eq!(parse_launch_args(["--mode=tools"]), None);
    }

    #[test]
    fn defaults_to_formats_mode() {
        assert_eq!(
            parse_launch_args(["photo.png"]),
            Some(LaunchRequest {
                mode: MenuMode::Formats,
                paths: vec!["photo.png".to_string()],
                category: FileCategory::Image,
            })
        );
    }

    #[test]
    fn parses_explicit_tools_mode() {
        assert_eq!(
            parse_launch_args(["--mode=tools", "clip.mp4"]),
            Some(LaunchRequest {
                mode: MenuMode::Tools,
                paths: vec!["clip.mp4".to_string()],
                category: FileCategory::Video,
            })
        );
    }

    #[test]
    fn mode_flag_can_come_after_paths() {
        assert_eq!(
            parse_launch_args(["clip.mp4", "--mode=tools"]),
            Some(LaunchRequest {
                mode: MenuMode::Tools,
                paths: vec!["clip.mp4".to_string()],
                category: FileCategory::Video,
            })
        );
    }

    #[test]
    fn collects_multiple_paths_in_order() {
        let result = parse_launch_args(["--mode=formats", "a.png", "b.jpg"]).unwrap();
        assert_eq!(result.paths, vec!["a.png".to_string(), "b.jpg".to_string()]);
    }

    #[test]
    fn unrecognized_mode_value_falls_back_to_default() {
        assert_eq!(
            parse_launch_args(["--mode=bogus", "a.png"]),
            Some(LaunchRequest {
                mode: MenuMode::Formats,
                paths: vec!["a.png".to_string()],
                category: FileCategory::Image,
            })
        );
    }

    #[test]
    fn category_is_computed_from_the_first_path() {
        let result = parse_launch_args(["presentation.pptx", "other.pptx"]).unwrap();
        assert_eq!(result.category, FileCategory::Unknown);

        let result = parse_launch_args(["bundle.zip"]).unwrap();
        assert_eq!(result.category, FileCategory::Archive);

        let result = parse_launch_args(["song.mp3"]).unwrap();
        assert_eq!(result.category, FileCategory::Audio);
    }
}
