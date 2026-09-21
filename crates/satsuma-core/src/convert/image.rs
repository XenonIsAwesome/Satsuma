//! Image <-> image conversion (JPG/PNG/WebP/HEIC/TIFF/AVIF/BMP, plus SVG as
//! an input-only source), via the `image` crate for raster decode/encode
//! and `resvg`/`usvg`/`tiny-skia` for SVG rasterization. Cross-family
//! JPG/PNG -> PDF/DOCX export lives in [`super::document`], not here — this
//! module only ever writes another raster image format.
//!
//! ## Known gaps (real, verified decisions — not silent stubs)
//!
//! **HEIC/HEIF decoding is not implemented.** The only viable Rust decoder
//! is `libheif-rs`, a binding to the native `libheif` C library. This
//! sandbox has `libheif1` (the runtime shared library) installed but not
//! `libheif-dev` (headers/pkg-config), and even where present, requiring
//! `libheif` conflicts with Satsuma shipping without system dependencies on
//! both Windows and Linux. `supported_targets` still lists the full raster
//! set for a `.heic`/`.heif` source (per the format matrix spec — this
//! keeps the format offered in the wedge menu rather than hidden), but
//! [`convert`] returns [`ConvertError::EngineUnavailable`] for any HEIC/HEIF
//! source rather than faking success.
//!
//! **AVIF decoding is also not implemented**, for the same class of reason,
//! verified directly rather than assumed: the `image` crate's AVIF
//! *encoding* (the `avif` feature) is genuinely pure Rust (`ravif`/`rav1e`,
//! BSD-3-Clause) and is fully supported here. AVIF *decoding*, however,
//! requires the separate `avif-native` feature, which pulls in `dav1d` (a
//! binding to the C `libdav1d`). Its build script (`dav1d-sys`) first tries
//! `pkg-config` (no `dav1d.pc`/headers are installed in this sandbox), and
//! otherwise `git clone`s dav1d from source at build time and builds it
//! with `meson`+`ninja`+`nasm` — none of which are installed here, and a
//! network fetch during `cargo build` is a non-starter for this project
//! regardless. A pure-Rust AV1 decoder does exist (`rav1d`,
//! memorysafety.org, BSD-2-Clause), but it ships as a C-ABI static library
//! product (`crate-type = ["staticlib"]`) meant to replace `libdav1d.so` at
//! the link level, not as a Rust API matching `image`'s hardcoded
//! `dav1d::Decoder` usage — wiring it in would mean writing a new decoder
//! backend for the `image` crate, well beyond this module's scope. So,
//! unlike HEIC, AVIF is deliberately **not** listed as a source in
//! [`supported_targets`] at all (an always-EngineUnavailable menu entry
//! would be a worse UX than not offering it) — it remains fully usable as
//! a *target* for every other raster format.

use super::error::ConvertError;
use crate::file_type::extension_of;
use image::{DynamicImage, ImageFormat, ImageReader};
use std::path::Path;

/// Extensions this module can produce from `source_ext` (already
/// lowercased), excluding `source_ext` itself.
pub fn supported_targets(source_ext: &str) -> &'static [&'static str] {
    match normalize(source_ext) {
        // JPG/PNG additionally export to the document family's PDF/DOCX
        // writer (see `convert::mod`'s dispatcher) — no other raster format
        // gets that per the format matrix spec.
        "jpg" => &["png", "webp", "tiff", "avif", "bmp", "pdf", "docx"],
        "png" => &["jpg", "webp", "tiff", "avif", "bmp", "pdf", "docx"],
        "webp" => &["jpg", "png", "tiff", "avif", "bmp"],
        "tiff" => &["jpg", "png", "webp", "avif", "bmp"],
        "bmp" => &["jpg", "png", "webp", "tiff", "avif"],
        // Decode-only sources: can produce any raster target, but (per the
        // module docs above) never appear as a target themselves.
        "heic" | "heif" => &["jpg", "png", "webp", "tiff", "avif", "bmp"],
        "svg" => &["jpg", "png", "webp", "tiff", "avif", "bmp"],
        // "avif" deliberately falls through to the empty case: decoding
        // isn't implemented, so it can't be offered as a source (see the
        // module docs above for why this differs from the HEIC treatment).
        _ => &[],
    }
}

/// Normalizes an input-side extension alias to this module's canonical
/// spelling: `jpeg` decodes identically to `jpg`, and `tif` identically to
/// `tiff`. Every other extension (including already-canonical ones) passes
/// through unchanged. Target extensions are always the canonical spelling
/// already (`supported_targets` never lists `jpeg` or `tif`), so this is
/// only needed on the source side.
fn normalize(ext: &str) -> &str {
    match ext {
        "jpeg" => "jpg",
        "tif" => "tiff",
        other => other,
    }
}

