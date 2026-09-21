//! Document/text/subtitle conversion: PDF -> DOCX/JPG/PNG/TXT, JPG/PNG ->
//! PDF/DOCX (the cross-family image export path — this module owns the
//! PDF/DOCX writer entirely, so it isn't built twice against
//! [`super::image`]), TXT -> PDF/JPG/PNG/SRT/VTT, and pairwise TXT/SRT/VTT
//! conversion.
//!
//! ## Crate choices (researched against the actual Rust PDF ecosystem)
//!
//! PDF **parsing/rasterization** and PDF **writing** are different problems
//! with different realistic solutions:
//!
//! * **Text extraction** (PDF -> TXT, and the text half of PDF -> DOCX) uses
//!   [`pdf_extract`] (MIT) — pure Rust, no native dependency, always
//!   available.
//! * **Rasterization** (PDF -> JPG/PNG, and the scanned-page image fallback
//!   of PDF -> DOCX) uses [`pdfium_render`] (MIT OR Apache-2.0), binding
//!   dynamically at runtime (via `libloading`, never linked at build time)
//!   to a pdfium shared library whose *directory* is passed in as this
//!   module's `pdfium_lib_dir: Option<&Path>` parameter — exactly the same
//!   externally-vendored-binary pattern the video/audio families use for
//!   their FFmpeg sidecar. Faithfully rasterizing arbitrary PDFs (vector
//!   paths, embedded fonts, transparency, ...) in pure Rust with zero native
//!   dependencies is not a solved problem today; a from-scratch pipeline on
//!   top of `lopdf` + `ttf-parser`/`ab_glyph` + `tiny-skia` was considered
//!   and rejected as out of scope for this phase. `mupdf-rs` was rejected
//!   outright: the underlying MuPDF library is (L)GPL/AGPL-licensed in the
//!   editions that matter here, which this MIT-licensed, distributed-binary
//!   project cannot take on. `None` gracefully degrades every
//!   rasterization-dependent conversion to [`ConvertError::EngineUnavailable`]
//!   — everything else (PDF -> TXT, TXT -> anything, SRT/VTT/TXT
//!   interconversion, JPG/PNG -> PDF/DOCX) keeps working with zero native
//!   dependencies.
//! * **Writing** PDFs (TXT -> PDF, JPG/PNG -> PDF) uses [`printpdf`] (MIT) —
//!   pure Rust, no native dependency in either direction. Its `text_layout`
//!   feature is enabled for real embedded-font text-showing operators
//!   (genuinely selectable/copyable text, not vector outlines or a
//!   rendered image); its `html`/image-codec features are left off since
//!   this module decodes JPG/PNG itself via its own `image` dependency and
//!   hands printpdf a [`printpdf::RawImage`] built from already-decoded
//!   pixels.
//! * **DOCX** is a hand-rolled minimal OOXML package (ZIP via the `zip`
//!   crate, MIT; hand-written XML strings with an escaping helper) per the
//!   project brief's explicit instruction not to reach for a generic
//!   high-level "write a docx" crate that silently drops image attachments.
//!   See the private `docx` submodule below for the ZIP/XML structure and this module's tests
//!   for how it's verified.
//! * **TXT -> JPG/PNG** rasterizes text itself (no PDF involved) using the
//!   [`font8x8`] crate (MIT) — a small, public-domain 8x8 bitmap font
//!   re-published under that crate's own MIT license. This is a deliberate
//!   choice over embedding a real TrueType font file (e.g. DejaVu/Liberation):
//!   those carry their own separate font-specific licenses (Bitstream Vera
//!   License / SIL OFL) that aren't on this workspace's `deny.toml` allow
//!   list, whereas font8x8's bitmap data ships as ordinary MIT-licensed
//!   crate content. The tradeoff is fidelity (blocky monospace glyphs,
//!   ASCII-only — non-ASCII characters render as blank cells) for zero
//!   license ambiguity; TXT -> JPG/PNG is a utility export, not a
//!   typeset document.

use super::error::ConvertError;
use super::naming::output_dir_path;
use std::fs;
use std::path::{Path, PathBuf};

/// Extensions this module can produce from `source_ext` (a PDF/TXT/SRT/VTT
/// file, or a JPG/PNG file being exported to PDF/DOCX), excluding
/// `source_ext` itself.
pub fn supported_targets(source_ext: &str) -> &'static [&'static str] {
    match source_ext.to_lowercase().as_str() {
        "pdf" => &["docx", "jpg", "png", "txt"],
        "jpg" | "jpeg" | "png" => &["pdf", "docx"],
        "txt" => &["pdf", "jpg", "png", "srt", "vtt"],
        "srt" => &["txt", "vtt"],
        "vtt" => &["txt", "srt"],
        _ => &[],
    }
}

/// Converts `source` to `target_extension`, writing to `destination` for a
/// single-file output (TXT/SRT/VTT/DOCX targets, or a single-page PDF ->
/// JPG/PNG), and returns the path actually written. A multi-page PDF ->
/// JPG/PNG instead writes into a sibling folder computed via
/// [`output_dir_path`] and ignores `destination` for the individual page
/// files (see Tools.md's "Report JPG Pages/Page 001.jpg" behavior) — in
/// that case the returned path is that sibling folder, never the phantom
/// `destination`, so callers (the App toast, a future open-output-folder)
/// always name a real, existing path.
///
/// `pdfium_lib_dir` is the directory containing a vendored pdfium shared
/// library (e.g. from <https://github.com/bblanchon/pdfium-binaries>),
/// consulted only for the PDF-rasterization-dependent conversions (PDF ->
/// JPG/PNG, and the scanned-page-image fallback of PDF -> DOCX). Pass
/// `None` when it's unavailable — those specific conversions then fail with
/// [`ConvertError::EngineUnavailable`] instead of panicking; every other
/// conversion in this module works with `None`.
///
/// This signature has one extra parameter versus the family-module
/// convention documented in `convert::mod` (`source, target_extension,
/// destination`) — see this module's own top-level doc comment for why, and
/// the caller of this function (`convert::mod`'s dispatcher) needs to be
/// updated to pass it through, e.g. from its own top-level `convert()` the
/// same way it already threads `ffmpeg_bin: Option<&Path>` to the video/audio
/// families.
pub fn convert(
    source: &Path,
    target_extension: &str,
    destination: &Path,
    pdfium_lib_dir: Option<&Path>,
) -> Result<PathBuf, ConvertError> {
    let source_ext = ext_of(source);
    let target = target_extension.to_lowercase();

    match source_ext.as_str() {
        "pdf" => convert_from_pdf(source, &target, destination, pdfium_lib_dir),
        "jpg" | "jpeg" | "png" => convert_from_image(source, &target, destination),
        "txt" => convert_from_txt(source, &target, destination),
        "srt" => convert_from_subtitle(source, &target, destination, SubtitleKind::Srt),
        "vtt" => convert_from_subtitle(source, &target, destination, SubtitleKind::Vtt),
        _ => Err(ConvertError::UnsupportedConversion { from: source_ext, to: target }),
    }
}

/// The lowercased extension of `path`, or `""` if it has none.
fn ext_of(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase()
}

/// The user-facing message for a PDF with no extractable text layer — a
/// deliberate, spec-mandated stop rather than a shortcut: this module never
/// attempts OCR.
fn scanned_pdf_error() -> ConvertError {
    ConvertError::Other(
        "this PDF appears to be scanned — OCR would be needed to extract text, which Satsuma doesn't currently attempt"
            .to_string(),
    )
}

/// Extracts all selectable text from a PDF, wrapping any real parse failure
/// as [`ConvertError::Decode`]. An `Ok` empty/whitespace-only result means
/// the PDF has no text layer at all (scanned/image-only) — callers decide
/// what to do with that (see [`scanned_pdf_error`]).
fn extract_pdf_text(source: &Path) -> Result<String, ConvertError> {
    pdf_extract::extract_text(source).map_err(|error| ConvertError::Decode(format!("could not read PDF: {error}")))
}

