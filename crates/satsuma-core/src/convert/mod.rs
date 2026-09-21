//! The real Phase 2 conversion engine: one module per format family, a
//! shared error type, and shared collision-safe output naming. Replaces
//! Phase 0's `stub_conversion` byte-copy with real encodes/transcodes.
//!
//! Each family module (`image`, `video`, `audio`, `document`, `archive`) is
//! independent — there's no dependency between them, matching Phase 2's
//! "work proceeds on all of them in parallel" scope note — except that
//! cross-family JPG/PNG -> PDF/DOCX export is owned entirely by
//! [`document`] (this module's [`convert`] routes it there), so the writer
//! for that path only exists once.

pub mod archive;
pub mod audio;
pub mod document;
mod error;
pub mod image;
mod naming;
pub mod video;

use crate::file_type::{detect_category, extension_of, FileCategory};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Builds a command that launches `ffmpeg_bin` with stdin/stdout
/// silenced and stderr piped (so the conversion progress can be
/// read from the process without inheriting the terminal). On
/// Windows it also sets `CREATE_NO_WINDOW` so the bundled FFmpeg
/// sidecar does not pop up a console window behind the app.
pub(super) fn build_ffmpeg_command(ffmpeg_bin: &Path) -> Command {
    let mut cmd = Command::new(ffmpeg_bin);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW — https://learn.microsoft.com/windows/win32/procthread/process-creation-flags
        cmd.creation_flags(0x08000000);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    cmd
}

pub use error::ConvertError;
pub use naming::{output_dir_path, output_path};

/// The target extensions `source_path` can be converted to, already
/// excluding its own extension. Returns an empty list for a directory,
/// an extensionless path, or an otherwise-unrecognized format — the menu
/// shows nothing for those, per Interaction.md's "unsupported inputs are
/// rejected outright" rule.
pub fn supported_targets(source_path: &str) -> Vec<&'static str> {
    let Some(ext) = extension_of(source_path) else {
        return Vec::new();
    };
    family_targets(&ext).to_vec()
}

/// The target extensions several selected files can *all* convert to: the
/// intersection of each file's own [`supported_targets`], or empty if any
/// path is unrecognized (including a mismatched/mixed family with no
/// common targets) — per Interaction.md's multi-file selection rule.
/// Returns an empty list for an empty `source_paths`.
pub fn supported_targets_for_selection(source_paths: &[String]) -> Vec<&'static str> {
    let mut paths = source_paths.iter();
    let Some(first) = paths.next() else {
        return Vec::new();
    };
    let mut common: Vec<&'static str> = supported_targets(first);
    for path in paths {
        let targets = supported_targets(path);
        common.retain(|target| targets.contains(target));
        if common.is_empty() {
            break;
        }
    }
    common
}

fn family_targets(source_ext: &str) -> &'static [&'static str] {
    match detect_category(&format!("f.{source_ext}")) {
        FileCategory::Image => image::supported_targets(source_ext),
        FileCategory::Video => video::supported_targets(source_ext),
        FileCategory::Audio => audio::supported_targets(source_ext),
        FileCategory::Document => document::supported_targets(source_ext),
        FileCategory::Archive => archive::supported_targets(source_ext),
        FileCategory::Unknown => &[],
    }
}

