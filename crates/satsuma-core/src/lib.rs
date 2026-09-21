#![deny(missing_docs)]
//! Core conversion engine for Satsuma.
//!
//! Kept independent of the Tauri command layer (`src-tauri`) so it can be
//! unit tested on its own and extended with new formats/tools without
//! touching UI plumbing. Phase 0 only implements file-type detection; later
//! phases add the actual image/video/audio conversion and advanced tools
//! (compress, crop, trim, split, merge) here.

pub mod convert;
pub mod file_type;
pub mod keys;
pub mod launch_request;
pub mod overlay_geometry;
pub mod settings;
pub mod themes;

pub use convert::{convert, output_dir_path, output_path, supported_targets, supported_targets_for_selection, ConvertError};
pub use file_type::{detect_category, FileCategory};
pub use keys::{
    code_for_windows_vk, code_for_x11_keycode, is_supported_key, windows_vk_for_code, windows_vks,
    x11_keycode_for_code, x11_keycodes,
};
pub use launch_request::{parse_launch_args, LaunchRequest, MenuMode};
pub use overlay_geometry::{overlay_position, ScreenRect};
pub use settings::{
    validate_hotkeys, CustomColors, HotkeyCombo, Settings, DEFAULT_FORMAT_MODIFIERS, DEFAULT_THEME_ID,
    DEFAULT_TOOLS_MODIFIERS,
};
pub use themes::{builtin_themes, slugify, ThemeFile};