fn convert_from_pdf(
    source: &Path,
    target: &str,
    destination: &Path,
    pdfium_lib_dir: Option<&Path>,
) -> Result<PathBuf, ConvertError> {
    match target {
        "txt" => {
            let text = extract_pdf_text(source)?;
            if text.trim().is_empty() {
                return Err(scanned_pdf_error());
            }
            fs::write(destination, text)?;
            Ok(destination.to_path_buf())
        }
        "docx" => {
            let text = extract_pdf_text(source)?;
            if !text.trim().is_empty() {
                let blocks: Vec<docx::Block> = text
                    .split("\n\n")
                    .map(|paragraph| docx::Block::Paragraph(paragraph.trim_end().to_string()))
                    .collect();
                docx::write(destination, &blocks).map(|()| destination.to_path_buf())
            } else {
                let Some(lib_dir) = pdfium_lib_dir else {
                    return Err(ConvertError::EngineUnavailable(
                        "this PDF has no extractable text, so embedding a rendered page image \
                         requires the pdfium rendering library, which was not provided"
                            .to_string(),
                    ));
                };
                let pages = raster::rasterize_pdf(source, lib_dir)?;
                let mut blocks = Vec::with_capacity(pages.len());
                for page in pages {
                    let width_px = page.width();
                    let height_px = page.height();
                    let bytes = encode_rgb_image(&page, image::ImageFormat::Png)?;
                    blocks.push(docx::Block::Image {
                        bytes,
                        ext: "png",
                        width_px,
                        height_px,
                        dpi: raster::DPI,
                    });
                }
                docx::write(destination, &blocks).map(|()| destination.to_path_buf())
            }
        }
        "jpg" | "png" => {
            let Some(lib_dir) = pdfium_lib_dir else {
                return Err(ConvertError::EngineUnavailable(
                    "PDF rasterization requires the pdfium rendering library, which was not provided".to_string(),
                ));
            };
            let pages = raster::rasterize_pdf(source, lib_dir)?;
            write_rendered_pages(source, &pages, target, destination)
        }
        _ => Err(ConvertError::UnsupportedConversion { from: "pdf".to_string(), to: target.to_string() }),
    }
}

fn convert_from_image(source: &Path, target: &str, destination: &Path) -> Result<PathBuf, ConvertError> {
    match target {
        "pdf" => {
            let decoded =
                image::open(source).map_err(|error| ConvertError::Decode(format!("could not read image: {error}")))?;
            let bytes = pdf_write::image_pdf(&decoded)?;
            fs::write(destination, bytes)?;
            Ok(destination.to_path_buf())
        }
        "docx" => {
            let dimensions = image::image_dimensions(source)
                .map_err(|error| ConvertError::Decode(format!("could not read image: {error}")))?;
            let source_ext = ext_of(source);
            let docx_ext: &'static str = if source_ext == "png" { "png" } else { "jpeg" };
            // Embed the original bytes untouched (no decode/re-encode round
            // trip), so a JPG source keeps its original compression.
            let bytes = fs::read(source)?;
            let block = docx::Block::Image {
                bytes,
                ext: docx_ext,
                width_px: dimensions.0,
                height_px: dimensions.1,
                // Arbitrary user images carry no reliable embedded DPI; 96 is
                // the conventional OOXML/web default for "display at native
                // pixel size".
                dpi: 96.0,
            };
            docx::write(destination, &[block]).map(|()| destination.to_path_buf())
        }
        _ => Err(ConvertError::UnsupportedConversion { from: ext_of(source), to: target.to_string() }),
    }
}

fn convert_from_txt(source: &Path, target: &str, destination: &Path) -> Result<PathBuf, ConvertError> {
    let text = read_utf8(source)?;

    match target {
        "pdf" => {
            let bytes = pdf_write::text_pdf(&text)?;
            fs::write(destination, bytes)?;
            Ok(destination.to_path_buf())
        }
        "jpg" | "png" => {
            let format = if target == "png" { image::ImageFormat::Png } else { image::ImageFormat::Jpeg };
            let rendered = text_image::render(&text);
            let bytes = encode_rgb_image(&rendered, format)?;
            fs::write(destination, bytes)?;
            Ok(destination.to_path_buf())
        }
        "srt" => {
            let cues = subtitles::cues_from_txt(&text);
            fs::write(destination, subtitles::write_srt(&cues))?;
            Ok(destination.to_path_buf())
        }
        "vtt" => {
            let cues = subtitles::cues_from_txt(&text);
            fs::write(destination, subtitles::write_vtt(&cues))?;
            Ok(destination.to_path_buf())
        }
        _ => Err(ConvertError::UnsupportedConversion { from: "txt".to_string(), to: target.to_string() }),
    }
}

/// Which subtitle format a `convert_from_subtitle` source file is in.
enum SubtitleKind {
    /// `.srt` (SubRip): comma-decimal timestamps, numeric cue indices.
    Srt,
    /// `.vtt` (WebVTT): dot-decimal timestamps, mandatory `WEBVTT` header.
    Vtt,
}

fn convert_from_subtitle(
    source: &Path,
    target: &str,
    destination: &Path,
    kind: SubtitleKind,
) -> Result<PathBuf, ConvertError> {
    let text = read_utf8(source)?;
    let cues = match kind {
        SubtitleKind::Srt => subtitles::parse_srt(&text),
        SubtitleKind::Vtt => subtitles::parse_vtt(&text),
    };
    let from_ext = match kind {
        SubtitleKind::Srt => "srt",
        SubtitleKind::Vtt => "vtt",
    };

    match target {
        "txt" => {
            fs::write(destination, subtitles::write_txt(&cues))?;
            Ok(destination.to_path_buf())
        }
        "srt" => {
            fs::write(destination, subtitles::write_srt(&cues))?;
            Ok(destination.to_path_buf())
        }
        "vtt" => {
            fs::write(destination, subtitles::write_vtt(&cues))?;
            Ok(destination.to_path_buf())
        }
        _ => Err(ConvertError::UnsupportedConversion { from: from_ext.to_string(), to: target.to_string() }),
    }
}

/// Reads `source` as UTF-8 text, turning a decode failure into
/// [`ConvertError::Decode`] rather than the generic I/O error.
fn read_utf8(source: &Path) -> Result<String, ConvertError> {
    fs::read_to_string(source).map_err(|error| {
        if error.kind() == std::io::ErrorKind::InvalidData {
            ConvertError::Decode(format!("source file is not valid UTF-8 text: {error}"))
        } else {
            ConvertError::Io(error)
        }
    })
}

/// Writes rasterized PDF pages either to `destination` directly (a single
/// page) or into a collision-safe sibling folder (multiple pages), per
/// Tools.md's "Report JPG Pages/Page 001.jpg" behavior. Returns the path
/// actually written: `destination` for the single-page case, the sibling
/// folder for the multi-page case (never the phantom `destination`, which
/// is ignored there) — so callers always name a real, existing path.
fn write_rendered_pages(
    source: &Path,
    pages: &[image::RgbImage],
    target: &str,
    destination: &Path,
) -> Result<PathBuf, ConvertError> {
    let format = if target == "png" { image::ImageFormat::Png } else { image::ImageFormat::Jpeg };

    if pages.len() <= 1 {
        let page = pages
            .first()
            .ok_or_else(|| ConvertError::Other("this PDF has no pages to render".to_string()))?;
        let bytes = encode_rgb_image(page, format)?;
        fs::write(destination, bytes)?;
        Ok(destination.to_path_buf())
    } else {
        let label = if target == "png" { "PNG Pages" } else { "JPG Pages" };
        let dir = output_dir_path(source, label);
        fs::create_dir_all(&dir)?;
        for (index, page) in pages.iter().enumerate() {
            let bytes = encode_rgb_image(page, format)?;
            let file_name = format!("Page {:03}.{target}", index + 1);
            fs::write(dir.join(file_name), bytes)?;
        }
        Ok(dir)
    }
}