/// Converts `source` to `target_extension`, computing a collision-safe
/// destination via [`output_path`] and routing to whichever family module
/// owns that (source, target) pair. `ffmpeg_bin` is only consulted for a
/// video/audio target; pass `None` when it's unavailable (e.g. the sidecar
/// binary wasn't found) — that turns any FFmpeg-routed conversion into an
/// [`ConvertError::EngineUnavailable`] instead of panicking. `on_progress`
/// receives a `0.0..=1.0` fraction when the engine can report one
/// (currently only FFmpeg); other families call it with `1.0` once, since
/// they complete too fast for meaningful intermediate progress.
///
/// `pdfium_lib_dir` is only consulted for a PDF-rasterization-dependent
/// conversion (PDF -> JPG/PNG, or the scanned-page image fallback of PDF
/// -> DOCX) — pass `None` when no pdfium library has been vendored; every
/// other document conversion (PDF -> TXT via text extraction, TXT ->
/// anything, SRT/VTT/TXT interconversion, JPG/PNG -> PDF/DOCX) needs no
/// native dependency at all and keeps working regardless.
///
/// Returns the path actually written on success (matching `destination`
/// for every family except the two that write somewhere of their own: a
/// multi-page PDF -> JPG/PNG export, which writes into a sibling folder
/// instead -- see [`document::convert`] -- and ZIP/TAR/RAR -> GZIP, which
/// writes a real `"<stem>.tar.gz"` -- see [`archive::convert`]).
pub fn convert(
    source: &Path,
    target_extension: &str,
    ffmpeg_bin: Option<&Path>,
    pdfium_lib_dir: Option<&Path>,
    on_progress: &mut dyn FnMut(f32),
) -> Result<PathBuf, ConvertError> {
    let source_str = source.to_str().ok_or_else(|| ConvertError::InvalidSource("path is not valid UTF-8".to_string()))?;
    let Some(source_ext) = extension_of(source_str) else {
        return Err(ConvertError::InvalidSource("source file has no extension".to_string()));
    };
    if !source.is_file() {
        return Err(ConvertError::InvalidSource(format!("not a file: {}", source.display())));
    }

    let target_extension = target_extension.to_lowercase();
    if !family_targets(&source_ext).contains(&target_extension.as_str()) {
        return Err(ConvertError::UnsupportedConversion {
            from: source_ext,
            to: target_extension,
        });
    }

    let destination = output_path(source, &target_extension);
    let category = detect_category(source_str);

    let result = match (category, target_extension.as_str()) {
        (FileCategory::Image, "pdf") | (FileCategory::Image, "docx") => {
            document::convert(source, &target_extension, &destination, pdfium_lib_dir)
        }
        (FileCategory::Image, _) => image::convert(source, &target_extension, &destination).map(|()| destination.clone()),
        (FileCategory::Video, _) => {
            let ffmpeg_bin = ffmpeg_bin.ok_or_else(|| ConvertError::EngineUnavailable("FFmpeg sidecar not found".to_string()))?;
            video::convert(ffmpeg_bin, source, &target_extension, &destination, on_progress).map(|()| destination.clone())
        }
        (FileCategory::Audio, _) => {
            let ffmpeg_bin = ffmpeg_bin.ok_or_else(|| ConvertError::EngineUnavailable("FFmpeg sidecar not found".to_string()))?;
            audio::convert(ffmpeg_bin, source, &target_extension, &destination, on_progress).map(|()| destination.clone())
        }
        // Document conversions report the path actually written, which
        // differs from `destination` for a multi-page PDF -> JPG/PNG (see
        // document::convert). Archive conversions likewise report the real
        // path, differing for a GZIP target (see archive::convert).
        (FileCategory::Document, _) => document::convert(source, &target_extension, &destination, pdfium_lib_dir),
        (FileCategory::Archive, _) => archive::convert(source, &target_extension, &destination),
        (FileCategory::Unknown, _) => Err(ConvertError::InvalidSource("unrecognized source format".to_string())),
    };

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_targets_is_empty_for_a_directory_style_or_extensionless_path() {
        assert_eq!(supported_targets("no_extension"), Vec::<&str>::new());
    }

    #[test]
    fn supported_targets_for_selection_is_empty_for_an_empty_list() {
        assert_eq!(supported_targets_for_selection(&[]), Vec::<&str>::new());
    }

    #[test]
    fn supported_targets_for_selection_intersects_across_files() {
        // jpg and png share every raster target (minus each other, since
        // each is excluded from its own list) plus pdf/docx, so the
        // intersection should be non-empty and contain neither jpg nor
        // png themselves.
        let same_family = vec!["a.jpg".to_string(), "b.png".to_string()];
        let intersected = supported_targets_for_selection(&same_family);
        assert!(!intersected.is_empty());
        assert!(!intersected.contains(&"jpg"));
        assert!(!intersected.contains(&"png"));
        assert!(intersected.contains(&"webp"));

        // An image plus an unrelated family (audio) shares nothing —
        // exercises the intersection mechanism's early-exit-on-empty path.
        let mixed_family = vec!["a.jpg".to_string(), "b.mp3".to_string()];
        assert_eq!(supported_targets_for_selection(&mixed_family), Vec::<&str>::new());
    }

    #[test]
    fn convert_rejects_a_target_not_in_the_source_format_matrix() {
        let dir = std::env::temp_dir().join(format!("satsuma-convert-mod-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("photo.jpg");
        std::fs::write(&source, b"fake jpg bytes").unwrap();

        let result = convert(&source, "zip", None, None, &mut |_| {});
        assert!(matches!(result, Err(ConvertError::UnsupportedConversion { .. })));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn convert_rejects_a_missing_source_file() {
        let result = convert(Path::new("/no/such/file.jpg"), "png", None, None, &mut |_| {});
        assert!(matches!(result, Err(ConvertError::UnsupportedConversion { .. }) | Err(ConvertError::InvalidSource(_))));
    }
}
