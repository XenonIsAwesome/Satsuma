//! Collision-safe output naming, shared by every conversion family. Carried
//! over unchanged from Phase 0's `stub_conversion` (same rule: `cat.jpg` ->
//! `cat 2.jpg` -> `cat 3.jpg`, never silently overwriting an existing
//! file) — only what gets written into the file changed in Phase 2, not
//! where it's written.

use std::path::{Path, PathBuf};

/// Computes a collision-safe destination path for converting `source` to
/// `target_extension`: same directory and base name as `source`, with the
/// extension swapped in. If that path already exists, falls back to
/// `"<name> 2.<ext>"`, `"<name> 3.<ext>"`, and so on, until an unused path
/// is found.
pub fn output_path(source: &Path, target_extension: &str) -> PathBuf {
    let dir = source.parent().unwrap_or_else(|| Path::new(""));
    let stem = source.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
    unique_output_path(dir, stem, target_extension)
}

/// The collision-safe destination a conversion writes to, given the output
/// base name *independently* of `source`'s file stem. Same naming rule as
/// [`output_path`] — `"<stem>.<ext>"`, then `"<stem> 2.<ext>"` on collision
/// — but with the stem supplied by the caller, for the one family that
/// must derive it differently from the source's own filename: the archive
/// family strips *both* suffixes of a gzipped tarball (`backup.tar.gz` ->
/// stem `backup`, so converting it yields `backup.tar`, never the
/// single-stripped `backup.tar.tar`). See [`crate::convert::archive`]'s
/// "double suffix" rule.
pub(crate) fn unique_output_path(dir: &Path, stem: &str, target_extension: &str) -> PathBuf {
    unique_path(&dir.join(format!("{stem}.{target_extension}")))
}

/// Computes a collision-safe sibling *directory* next to `source`, named
/// `"<stem> <label>"` (e.g. `Report.pdf` + `"JPG Pages"` -> `Report JPG
/// Pages/`, then `Report JPG Pages 2/` on collision) — used by
/// multi-page-output conversions such as PDF -> JPG/PNG, which write one
/// file per page rather than a single sibling file.
pub fn output_dir_path(source: &Path, label: &str) -> PathBuf {
    let dir = source.parent().unwrap_or_else(|| Path::new(""));
    let stem = source.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
    unique_path(&dir.join(format!("{stem} {label}")))
}

/// Returns `candidate` unchanged if nothing exists there yet, otherwise
/// appends an incrementing numeric suffix (before the extension, if any)
/// until an unused path is found.
fn unique_path(candidate: &Path) -> PathBuf {
    if !candidate.exists() {
        return candidate.to_path_buf();
    }

    let dir = candidate.parent().unwrap_or_else(|| Path::new(""));
    let stem = candidate.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
    let extension = candidate.extension().and_then(|e| e.to_str());

    let mut suffix = 2u32;
    loop {
        let name = match extension {
            Some(ext) => format!("{stem} {suffix}.{ext}"),
            None => format!("{stem} {suffix}"),
        };
        let next = dir.join(name);
        if !next.exists() {
            return next;
        }
        suffix += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A fresh, empty temp directory for one test, cleaned up on drop so
    /// tests never interfere with each other's collision checks.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let id = COUNTER.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir().join(format!("satsuma-naming-test-{}-{id}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn swaps_the_extension_when_the_target_does_not_exist() {
        let dir = TempDir::new();
        let source = dir.path().join("cat.jpg");
        std::fs::write(&source, b"fake jpg bytes").unwrap();

        assert_eq!(output_path(&source, "png"), dir.path().join("cat.png"));
    }

    #[test]
    fn appends_a_numeric_suffix_on_collision() {
        let dir = TempDir::new();
        let source = dir.path().join("cat.jpg");
        std::fs::write(&source, b"fake jpg bytes").unwrap();
        std::fs::write(dir.path().join("cat.png"), b"already here").unwrap();

        assert_eq!(output_path(&source, "png"), dir.path().join("cat 2.png"));
    }

    #[test]
    fn keeps_incrementing_past_multiple_collisions() {
        let dir = TempDir::new();
        let source = dir.path().join("cat.jpg");
        std::fs::write(&source, b"fake jpg bytes").unwrap();
        std::fs::write(dir.path().join("cat.png"), b"1").unwrap();
        std::fs::write(dir.path().join("cat 2.png"), b"2").unwrap();
        std::fs::write(dir.path().join("cat 3.png"), b"3").unwrap();

        assert_eq!(output_path(&source, "png"), dir.path().join("cat 4.png"));
    }

    #[test]
    fn preserves_the_full_base_name_including_spaces() {
        let dir = TempDir::new();
        let source = dir.path().join("holiday photo.jpeg");
        std::fs::write(&source, b"fake jpeg bytes").unwrap();

        assert_eq!(output_path(&source, "webp"), dir.path().join("holiday photo.webp"));
    }

    #[test]
    fn output_dir_path_names_a_sibling_folder_and_avoids_collisions() {
        let dir = TempDir::new();
        let source = dir.path().join("Report.pdf");
        std::fs::write(&source, b"fake pdf bytes").unwrap();

        assert_eq!(output_dir_path(&source, "JPG Pages"), dir.path().join("Report JPG Pages"));

        std::fs::create_dir_all(dir.path().join("Report JPG Pages")).unwrap();
        assert_eq!(
            output_dir_path(&source, "JPG Pages"),
            dir.path().join("Report JPG Pages 2")
        );
    }
}