/// Encodes an in-memory RGB image to bytes in the given format.
fn encode_rgb_image(image: &image::RgbImage, format: image::ImageFormat) -> Result<Vec<u8>, ConvertError> {
    let dynamic = image::DynamicImage::ImageRgb8(image.clone());
    let mut buffer = std::io::Cursor::new(Vec::new());
    dynamic
        .write_to(&mut buffer, format)
        .map_err(|error| ConvertError::Other(format!("could not encode image: {error}")))?;
    Ok(buffer.into_inner())
}

/// PDF *rasterization*: binds to an externally-vendored pdfium shared
/// library at a caller-supplied path and renders pages to RGB bitmaps.
/// Never linked at build time (see this module's top-level doc comment) —
/// `cargo build`/`cargo test` need no native library present.
mod raster {
    use super::ConvertError;
    use pdfium_render::prelude::*;
    use std::path::Path;

    /// Tools.md mandates every PDF page/image render at 300 DPI.
    pub(super) const DPI: f32 = 300.0;

    /// Renders every page of the PDF at `source` to an RGB bitmap at 300 DPI,
    /// using the pdfium shared library found in `lib_dir`.
    pub(super) fn rasterize_pdf(source: &Path, lib_dir: &Path) -> Result<Vec<image::RgbImage>, ConvertError> {
        let bindings = Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(lib_dir)).map_err(
            |error| {
                ConvertError::EngineUnavailable(format!(
                    "could not load the pdfium library from {}: {error}",
                    lib_dir.display()
                ))
            },
        )?;
        let pdfium = Pdfium::new(bindings);
        let document = pdfium
            .load_pdf_from_file(source, None)
            .map_err(|error| ConvertError::Decode(format!("could not open PDF: {error}")))?;

        let mut pages = Vec::new();
        for page in document.pages().iter() {
            pages.push(render_page(&page)?);
        }
        Ok(pages)
    }

    fn render_page(page: &PdfPage) -> Result<image::RgbImage, ConvertError> {
        let width_px = ((page.width().value / 72.0) * DPI).round().max(1.0) as i32;
        let height_px = ((page.height().value / 72.0) * DPI).round().max(1.0) as i32;

        let config = PdfRenderConfig::new().set_target_size(width_px, height_px);
        let bitmap = page
            .render_with_config(&config)
            .map_err(|error| ConvertError::Other(format!("could not rasterize PDF page: {error}")))?;
        let dynamic_image = bitmap
            .as_image()
            .map_err(|error| ConvertError::Other(format!("could not convert the rendered page to an image: {error}")))?;
        Ok(dynamic_image.into_rgb8())
    }
}

/// PDF *writing* (the opposite, native-dependency-free direction from
/// [`raster`]): TXT -> PDF with real embedded-font selectable text, and
/// JPG/PNG -> PDF by embedding already-decoded pixel data.
mod pdf_write {
    use super::ConvertError;
    use printpdf::*;

    const PAGE_WIDTH_MM: f32 = 210.0; // A4
    const PAGE_HEIGHT_MM: f32 = 297.0;
    const MARGIN_MM: f32 = 20.0;
    const FONT_SIZE_PT: f32 = 11.0;
    const LINE_HEIGHT_PT: f32 = 14.0;
    /// Courier's advance width is exactly 600/1000 em for every glyph
    /// (including space) — used both for character-count line wrapping and
    /// for the `TextItem::Offset` space-width workaround below.
    const COURIER_SPACE_WIDTH_PER_MILLE: f32 = 600.0;
    /// Matches [`super::raster::DPI`] so a JPG/PNG page comes out at the same
    /// physical size a PDF -> JPG/PNG round trip would have rendered it at.
    const IMAGE_DPI: f32 = 300.0;

    fn mm_to_pt(mm: f32) -> f32 {
        mm * 72.0 / 25.4
    }

    /// Builds a paginated PDF of `text` with real, selectable/copyable text:
    /// an embedded (subsetted) Courier font and genuine text-showing content
    /// stream operators, not vector outlines or a rendered image.
    pub(super) fn text_pdf(text: &str) -> Result<Vec<u8>, ConvertError> {
        let mut doc = PdfDocument::new("Satsuma export");
        let parsed_font = BuiltinFont::Courier
            .get_parsed_font()
            .ok_or_else(|| ConvertError::Other("could not load the embedded Courier font".to_string()))?;
        let font_id = doc.add_font(&parsed_font);

        let usable_width_pt = mm_to_pt(PAGE_WIDTH_MM - 2.0 * MARGIN_MM);
        let usable_height_pt = mm_to_pt(PAGE_HEIGHT_MM - 2.0 * MARGIN_MM);
        // Courier's advance width is exactly 600/1000 em for every glyph, so
        // this character-count wrap is exact, not an approximation.
        let char_width_pt = FONT_SIZE_PT * (COURIER_SPACE_WIDTH_PER_MILLE / 1000.0);
        let chars_per_line = ((usable_width_pt / char_width_pt).floor() as usize).max(1);
        let lines_per_page = ((usable_height_pt / LINE_HEIGHT_PT).floor() as usize).max(1);

        let wrapped = wrap_text_lines(text, chars_per_line);
        let page_line_groups: Vec<Vec<String>> = if wrapped.is_empty() {
            vec![Vec::new()]
        } else {
            wrapped.chunks(lines_per_page).map(<[String]>::to_vec).collect()
        };

        let mut pages = Vec::with_capacity(page_line_groups.len());
        for group in &page_line_groups {
            let mut ops = vec![
                Op::StartTextSection,
                Op::SetTextCursor { pos: Point { x: Mm(MARGIN_MM).into(), y: Mm(PAGE_HEIGHT_MM - MARGIN_MM).into() } },
                Op::SetLineHeight { lh: Pt(LINE_HEIGHT_PT) },
                Op::SetFont { font: PdfFontHandle::External(font_id.clone()), size: Pt(FONT_SIZE_PT) },
            ];
            for (index, line) in group.iter().enumerate() {
                if index > 0 {
                    ops.push(Op::AddLineBreak);
                }
                // Built as explicit `TextItem::Text`/`TextItem::Offset` runs
                // rather than one `TextItem::Text(line)` with literal spaces:
                // printpdf 0.12.8's embedded-font glyph mapping for the space
                // character collides with glyph 0 (.notdef) for every
                // built-in font tested, which corrupts every space into "!"
                // on re-extraction (confirmed both in this module's own
                // tests and in isolation against upstream printpdf/pdf-extract
                // directly). Advancing the cursor by one glyph-width via
                // `Offset` instead of showing an actual space glyph sidesteps
                // the bug entirely; `pdf_extract` still recovers the space
                // from the resulting horizontal gap between words.
                let mut items = Vec::new();
                for (word_index, word) in line.split(' ').enumerate() {
                    if word_index > 0 {
                        items.push(TextItem::Offset(-COURIER_SPACE_WIDTH_PER_MILLE));
                    }
                    if !word.is_empty() {
                        items.push(TextItem::Text(word.to_string()));
                    }
                }
                if !items.is_empty() {
                    ops.push(Op::ShowText { items });
                }
            }
            ops.push(Op::EndTextSection);
            pages.push(PdfPage::new(Mm(PAGE_WIDTH_MM), Mm(PAGE_HEIGHT_MM), ops));
        }

        let save_options = PdfSaveOptions { subset_fonts: true, ..Default::default() };
        let mut warnings = Vec::new();
        Ok(doc.with_pages(pages).save(&save_options, &mut warnings))
    }