/// Converts `source` to `target_extension`, writing to `destination`.
pub fn convert(source: &Path, target_extension: &str, destination: &Path) -> Result<(), ConvertError> {
    // Defensive dead code: `convert::mod`'s dispatcher special-cases
    // `(Image, "pdf" | "docx")` and routes those to `super::document`
    // before ever reaching this function. Kept as a real error (never
    // `unreachable!()`) since this module can also be called directly, e.g.
    // from tests or a future caller that skips the dispatcher.
    if target_extension == "pdf" || target_extension == "docx" {
        return Err(ConvertError::UnsupportedConversion {
            from: "image".to_string(),
            to: target_extension.to_string(),
        });
    }

    let format = target_format(target_extension)?;

    let source_str = source
        .to_str()
        .ok_or_else(|| ConvertError::InvalidSource("path is not valid UTF-8".to_string()))?;
    let source_ext = normalize(&extension_of(source_str).unwrap_or_default()).to_string();

    if source_ext == "heic" || source_ext == "heif" {
        return Err(ConvertError::EngineUnavailable(
            "HEIC/HEIF decoding requires libheif, which isn't bundled in this build".to_string(),
        ));
    }
    if source_ext == "avif" {
        return Err(ConvertError::EngineUnavailable(
            "AVIF decoding requires an AV1 decoder (e.g. libdav1d), which isn't bundled in this build".to_string(),
        ));
    }

    let decoded = if source_ext == "svg" {
        rasterize_svg(source)?
    } else {
        ImageReader::open(source)
            .map_err(ConvertError::Io)?
            .with_guessed_format()
            .map_err(ConvertError::Io)?
            .decode()
            .map_err(|error| ConvertError::Decode(error.to_string()))?
    };

    // `DynamicImage::save_with_format` automatically converts to a color
    // type the target encoder supports (e.g. dropping alpha for JPEG) —
    // see its own doc comment ("Color Conversion") in the `image` crate.
    decoded
        .save_with_format(destination, format)
        .map_err(|error| ConvertError::Other(error.to_string()))
}

/// Maps a validated raster target extension to the `image` crate's format
/// enum. `target_extension` is expected to already be one of this module's
/// own `supported_targets` entries, but this still returns a real error
/// instead of panicking on anything else (e.g. a direct call from a test).
fn target_format(target_extension: &str) -> Result<ImageFormat, ConvertError> {
    match target_extension {
        "jpg" => Ok(ImageFormat::Jpeg),
        "png" => Ok(ImageFormat::Png),
        "webp" => Ok(ImageFormat::WebP),
        "tiff" => Ok(ImageFormat::Tiff),
        "avif" => Ok(ImageFormat::Avif),
        "bmp" => Ok(ImageFormat::Bmp),
        other => Err(ConvertError::UnsupportedConversion {
            from: "image".to_string(),
            to: other.to_string(),
        }),
    }
}

