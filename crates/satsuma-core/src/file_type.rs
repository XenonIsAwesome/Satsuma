//! File-extension-based classification of a dropped/selected file into
//! image/video/audio/unknown, driving which wedge menu the frontend offers.

use serde::{Deserialize, Serialize};

/// Broad category a dropped file falls into, driving which wedge menu
/// (formats/tools) the frontend should offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileCategory {
    /// JPG/PNG/WebP/HEIC/HEIF/TIFF/AVIF/BMP/SVG (SVG is input-only — never
    /// offered as a conversion target, see `convert::image`).
    Image,
    /// MP4/MOV/MKV/WebM/AVI/WMV/GIF (GIF is a video-family format for
    /// conversion purposes, per Tools.md).
    Video,
    /// MP3/M4A/WAV/FLAC/OGG/Opus/AIFF/WMA.
    Audio,
    /// PDF, plus the TXT-linked text/subtitle formats (TXT/SRT/VTT) — see
    /// Tools.md's Documents/Text/Subtitles sections, which all interlink
    /// through TXT as the common format.
    Document,
    /// ZIP/TAR/GZIP/RAR.
    Archive,
    /// Extension didn't match any known category, or the path had no
    /// extension at all.
    Unknown,
}

const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "webp", "heic", "heif", "tiff", "tif", "avif", "bmp", "svg",
];
const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mov", "mkv", "webm", "avi", "wmv", "gif"];
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "m4a", "wav", "flac", "ogg", "opus", "aiff", "aif", "wma"];
const DOCUMENT_EXTENSIONS: &[&str] = &["pdf", "txt", "srt", "vtt"];
const ARCHIVE_EXTENSIONS: &[&str] = &["zip", "tar", "gz", "tgz", "rar"];

/// Returns the lowercased extension of a path (without the leading dot),
/// or `None` if the path has no extension.
///
/// Deliberately string-based rather than `std::path::Path`-based: the input
/// comes straight from OS drag-and-drop paths (which may use either `/` or
/// `\` separators on Windows vs. Linux) and we only ever need the suffix.
pub fn extension_of(path: &str) -> Option<String> {
    let file_name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let (name, ext) = file_name.rsplit_once('.')?;
    if name.is_empty() {
        // Dotfiles like ".gitignore" have no extension in this sense.
        return None;
    }
    if ext.is_empty() {
        return None;
    }
    Some(ext.to_lowercase())
}

/// Classifies a file path into a [`FileCategory`] based on its extension.
pub fn detect_category(path: &str) -> FileCategory {
    let Some(ext) = extension_of(path) else {
        return FileCategory::Unknown;
    };
    let ext = ext.as_str();

    if IMAGE_EXTENSIONS.contains(&ext) {
        FileCategory::Image
    } else if VIDEO_EXTENSIONS.contains(&ext) {
        FileCategory::Video
    } else if AUDIO_EXTENSIONS.contains(&ext) {
        FileCategory::Audio
    } else if DOCUMENT_EXTENSIONS.contains(&ext) {
        FileCategory::Document
    } else if ARCHIVE_EXTENSIONS.contains(&ext) {
        FileCategory::Archive
    } else {
        FileCategory::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_every_listed_image_extension() {
        for ext in IMAGE_EXTENSIONS {
            let path = format!("/home/user/photo.{ext}");
            assert_eq!(detect_category(&path), FileCategory::Image, "failed for {ext}");
        }
    }

    #[test]
    fn detects_every_listed_video_extension() {
        for ext in VIDEO_EXTENSIONS {
            let path = format!("C:\\Users\\me\\clip.{ext}");
            assert_eq!(detect_category(&path), FileCategory::Video, "failed for {ext}");
        }
    }

    #[test]
    fn detects_every_listed_audio_extension() {
        for ext in AUDIO_EXTENSIONS {
            let path = format!("song.{ext}");
            assert_eq!(detect_category(&path), FileCategory::Audio, "failed for {ext}");
        }
    }

    #[test]
    fn detects_every_listed_document_extension() {
        for ext in DOCUMENT_EXTENSIONS {
            let path = format!("notes.{ext}");
            assert_eq!(detect_category(&path), FileCategory::Document, "failed for {ext}");
        }
    }

    #[test]
    fn detects_every_listed_archive_extension() {
        for ext in ARCHIVE_EXTENSIONS {
            let path = format!("bundle.{ext}");
            assert_eq!(detect_category(&path), FileCategory::Archive, "failed for {ext}");
        }
    }

    #[test]
    fn extension_matching_is_case_insensitive() {
        assert_eq!(detect_category("IMAGE.PNG"), FileCategory::Image);
        assert_eq!(detect_category("Clip.MoV"), FileCategory::Video);
        assert_eq!(detect_category("Track.WAV"), FileCategory::Audio);
        assert_eq!(detect_category("REPORT.PDF"), FileCategory::Document);
        assert_eq!(detect_category("Bundle.ZIP"), FileCategory::Archive);
    }

    #[test]
    fn unknown_extension_is_unknown() {
        assert_eq!(detect_category("presentation.pptx"), FileCategory::Unknown);
        assert_eq!(detect_category("spreadsheet.xlsx"), FileCategory::Unknown);
        assert_eq!(detect_category("compressed.7z"), FileCategory::Unknown);
    }

    #[test]
    fn no_extension_is_unknown() {
        assert_eq!(detect_category("README"), FileCategory::Unknown);
        assert_eq!(detect_category("/home/user/Makefile"), FileCategory::Unknown);
    }

    #[test]
    fn dotfile_with_no_real_extension_is_unknown() {
        assert_eq!(detect_category(".gitignore"), FileCategory::Unknown);
    }

    #[test]
    fn windows_style_paths_are_handled() {
        assert_eq!(
            detect_category("C:\\Users\\me\\Pictures\\vacation.jpeg"),
            FileCategory::Image
        );
    }

    #[test]
    fn extension_of_strips_the_dot_and_lowercases() {
        assert_eq!(extension_of("a/b/c.JPG"), Some("jpg".to_string()));
        assert_eq!(extension_of("no_extension"), None);
        assert_eq!(extension_of(".hidden"), None);
    }
}