    /// Builds a single-page PDF containing `image`, sized to the image's
    /// pixel dimensions at [`IMAGE_DPI`].
    pub(super) fn image_pdf(image: &image::DynamicImage) -> Result<Vec<u8>, ConvertError> {
        let rgb = image.to_rgb8();
        let (width_px, height_px) = (rgb.width(), rgb.height());
        let raw_image = RawImage {
            pixels: RawImageData::U8(rgb.into_raw()),
            width: width_px as usize,
            height: height_px as usize,
            data_format: RawImageFormat::RGB8,
            tag: Vec::new(),
        };

        let mut doc = PdfDocument::new("Satsuma export");
        let image_id = doc.add_image(&raw_image);

        let page_width_mm = (width_px as f32 / IMAGE_DPI * 25.4).max(1.0);
        let page_height_mm = (height_px as f32 / IMAGE_DPI * 25.4).max(1.0);

        let ops = vec![Op::UseXobject {
            id: image_id,
            transform: XObjectTransform { dpi: Some(IMAGE_DPI), ..Default::default() },
        }];
        let page = PdfPage::new(Mm(page_width_mm), Mm(page_height_mm), ops);

        let mut warnings = Vec::new();
        Ok(doc.with_pages(vec![page]).save(&PdfSaveOptions::default(), &mut warnings))
    }

    /// Word-wraps `text` (preserving existing line breaks and blank lines)
    /// to at most `max_chars` characters per line, hard-breaking any single
    /// word that alone exceeds `max_chars`.
    fn wrap_text_lines(text: &str, max_chars: usize) -> Vec<String> {
        let mut lines = Vec::new();
        for raw_line in text.split('\n') {
            if raw_line.is_empty() {
                lines.push(String::new());
                continue;
            }
            let mut current = String::new();
            for word in raw_line.split_whitespace() {
                let mut word = word.to_string();
                while word.chars().count() > max_chars {
                    let head: String = word.chars().take(max_chars).collect();
                    let tail: String = word.chars().skip(max_chars).collect();
                    if !current.is_empty() {
                        lines.push(std::mem::take(&mut current));
                    }
                    lines.push(head);
                    word = tail;
                }
                if current.is_empty() {
                    current = word;
                } else if current.chars().count() + 1 + word.chars().count() <= max_chars {
                    current.push(' ');
                    current.push_str(&word);
                } else {
                    lines.push(std::mem::take(&mut current));
                    current = word;
                }
            }
            lines.push(current);
        }
        lines
    }
}

/// Hand-rolled minimal OOXML `.docx` package writer: a ZIP container ([`zip`]
/// crate) with hand-written XML parts, per the project brief's explicit
/// instruction not to reach for a generic high-level "write a docx" crate
/// that silently drops image attachments.
mod docx {
    use super::ConvertError;
    use std::fs;
    use std::io::Write;
    use std::path::Path;
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    /// One piece of DOCX body content.
    pub(super) enum Block {
        /// A text paragraph (internal `\n`s become explicit line breaks).
        Paragraph(String),
        /// An embedded image, as an inline drawing.
        Image {
            /// The raw, already-encoded image file bytes (PNG or JPEG).
            bytes: Vec<u8>,
            /// The file extension to give it under `word/media/` — `"png"`
            /// or `"jpeg"`, which also determines its `[Content_Types].xml`
            /// entry.
            ext: &'static str,
            /// Pixel width, for sizing the drawing.
            width_px: u32,
            /// Pixel height, for sizing the drawing.
            height_px: u32,
            /// The DPI to size the drawing at (96 for an arbitrary source
            /// image, 300 for an internally-rasterized PDF page).
            dpi: f32,
        },
    }

    /// Writes `blocks` as a minimal, valid `.docx` package to `destination`:
    /// `[Content_Types].xml`, `_rels/.rels`, `word/document.xml`,
    /// `word/_rels/document.xml.rels`, and `word/media/imageN.<ext>` for
    /// each [`Block::Image`].
    pub(super) fn write(destination: &Path, blocks: &[Block]) -> Result<(), ConvertError> {
        let mut document_xml = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <w:document \
             xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" \
             xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" \
             xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
             xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\" \
             xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">\
             <w:body>\n",
        );

        let mut relationships = String::new();
        let mut media: Vec<(String, &'static str, &[u8])> = Vec::new();
        let mut counter = 0u32;

        for block in blocks {
            match block {
                Block::Paragraph(text) => {
                    document_xml.push_str("<w:p>");
                    let lines: Vec<&str> = text.split('\n').collect();
                    for (index, line) in lines.iter().enumerate() {
                        if index > 0 {
                            document_xml.push_str("<w:r><w:br/></w:r>");
                        }
                        document_xml.push_str("<w:r><w:t xml:space=\"preserve\">");
                        document_xml.push_str(&xml_escape(line));
                        document_xml.push_str("</w:t></w:r>");
                    }
                    document_xml.push_str("</w:p>\n");
                }
                Block::Image { bytes, ext, width_px, height_px, dpi } => {
                    counter += 1;
                    let rel_id = format!("rId{counter}");
                    let file_name = format!("image{counter}.{ext}");
                    let width_emu = (*width_px as f64 * 914_400.0 / *dpi as f64).round() as i64;
                    let height_emu = (*height_px as f64 * 914_400.0 / *dpi as f64).round() as i64;

                    document_xml.push_str(&format!(
                        "<w:p><w:r><w:drawing>\
                         <wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\">\
                         <wp:extent cx=\"{width_emu}\" cy=\"{height_emu}\"/>\
                         <wp:docPr id=\"{counter}\" name=\"Picture {counter}\"/>\
                         <a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\">\
                         <pic:pic>\
                         <pic:nvPicPr><pic:cNvPr id=\"{counter}\" name=\"image{counter}.{ext}\"/><pic:cNvPicPr/></pic:nvPicPr>\
                         <pic:blipFill><a:blip r:embed=\"{rel_id}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>\
                         <pic:spPr>\
                         <a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{width_emu}\" cy=\"{height_emu}\"/></a:xfrm>\
                         <a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom>\
                         </pic:spPr>\
                         </pic:pic></a:graphicData></a:graphic>\
                         </wp:inline></w:drawing></w:r></w:p>\n"
                    ));
                    relationships.push_str(&format!(
                        "<Relationship Id=\"{rel_id}\" \
                         Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" \
                         Target=\"media/{file_name}\"/>"
                    ));
                    media.push((file_name, ext, bytes.as_slice()));
                }
            }
        }

        document_xml.push_str("</w:body></w:document>");

        let mut image_extensions: Vec<&str> = media.iter().map(|(_, ext, _)| *ext).collect();
        image_extensions.sort_unstable();
        image_extensions.dedup();
        let content_type_defaults: String = image_extensions
            .iter()
            .map(|ext| {
                let content_type = match *ext {
                    "png" => "image/png",
                    "jpeg" | "jpg" => "image/jpeg",
                    other => other,
                };
                format!("<Default Extension=\"{ext}\" ContentType=\"{content_type}\"/>")
            })
            .collect();

        let content_types = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
             <Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
             <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
             <Default Extension=\"xml\" ContentType=\"application/xml\"/>\
             {content_type_defaults}\
             <Override PartName=\"/word/document.xml\" \
             ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/>\
             </Types>"
        );

        let root_rels = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
             <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
             <Relationship Id=\"rId1\" \
             Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" \
             Target=\"word/document.xml\"/>\
             </Relationships>";

        let document_rels = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
             <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
             {relationships}\
             </Relationships>"
        );

        let file = fs::File::create(destination)?;
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        zip.start_file("[Content_Types].xml", options).map_err(zip_err)?;
        zip.write_all(content_types.as_bytes())?;

        zip.start_file("_rels/.rels", options).map_err(zip_err)?;
        zip.write_all(root_rels.as_bytes())?;

        zip.start_file("word/document.xml", options).map_err(zip_err)?;
        zip.write_all(document_xml.as_bytes())?;

        zip.start_file("word/_rels/document.xml.rels", options).map_err(zip_err)?;
        zip.write_all(document_rels.as_bytes())?;

        for (file_name, _ext, bytes) in &media {
            zip.start_file(format!("word/media/{file_name}"), options).map_err(zip_err)?;
            zip.write_all(bytes)?;
        }

        zip.finish().map_err(zip_err)?;
        Ok(())
    }

    fn xml_escape(input: &str) -> String {
        input
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }

    fn zip_err(error: zip::result::ZipError) -> ConvertError {
        ConvertError::Other(format!("could not write DOCX package: {error}"))
    }
}