/// Rasterizes an SVG source file into an RGBA image, via `resvg`. Renders
/// at the SVG's intrinsic size (its `width`/`height`/`viewBox`); if none of
/// those pin down a concrete size, falls back to a 1024x1024 default
/// (`usvg`'s own `default_size` fallback mechanism, used for viewports that
/// only specify relative/percentage dimensions).
fn rasterize_svg(source: &Path) -> Result<DynamicImage, ConvertError> {
    let data = std::fs::read(source).map_err(ConvertError::Io)?;

    let options = usvg::Options {
        default_size: usvg::Size::from_wh(1024.0, 1024.0).expect("1024x1024 is a valid non-zero size"),
        ..Default::default()
    };

    let tree = usvg::Tree::from_data(&data, &options).map_err(|error| ConvertError::Decode(format!("invalid SVG: {error}")))?;

    let size = tree.size();
    let width = (size.width().round() as u32).max(1);
    let height = (size.height().round() as u32).max(1);

    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or_else(|| {
        ConvertError::Other(format!("could not allocate a {width}x{height} pixmap for SVG rasterization"))
    })?;

    resvg::render(&tree, tiny_skia::Transform::identity(), &mut pixmap.as_mut());

    let rgba = pixmap.take_demultiplied();
    image::RgbaImage::from_raw(width, height, rgba)
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| ConvertError::Other("rasterized SVG pixel buffer did not match its declared dimensions".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A fresh, empty temp directory for one test, cleaned up on drop.
    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let id = COUNTER.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir().join(format!("satsuma-image-convert-test-{}-{id}", std::process::id()));
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

    /// A tiny 4x4 image split into four solid-colored quadrants, so pixel
    /// content actually varies across the image and a round-trip test can
    /// meaningfully check more than just "it decoded".
    fn sample_image() -> RgbaImage {
        let mut img = RgbaImage::new(4, 4);
        for y in 0..4 {
            for x in 0..4 {
                let color = match (x < 2, y < 2) {
                    (true, true) => Rgba([255, 0, 0, 255]),
                    (false, true) => Rgba([0, 255, 0, 255]),
                    (true, false) => Rgba([0, 0, 255, 255]),
                    (false, false) => Rgba([255, 255, 255, 255]),
                };
                img.put_pixel(x, y, color);
            }
        }
        img
    }

    fn write_source(dir: &Path, name: &str, format: ImageFormat) -> std::path::PathBuf {
        let path = dir.join(name);
        DynamicImage::ImageRgba8(sample_image())
            .save_with_format(&path, format)
            .expect("failed to write test fixture source image");
        path
    }

    fn assert_quadrants(image: &DynamicImage, label: &str) {
        let rgba = image.to_rgba8();
        assert_eq!(rgba.get_pixel(0, 0), &Rgba([255, 0, 0, 255]), "{label} top-left");
        assert_eq!(rgba.get_pixel(3, 0), &Rgba([0, 255, 0, 255]), "{label} top-right");
        assert_eq!(rgba.get_pixel(0, 3), &Rgba([0, 0, 255, 255]), "{label} bottom-left");
        assert_eq!(rgba.get_pixel(3, 3), &Rgba([255, 255, 255, 255]), "{label} bottom-right");
    }

    #[test]
    fn raster_set_is_pairwise_interconvertible() {
        let raster = [
            ("jpg", ImageFormat::Jpeg),
            ("png", ImageFormat::Png),
            ("webp", ImageFormat::WebP),
            ("tiff", ImageFormat::Tiff),
            ("bmp", ImageFormat::Bmp),
        ];
        // Lossless formats should round-trip pixel content exactly; jpg is
        // lossy so it only gets a dimensions/decodability check.
        let lossless = ["png", "webp", "tiff", "bmp"];

        for (source_ext, source_format) in raster {
            let dir = TempDir::new();
            let source = write_source(dir.path(), &format!("source.{source_ext}"), source_format);

            for &target_ext in supported_targets(source_ext) {
                if target_ext == "pdf" || target_ext == "docx" {
                    continue; // Owned by the documents module, not this one.
                }
                if target_ext == "avif" {
                    continue; // Covered separately: can't re-decode via `image` (see module docs).
                }

                let destination = dir.path().join(format!("out-{source_ext}-to-{target_ext}.{target_ext}"));
                convert(&source, target_ext, &destination).unwrap_or_else(|e| panic!("{source_ext} -> {target_ext} failed: {e}"));

                let decoded = image::open(&destination).unwrap_or_else(|e| panic!("could not re-decode {source_ext} -> {target_ext} output: {e}"));
                assert_eq!(decoded.width(), 4, "{source_ext} -> {target_ext} width");
                assert_eq!(decoded.height(), 4, "{source_ext} -> {target_ext} height");

                // Pixel-exact round-trip only holds when *both* ends are
                // lossless — a lossless target still faithfully reproduces
                // whatever a lossy *source* already degraded.
                if lossless.contains(&source_ext) && lossless.contains(&target_ext) {
                    assert_quadrants(&decoded, &format!("{source_ext} -> {target_ext}"));
                }
            }
        }
    }

    #[test]
    fn avif_target_produces_a_valid_avif_file() {
        // AVIF encoding is real (pure-Rust `ravif`), but this build has no
        // AVIF decoder (see module docs), so we can't round-trip it through
        // `image::open`. Verify instead via the ISOBMFF `ftyp`/`avif` brand
        // signature every valid still-image AVIF file starts with.
        // (Manually cross-checked in development with `ffmpeg -i`, which
        // successfully decoded a file produced this way — not part of this
        // automated suite since it isn't a dependency of the build.)
        let dir = TempDir::new();
        let source = write_source(dir.path(), "source.png", ImageFormat::Png);
        let destination = dir.path().join("out.avif");

        convert(&source, "avif", &destination).expect("png -> avif should succeed");

        let bytes = std::fs::read(&destination).expect("avif output should exist");
        assert!(bytes.len() > 12, "avif output is suspiciously small");
        assert_eq!(&bytes[4..8], b"ftyp", "missing ISOBMFF ftyp box");
        assert_eq!(&bytes[8..12], b"avif", "missing avif major brand");
    }

    #[test]
    fn svg_rasterizes_to_every_raster_target() {
        let dir = TempDir::new();
        let source = dir.path().join("source.svg");
        std::fs::write(
            &source,
            br#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><rect width="8" height="8" fill="red"/></svg>"#,
        )
        .unwrap();

        for &target_ext in supported_targets("svg") {
            if target_ext == "avif" {
                continue; // Can't re-decode avif in this build; see above.
            }
            let destination = dir.path().join(format!("out.{target_ext}"));
            convert(&source, target_ext, &destination).unwrap_or_else(|e| panic!("svg -> {target_ext} failed: {e}"));

            let decoded = image::open(&destination).unwrap_or_else(|e| panic!("could not re-decode svg -> {target_ext} output: {e}"));
            assert_eq!(decoded.width(), 8);
            assert_eq!(decoded.height(), 8);

            // The rect fully covers the canvas with no anti-aliasing at the
            // edges, so every lossless target's pixels should be exactly
            // red; jpg is lossy so only check it's close.
            let rgba = decoded.to_rgba8();
            let center = rgba.get_pixel(4, 4);
            if target_ext == "jpg" {
                assert!(center[0] > 200 && center[1] < 60 && center[2] < 60, "{target_ext}: expected roughly red, got {center:?}");
            } else {
                assert_eq!(center, &Rgba([255, 0, 0, 255]), "{target_ext}: expected exact red");
            }
        }
    }

    #[test]
    fn svg_is_never_offered_as_a_conversion_target() {
        for source_ext in ["jpg", "png", "webp", "tiff", "bmp", "heic", "heif"] {
            assert!(
                !supported_targets(source_ext).contains(&"svg"),
                "{source_ext} should never offer svg as a target"
            );
        }
    }

    #[test]
    fn jpg_and_png_also_export_to_pdf_and_docx() {
        assert!(supported_targets("jpg").contains(&"pdf"));
        assert!(supported_targets("jpg").contains(&"docx"));
        assert!(supported_targets("png").contains(&"pdf"));
        assert!(supported_targets("png").contains(&"docx"));
    }

    #[test]
    fn only_jpg_and_png_export_to_pdf_and_docx() {
        for source_ext in ["webp", "tiff", "bmp", "heic", "heif", "svg"] {
            let targets = supported_targets(source_ext);
            assert!(!targets.contains(&"pdf"), "{source_ext} should not offer pdf");
            assert!(!targets.contains(&"docx"), "{source_ext} should not offer docx");
        }
    }

    #[test]
    fn convert_defensively_rejects_pdf_and_docx_targets() {
        let dir = TempDir::new();
        let source = write_source(dir.path(), "source.jpg", ImageFormat::Jpeg);

        for target in ["pdf", "docx"] {
            let destination = dir.path().join(format!("out.{target}"));
            let result = convert(&source, target, &destination);
            assert!(matches!(result, Err(ConvertError::UnsupportedConversion { .. })), "target {target} should be rejected");
        }
    }

    #[test]
    fn heic_and_heif_list_the_full_raster_set_but_never_convert() {
        let expected: &[&str] = &["jpg", "png", "webp", "tiff", "avif", "bmp"];
        assert_eq!(supported_targets("heic"), expected);
        assert_eq!(supported_targets("heif"), expected);

        let dir = TempDir::new();
        // A real HEIC/HEIF sample wasn't obtainable in this sandbox (no
        // heif-enc/avifenc/imagemagick binaries, and no libheif-dev to
        // build an encoder against) — but convert() never actually parses
        // the source bytes for a HEIC input, it rejects by extension
        // before opening the decoder, so this still exercises the real
        // code path.
        let source = dir.path().join("source.heic");
        std::fs::write(&source, b"not a real heic file, see module docs").unwrap();

        let destination = dir.path().join("out.jpg");
        let result = convert(&source, "jpg", &destination);
        assert!(matches!(result, Err(ConvertError::EngineUnavailable(_))));
    }

    #[test]
    fn avif_is_never_offered_as_a_source() {
        assert_eq!(supported_targets("avif"), &[] as &[&str]);
    }

    #[test]
    fn jpeg_alias_matches_jpg_targets() {
        assert_eq!(supported_targets("jpeg"), supported_targets("jpg"));
    }

    #[test]
    fn tif_alias_matches_tiff_targets() {
        assert_eq!(supported_targets("tif"), supported_targets("tiff"));
    }

    #[test]
    fn convert_rejects_a_target_this_module_does_not_support() {
        let dir = TempDir::new();
        let source = write_source(dir.path(), "source.jpg", ImageFormat::Jpeg);
        let destination = dir.path().join("out.zip");

        let result = convert(&source, "zip", &destination);
        assert!(matches!(result, Err(ConvertError::UnsupportedConversion { .. })));
    }

    #[test]
    fn convert_rejects_a_missing_source_file() {
        let dir = TempDir::new();
        let destination = dir.path().join("out.png");

        let result = convert(Path::new("/no/such/file.jpg"), "png", &destination);
        assert!(result.is_err());
    }

    #[test]
    fn convert_rejects_a_corrupt_source_file() {
        let dir = TempDir::new();
        let source = dir.path().join("corrupt.png");
        std::fs::write(&source, b"this is not a real png file").unwrap();
        let destination = dir.path().join("out.jpg");

        let result = convert(&source, "jpg", &destination);
        assert!(result.is_err());
    }
}