/// TXT -> JPG/PNG: rasterizes plain text directly (no PDF involved) into one
/// tall monospace-bitmap image of the whole document.
mod text_image {
    /// Public-domain glyphs are 8x8; scaled up for legibility.
    const SCALE: u32 = 3;
    const CELL_W: u32 = 8 * SCALE;
    const CELL_H: u32 = 8 * SCALE;
    /// Wrap width, in characters — keeps the image from becoming absurdly
    /// wide for one long line, while still producing "one tall image of the
    /// full document" per Tools.md (no pagination).
    const MAX_COLUMNS: usize = 100;
    const MARGIN_PX: u32 = 20;

    /// Renders `text` to a single RGB image, white background/black text.
    pub(super) fn render(text: &str) -> image::RgbImage {
        let lines = wrap_lines(text, MAX_COLUMNS);
        let line_count = lines.len().max(1) as u32;
        let max_line_len = lines.iter().map(|line| line.chars().count()).max().unwrap_or(0).max(1) as u32;

        let width = MARGIN_PX * 2 + max_line_len * CELL_W;
        let height = MARGIN_PX * 2 + line_count * CELL_H;

        let mut image = image::RgbImage::from_pixel(width, height, image::Rgb([255, 255, 255]));
        for (row, line) in lines.iter().enumerate() {
            for (col, ch) in line.chars().enumerate() {
                draw_glyph(&mut image, ch, MARGIN_PX + col as u32 * CELL_W, MARGIN_PX + row as u32 * CELL_H);
            }
        }
        image
    }

    /// Splits `text` on existing line breaks, then hard-wraps any line
    /// longer than `max_columns` characters.
    fn wrap_lines(text: &str, max_columns: usize) -> Vec<String> {
        let mut out = Vec::new();
        for raw_line in text.split('\n') {
            let chars: Vec<char> = raw_line.chars().collect();
            if chars.len() <= max_columns {
                out.push(raw_line.to_string());
            } else {
                for chunk in chars.chunks(max_columns) {
                    out.push(chunk.iter().collect());
                }
            }
        }
        if out.is_empty() {
            out.push(String::new());
        }
        out
    }

    /// Draws one ASCII glyph's 8x8 bitmap, scaled up, with its top-left
    /// corner at `(x0, y0)`. Non-ASCII characters render as a blank cell.
    fn draw_glyph(image: &mut image::RgbImage, ch: char, x0: u32, y0: u32) {
        let code = ch as usize;
        let glyph = if code < 128 { font8x8::legacy::BASIC_LEGACY[code] } else { font8x8::legacy::NOTHING_TO_DISPLAY };

        for (row, byte) in glyph.iter().enumerate() {
            for col in 0..8u32 {
                if byte & (1 << col) == 0 {
                    continue;
                }
                for sy in 0..SCALE {
                    for sx in 0..SCALE {
                        let px = x0 + col * SCALE + sx;
                        let py = y0 + row as u32 * SCALE + sy;
                        image.put_pixel(px, py, image::Rgb([0, 0, 0]));
                    }
                }
            }
        }
    }
}

/// Shared cue model and parsers/writers for TXT/SRT/VTT interconversion.
///
/// TXT carries no timing information, so TXT -> SRT/VTT invents one: each
/// non-blank line of the source text becomes one cue, spaced 3 seconds
/// apart with a 2.5-second duration. This is a synthetic placeholder timing
/// scheme, not a transcription alignment — documented here since it's the
/// one lossy/generative direction in an otherwise format-preserving group.
mod subtitles {
    /// One subtitle cue: a time range and its text (which may itself be
    /// multiple lines, for SRT/VTT).
    pub(super) struct Cue {
        pub(super) start_ms: u64,
        pub(super) end_ms: u64,
        pub(super) text: String,
    }

    /// Parses SubRip (`.srt`) content: blocks separated by a blank line,
    /// each an optional numeric index line, a comma-decimal timing line,
    /// then one or more text lines.
    pub(super) fn parse_srt(input: &str) -> Vec<Cue> {
        let normalized = input.replace("\r\n", "\n");
        let mut cues = Vec::new();

        for block in normalized.split("\n\n") {
            let block = block.trim();
            if block.is_empty() {
                continue;
            }
            let mut lines = block.lines();
            let Some(first) = lines.next() else { continue };
            let timing_line = if first.contains("-->") { Some(first) } else { lines.next() };
            let Some(timing_line) = timing_line else { continue };
            let Some((start_ms, end_ms)) = parse_timing_line(timing_line, ',') else { continue };
            let text: Vec<&str> = lines.collect();
            cues.push(Cue { start_ms, end_ms, text: text.join("\n") });
        }
        cues
    }

    /// Parses WebVTT (`.vtt`) content: the `WEBVTT` header, any cue
    /// identifiers/`NOTE` blocks, and dot-decimal timing lines (optionally
    /// followed by cue settings) are all tolerated by simply skipping
    /// anything before the first `-->` line in a block.
    pub(super) fn parse_vtt(input: &str) -> Vec<Cue> {
        let normalized = input.replace("\r\n", "\n");
        let mut cues = Vec::new();

        for block in normalized.split("\n\n") {
            let block = block.trim();
            if block.is_empty() {
                continue;
            }
            let mut timing_line = None;
            let mut text_lines = Vec::new();
            for line in block.lines() {
                if timing_line.is_none() && line.contains("-->") {
                    timing_line = Some(line);
                } else if timing_line.is_some() {
                    text_lines.push(line);
                }
            }
            let Some(timing_line) = timing_line else { continue };
            let Some((start_ms, end_ms)) = parse_timing_line(timing_line, '.') else { continue };
            cues.push(Cue { start_ms, end_ms, text: text_lines.join("\n") });
        }
        cues
    }

    /// Builds cues from plain text: one cue per non-blank line, with
    /// synthetic sequential timing (see this module's doc comment).
    pub(super) fn cues_from_txt(input: &str) -> Vec<Cue> {
        input
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .enumerate()
            .map(|(index, line)| {
                let start_ms = index as u64 * 3_000;
                Cue { start_ms, end_ms: start_ms + 2_500, text: line.to_string() }
            })
            .collect()
    }

    /// Renders cues as SubRip: 1-based numeric index, comma-decimal timing.
    pub(super) fn write_srt(cues: &[Cue]) -> String {
        let mut out = String::new();
        for (index, cue) in cues.iter().enumerate() {
            out.push_str(&format!(
                "{}\n{} --> {}\n{}\n\n",
                index + 1,
                format_timestamp(cue.start_ms, ','),
                format_timestamp(cue.end_ms, ','),
                cue.text
            ));
        }
        out
    }

    /// Renders cues as WebVTT: mandatory header, dot-decimal timing.
    pub(super) fn write_vtt(cues: &[Cue]) -> String {
        let mut out = String::from("WEBVTT\n\n");
        for cue in cues {
            out.push_str(&format!(
                "{} --> {}\n{}\n\n",
                format_timestamp(cue.start_ms, '.'),
                format_timestamp(cue.end_ms, '.'),
                cue.text
            ));
        }
        out
    }

    /// Renders cues as plain text: each cue's text on its own paragraph,
    /// timing dropped (TXT is the common, timing-free interchange format).
    pub(super) fn write_txt(cues: &[Cue]) -> String {
        cues.iter().map(|cue| cue.text.as_str()).collect::<Vec<_>>().join("\n\n")
    }

    fn parse_timing_line(line: &str, decimal_separator: char) -> Option<(u64, u64)> {
        let (start_str, rest) = line.split_once("-->")?;
        let start_ms = parse_timestamp(start_str.trim(), decimal_separator)?;
        // A VTT timing line may have cue settings after the end timestamp,
        // separated by whitespace (e.g. "... --> ... line:0 position:50%").
        let end_str = rest.split_whitespace().next()?;
        let end_ms = parse_timestamp(end_str, decimal_separator)?;
        Some((start_ms, end_ms))
    }

    fn parse_timestamp(value: &str, decimal_separator: char) -> Option<u64> {
        let (time_part, ms_part) = value.rsplit_once(decimal_separator)?;
        let ms: u64 = ms_part.trim().parse().ok()?;
        let parts: Vec<&str> = time_part.split(':').collect();
        let (hours, minutes, seconds): (u64, u64, u64) = match parts.as_slice() {
            [h, m, s] => (h.parse().ok()?, m.parse().ok()?, s.parse().ok()?),
            [m, s] => (0, m.parse().ok()?, s.parse().ok()?),
            _ => return None,
        };
        Some((hours * 3600 + minutes * 60 + seconds) * 1000 + ms)
    }

    fn format_timestamp(total_ms: u64, decimal_separator: char) -> String {
        let ms = total_ms % 1000;
        let total_seconds = total_ms / 1000;
        let seconds = total_seconds % 60;
        let minutes = (total_seconds / 60) % 60;
        let hours = total_seconds / 3600;
        format!("{hours:02}:{minutes:02}:{seconds:02}{decimal_separator}{ms:03}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A fresh, empty temp directory for one test, cleaned up on drop.
    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let id = COUNTER.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir().join(format!("satsuma-document-test-{}-{id}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    // ---- supported_targets ----

    #[test]
    fn supported_targets_matches_the_format_matrix() {
        assert_eq!(supported_targets("pdf"), &["docx", "jpg", "png", "txt"]);
        assert_eq!(supported_targets("PDF"), &["docx", "jpg", "png", "txt"]);
        assert_eq!(supported_targets("jpg"), &["pdf", "docx"]);
        assert_eq!(supported_targets("jpeg"), &["pdf", "docx"]);
        assert_eq!(supported_targets("png"), &["pdf", "docx"]);
        assert_eq!(supported_targets("txt"), &["pdf", "jpg", "png", "srt", "vtt"]);
        assert_eq!(supported_targets("srt"), &["txt", "vtt"]);
        assert_eq!(supported_targets("vtt"), &["txt", "srt"]);
        assert_eq!(supported_targets("docx"), &[] as &[&str]);
        assert_eq!(supported_targets("mp4"), &[] as &[&str]);
    }

    // ---- unsupported target rejection ----

    #[test]
    fn convert_rejects_an_unsupported_target() {
        let dir = TempDir::new();
        let source = dir.path().join("notes.txt");
        fs::write(&source, "hello").unwrap();
        let destination = dir.path().join("notes.zip");

        let result = convert(&source, "zip", &destination, None);
        assert!(matches!(result, Err(ConvertError::UnsupportedConversion { .. })));
    }

    // ---- TXT -> PDF -> TXT (selectable text round trip) ----

    #[test]
    fn txt_to_pdf_produces_selectable_text_recoverable_via_extraction() {
        let dir = TempDir::new();
        let source = dir.path().join("letter.txt");
        fs::write(&source, "Hello, Satsuma!\nThis is a test document.").unwrap();
        let destination = dir.path().join("letter.pdf");

        convert(&source, "pdf", &destination, None).unwrap();
        assert!(destination.exists());

        let extracted = extract_pdf_text(&destination).unwrap();
        assert!(extracted.contains("Hello, Satsuma!"), "extracted text was: {extracted:?}");
        assert!(extracted.contains("This is a test document."), "extracted text was: {extracted:?}");
    }

    #[test]
    fn txt_to_pdf_paginates_long_text_into_multiple_pages() {
        let dir = TempDir::new();
        let source = dir.path().join("long.txt");
        let long_text = "line of text\n".repeat(200);
        fs::write(&source, &long_text).unwrap();
        let destination = dir.path().join("long.pdf");

        convert(&source, "pdf", &destination, None).unwrap();

        let bytes = fs::read(&destination).unwrap();
        let pages = pdf_extract::extract_text_from_mem_by_pages(&bytes).unwrap();
        assert!(pages.len() > 1, "expected more than one page for 200 wrapped lines, got {}", pages.len());
        for page in &pages {
            assert!(!page.trim().is_empty(), "every page should carry some of the wrapped text");
        }
        let extracted: String = pages.join("");
        assert!(extracted.contains("line of text"));
    }

    // ---- TXT -> JPG/PNG (one tall image) ----

    #[test]
    fn txt_to_png_produces_one_image_sized_for_the_text() {
        let dir = TempDir::new();
        let source = dir.path().join("short.txt");
        fs::write(&source, "hi\nthere").unwrap();
        let destination = dir.path().join("short.png");

        convert(&source, "png", &destination, None).unwrap();

        let image = image::open(&destination).unwrap();
        // 2 lines, longest is "there" (5 chars): width/height should reflect
        // that grid at the module's fixed cell size (24px at SCALE=3).
        assert_eq!(image.width(), 20 * 2 + 5 * 24);
        assert_eq!(image.height(), 20 * 2 + 2 * 24);
    }

    #[test]
    fn txt_to_jpg_grows_taller_for_more_lines() {
        let dir = TempDir::new();
        let source = dir.path().join("many_lines.txt");
        fs::write(&source, "a\nb\nc\nd\ne").unwrap();
        let destination = dir.path().join("many_lines.jpg");

        convert(&source, "jpg", &destination, None).unwrap();

        let image = image::open(&destination).unwrap();
        assert_eq!(image.height(), 20 * 2 + 5 * 24);
    }

    // ---- JPG/PNG -> PDF / DOCX ----

    fn write_test_png(path: &Path, width: u32, height: u32) {
        let image = image::RgbImage::from_pixel(width, height, image::Rgb([200, 50, 50]));
        image.save(path).unwrap();
    }

    #[test]
    fn png_to_pdf_embeds_the_image() {
        let dir = TempDir::new();
        let source = dir.path().join("photo.png");
        write_test_png(&source, 40, 30);
        let destination = dir.path().join("photo.pdf");

        convert(&source, "pdf", &destination, None).unwrap();

        let bytes = fs::read(&destination).unwrap();
        assert_eq!(&bytes[0..4], b"%PDF", "output is not a valid PDF header");
        // The image XObject's raw pixel stream should be present somewhere
        // in the (uncompressed-by-default in debug builds) PDF bytes.
        assert!(bytes.len() > 1000, "PDF is suspiciously small for an embedded 40x30 image: {} bytes", bytes.len());
    }

    #[test]
    fn png_to_docx_produces_a_valid_zip_with_the_image_in_media() {
        let dir = TempDir::new();
        let source = dir.path().join("photo.png");
        write_test_png(&source, 40, 30);
        let destination = dir.path().join("photo.docx");

        convert(&source, "docx", &destination, None).unwrap();

        let (content_types, document_xml, document_rels, media_names) = read_docx_parts(&destination);
        assert!(content_types.contains("image/png"));
        assert!(document_xml.contains("<w:drawing>"));
        assert!(document_rels.contains("relationships/image"));
        assert_eq!(media_names, vec!["word/media/image1.png".to_string()]);
    }

    #[test]
    fn jpg_to_docx_embeds_original_bytes_without_recompression() {
        let dir = TempDir::new();
        let source = dir.path().join("photo.jpg");
        let image = image::RgbImage::from_pixel(16, 16, image::Rgb([10, 20, 30]));
        image::DynamicImage::ImageRgb8(image).save_with_format(&source, image::ImageFormat::Jpeg).unwrap();
        let original_bytes = fs::read(&source).unwrap();
        let destination = dir.path().join("photo.docx");

        convert(&source, "docx", &destination, None).unwrap();

        let file = fs::File::open(&destination).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut embedded = Vec::new();
        std::io::Read::read_to_end(&mut archive.by_name("word/media/image1.jpeg").unwrap(), &mut embedded).unwrap();
        assert_eq!(embedded, original_bytes);
    }

    // ---- PDF -> TXT / DOCX (text extraction) ----

    /// Builds a small real PDF with genuinely selectable text via this
    /// module's own TXT -> PDF writer, so tests exercise real PDF bytes
    /// rather than a mocked/hand-crafted fixture.
    fn build_text_pdf_fixture(text: &str) -> Vec<u8> {
        pdf_write::text_pdf(text).unwrap()
    }

    #[test]
    fn pdf_to_txt_extracts_selectable_text() {
        let dir = TempDir::new();
        let source = dir.path().join("report.pdf");
        fs::write(&source, build_text_pdf_fixture("Quarterly Report\nRevenue is up.")).unwrap();
        let destination = dir.path().join("report.txt");

        convert(&source, "txt", &destination, None).unwrap();

        let text = fs::read_to_string(&destination).unwrap();
        assert!(text.contains("Quarterly Report"));
        assert!(text.contains("Revenue is up."));
    }

    #[test]
    fn pdf_to_docx_embeds_extracted_text() {
        let dir = TempDir::new();
        let source = dir.path().join("report.pdf");
        fs::write(&source, build_text_pdf_fixture("Meeting Notes\nAction items follow.")).unwrap();
        let destination = dir.path().join("report.docx");

        convert(&source, "docx", &destination, None).unwrap();

        let (_content_types, document_xml, _rels, _media) = read_docx_parts(&destination);
        assert!(document_xml.contains("Meeting Notes"));
        assert!(document_xml.contains("Action items follow."));
    }

    #[test]
    fn pdf_to_jpg_without_pdfium_is_engine_unavailable() {
        let dir = TempDir::new();
        let source = dir.path().join("report.pdf");
        fs::write(&source, build_text_pdf_fixture("Some text")).unwrap();
        let destination = dir.path().join("report.jpg");

        let result = convert(&source, "jpg", &destination, None);
        assert!(matches!(result, Err(ConvertError::EngineUnavailable(_))), "got: {result:?}");
    }

    #[test]
    fn scanned_pdf_to_docx_without_pdfium_is_engine_unavailable() {
        // A PDF with a page but no text-showing operators at all: exercises
        // the "no extractable text -> needs pdfium for the image fallback"
        // branch directly, since pdf_extract will return an empty string.
        let dir = TempDir::new();
        let source = dir.path().join("scanned.pdf");
        fs::write(&source, minimal_textless_pdf()).unwrap();
        let destination = dir.path().join("scanned.docx");

        let result = convert(&source, "docx", &destination, None);
        assert!(matches!(result, Err(ConvertError::EngineUnavailable(_))), "got: {result:?}");
    }

    #[test]
    fn scanned_pdf_to_txt_reports_the_ocr_needed_error() {
        let dir = TempDir::new();
        let source = dir.path().join("scanned.pdf");
        fs::write(&source, minimal_textless_pdf()).unwrap();
        let destination = dir.path().join("scanned.txt");

        let result = convert(&source, "txt", &destination, None);
        match result {
            Err(ConvertError::Other(message)) => {
                assert!(message.contains("scanned"), "message was: {message}");
                assert!(message.contains("OCR"), "message was: {message}");
                assert!(message.contains("Satsuma"), "message should name the project correctly: {message}");
            }
            other => panic!("expected ConvertError::Other, got {other:?}"),
        }
    }

    /// A minimal, valid single-page PDF with no text-showing operators
    /// anywhere in its content stream (an empty content stream stands in
    /// for a scanned/image-only page for the purposes of testing the
    /// no-text branch, without needing a real embedded scanned image).
    /// Built with this module's own `printpdf`-based writer (rather than
    /// hand-crafted bytes) so the fixture is guaranteed structurally valid.
    fn minimal_textless_pdf() -> Vec<u8> {
        use printpdf::{Mm, PdfDocument, PdfPage, PdfSaveOptions};
        let mut doc = PdfDocument::new("Satsuma test fixture");
        let page = PdfPage::new(Mm(200.0), Mm(200.0), Vec::new());
        let mut warnings = Vec::new();
        doc.with_pages(vec![page]).save(&PdfSaveOptions::default(), &mut warnings)
    }

    #[test]
    fn extract_pdf_text_is_empty_for_a_textless_pdf() {
        let dir = TempDir::new();
        let source = dir.path().join("blank.pdf");
        fs::write(&source, minimal_textless_pdf()).unwrap();

        let text = extract_pdf_text(&source).unwrap();
        assert!(text.trim().is_empty(), "expected no extractable text, got: {text:?}");
    }

    // ---- SRT/VTT/TXT pairwise interconversion ----

    const SAMPLE_SRT: &str = "1\n00:00:01,000 --> 00:00:02,500\nHello world\n\n2\n00:00:03,000 --> 00:00:04,250\nSecond cue\n";
    const SAMPLE_VTT: &str = "WEBVTT\n\n00:00:01.000 --> 00:00:02.500\nHello world\n\n00:00:03.000 --> 00:00:04.250\nSecond cue\n";

    #[test]
    fn srt_to_vtt_converts_timing_syntax_and_adds_header() {
        let dir = TempDir::new();
        let source = dir.path().join("subs.srt");
        fs::write(&source, SAMPLE_SRT).unwrap();
        let destination = dir.path().join("subs.vtt");

        convert(&source, "vtt", &destination, None).unwrap();

        let output = fs::read_to_string(&destination).unwrap();
        assert!(output.starts_with("WEBVTT\n\n"));
        assert!(output.contains("00:00:01.000 --> 00:00:02.500"));
        assert!(output.contains("Hello world"));
        assert!(!output.contains(','), "VTT output must use dot-decimal timing, not comma: {output}");
    }

    #[test]
    fn vtt_to_srt_converts_timing_syntax_and_adds_index() {
        let dir = TempDir::new();
        let source = dir.path().join("subs.vtt");
        fs::write(&source, SAMPLE_VTT).unwrap();
        let destination = dir.path().join("subs.srt");

        convert(&source, "srt", &destination, None).unwrap();

        let output = fs::read_to_string(&destination).unwrap();
        assert!(output.contains("00:00:01,000 --> 00:00:02,500"));
        assert!(output.starts_with('1'));
        assert!(output.contains("Hello world"));
    }

    #[test]
    fn srt_to_txt_drops_timing_and_keeps_text() {
        let dir = TempDir::new();
        let source = dir.path().join("subs.srt");
        fs::write(&source, SAMPLE_SRT).unwrap();
        let destination = dir.path().join("subs.txt");

        convert(&source, "txt", &destination, None).unwrap();

        let output = fs::read_to_string(&destination).unwrap();
        assert!(output.contains("Hello world"));
        assert!(output.contains("Second cue"));
        assert!(!output.contains("-->"));
    }

    #[test]
    fn vtt_to_txt_drops_timing_and_keeps_text() {
        let dir = TempDir::new();
        let source = dir.path().join("subs.vtt");
        fs::write(&source, SAMPLE_VTT).unwrap();
        let destination = dir.path().join("subs.txt");

        convert(&source, "txt", &destination, None).unwrap();

        let output = fs::read_to_string(&destination).unwrap();
        assert!(output.contains("Hello world"));
        assert!(output.contains("Second cue"));
    }

    #[test]
    fn txt_to_srt_invents_sequential_timing_per_line() {
        let dir = TempDir::new();
        let source = dir.path().join("lines.txt");
        fs::write(&source, "First line\nSecond line\n").unwrap();
        let destination = dir.path().join("lines.srt");

        convert(&source, "srt", &destination, None).unwrap();

        let output = fs::read_to_string(&destination).unwrap();
        assert!(output.contains("00:00:00,000 --> 00:00:02,500"));
        assert!(output.contains("First line"));
        assert!(output.contains("00:00:03,000 --> 00:00:05,500"));
        assert!(output.contains("Second line"));
    }

    #[test]
    fn txt_to_vtt_invents_sequential_timing_per_line() {
        let dir = TempDir::new();
        let source = dir.path().join("lines.txt");
        fs::write(&source, "Only line\n").unwrap();
        let destination = dir.path().join("lines.vtt");

        convert(&source, "vtt", &destination, None).unwrap();

        let output = fs::read_to_string(&destination).unwrap();
        assert!(output.starts_with("WEBVTT"));
        assert!(output.contains("00:00:00.000 --> 00:00:02.500"));
        assert!(output.contains("Only line"));
    }

    #[test]
    fn txt_to_srt_to_txt_round_trips_line_text() {
        let dir = TempDir::new();
        let source = dir.path().join("roundtrip.txt");
        fs::write(&source, "Alpha\nBeta\nGamma\n").unwrap();
        let srt_path = dir.path().join("roundtrip.srt");
        let back_to_txt = dir.path().join("roundtrip_back.txt");

        convert(&source, "srt", &srt_path, None).unwrap();
        convert(&srt_path, "txt", &back_to_txt, None).unwrap();

        let recovered = fs::read_to_string(&back_to_txt).unwrap();
        assert!(recovered.contains("Alpha"));
        assert!(recovered.contains("Beta"));
        assert!(recovered.contains("Gamma"));
    }

    #[test]
    fn srt_to_vtt_to_srt_round_trips_cues_exactly() {
        let dir = TempDir::new();
        let source = dir.path().join("subs.srt");
        fs::write(&source, SAMPLE_SRT).unwrap();
        let vtt_path = dir.path().join("subs.vtt");
        let back_to_srt = dir.path().join("subs_back.srt");

        convert(&source, "vtt", &vtt_path, None).unwrap();
        convert(&vtt_path, "srt", &back_to_srt, None).unwrap();

        let original_cues = subtitles::parse_srt(&fs::read_to_string(&source).unwrap());
        let round_tripped_cues = subtitles::parse_srt(&fs::read_to_string(&back_to_srt).unwrap());

        assert_eq!(original_cues.len(), round_tripped_cues.len());
        for (original, round_tripped) in original_cues.iter().zip(round_tripped_cues.iter()) {
            assert_eq!(original.start_ms, round_tripped.start_ms);
            assert_eq!(original.end_ms, round_tripped.end_ms);
            assert_eq!(original.text, round_tripped.text);
        }
    }

    // ---- write_rendered_pages return value (the real written path) ----

    /// Builds a solid-color `image::RgbImage` without needing pdfium, so
    /// `write_rendered_pages`'s single- vs multi-page routing can be
    /// exercised directly (the pdfium-backed `convert` path that calls it
    /// is covered elsewhere / gated on the rendered library).
    fn solid_rgb_image(width: u32, height: u32, fill: image::Rgb<u8>) -> image::RgbImage {
        let mut image = image::RgbImage::new(width, height);
        for pixel in image.pixels_mut() {
            *pixel = fill;
        }
        image
    }

    #[test]
    fn write_rendered_pages_returns_destination_for_a_single_page() {
        let dir = TempDir::new();
        let source = dir.path().join("report.pdf");
        fs::write(&source, b"dummy").unwrap();
        let destination = dir.path().join("report.jpg");

        let written =
            write_rendered_pages(&source, &[solid_rgb_image(4, 4, image::Rgb([10, 20, 30]))], "jpg", &destination).unwrap();

        assert_eq!(written, destination);
        assert!(destination.is_file());
    }

    #[test]
    fn write_rendered_pages_returns_the_sibling_folder_for_multiple_pages() {
        let dir = TempDir::new();
        let source = dir.path().join("report.pdf");
        fs::write(&source, b"dummy").unwrap();
        let phantom_destination = dir.path().join("report.jpg");

        let pages = vec![solid_rgb_image(4, 4, image::Rgb([10, 20, 30])), solid_rgb_image(4, 4, image::Rgb([40, 50, 60]))];
        let written = write_rendered_pages(&source, &pages, "jpg", &phantom_destination).unwrap();

        // Regression: the returned path must be the sibling folder actually
        // written to, never the phantom `destination` that multi-page
        // exports ignore.
        assert_eq!(written, dir.path().join("report JPG Pages"));
        assert!(written.is_dir());
        assert!(written.join("Page 001.jpg").is_file());
        assert!(written.join("Page 002.jpg").is_file());
        assert!(!phantom_destination.exists());
    }

    // ---- helpers for inspecting a written .docx as a ZIP/XML package ----

    /// Reads back `[Content_Types].xml`, `word/document.xml`,
    /// `word/_rels/document.xml.rels`, and the list of `word/media/*` entry
    /// names from a `.docx` this module wrote — used to structurally verify
    /// the package (valid ZIP, well-formed XML parts) without needing Word.
    fn read_docx_parts(path: &Path) -> (String, String, String, Vec<String>) {
        let file = fs::File::open(path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();

        let read_entry = |archive: &mut zip::ZipArchive<fs::File>, name: &str| -> String {
            let mut entry = archive.by_name(name).unwrap();
            let mut contents = String::new();
            std::io::Read::read_to_string(&mut entry, &mut contents).unwrap();
            contents
        };

        let content_types = read_entry(&mut archive, "[Content_Types].xml");
        let document_xml = read_entry(&mut archive, "word/document.xml");
        let document_rels = read_entry(&mut archive, "word/_rels/document.xml.rels");

        // Every XML part must at least be well-formed - parse each with
        // quick-xml's reader and fail loudly if it errors out partway.
        for xml in [&content_types, &document_xml, &document_rels] {
            assert_well_formed_xml(xml);
        }

        let media_names: Vec<String> =
            (0..archive.len()).map(|i| archive.by_index(i).unwrap().name().to_string()).filter(|name| name.starts_with("word/media/")).collect();

        (content_types, document_xml, document_rels, media_names)
    }

    fn assert_well_formed_xml(xml: &str) {
        let mut reader = quick_xml::Reader::from_str(xml);
        loop {
            match reader.read_event() {
                Ok(quick_xml::events::Event::Eof) => break,
                Ok(_) => continue,
                Err(error) => panic!("malformed XML: {error} in:\n{xml}"),
            }
        }
    }

    // ---- pdfium end-to-end smoke (release pipeline, not plain cargo test) ----

    /// Runs the *real* vendored pdfium library end-to-end: this module's
    /// own pure-Rust writer produces a PDF (no native dependency), it is
    /// rasterized to JPG through pdfium, and the output is decoded back as
    /// a real image. `#[ignore]`d because it requires the vendored library,
    /// which plain `cargo test`/`cargo build` must never need — run it via
    /// `scripts/smoke-pdfium.sh`/`.ps1`, which `deploy.yml` calls right
    /// after `fetch-pdfium`, so a pinned pdfium release that has drifted
    /// incompatible with this app's pdfium-render bindings fails the
    /// release build instead of shipping a silent PDF -> JPG/DOCX break.
    #[test]
    #[ignore]
    fn pdfium_smoke_renders_a_pdf_page_to_jpg() {
        let lib_dir = std::env::var_os("SATSUMA_PDFIUM_SMOKE_LIB_DIR").expect(
            "this ignored smoke test must run via scripts/smoke-pdfium.sh (which sets SATSUMA_PDFIUM_SMOKE_LIB_DIR)",
        );
        let lib_dir = Path::new(&lib_dir);
        let dir = TempDir::new();

        let txt = dir.path().join("smoke.txt");
        fs::write(&txt, "Satsuma pdfium smoke test\nsecond line").unwrap();
        let pdf = dir.path().join("smoke.pdf");
        convert(&txt, "pdf", &pdf, None).unwrap();

        let jpg = dir.path().join("smoke.jpg");
        let written = convert(&pdf, "jpg", &jpg, Some(lib_dir)).unwrap();
        assert_eq!(written, jpg, "a single-page PDF should rasterize exactly to the destination");
        assert!(jpg.is_file(), "the rasterized JPG should exist");

        // Decoding it back proves the bytes are a real, complete JPEG —
        // not an error page or a stubbed library's empty output.
        let decoded = image::open(&jpg).expect("the rasterized output should decode as a real image");
        assert!(decoded.width() > 0 && decoded.height() > 0);
    }
}
