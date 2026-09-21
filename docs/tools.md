# Format & tool matrix

Satsuma's target end-state format/tool matrix. This describes **Satsuma's own planned scope**, informed by Tangerine's publicly documented feature set (its marketing site and shipped app documentation) at the time this was written — it is not a guaranteed match for Tangerine's actual implementation, which is closed-source and independent of this project (see the [README](../README.md)'s trademark notice). Satsuma's actual implementation reaches this matrix over several [phases](phases/functionality-phases.md) — the current repo's scope is intentionally a subset (images/video/audio only; PDF, archives, subtitles, and plain text are staged for later phases). This note covers *what* converts to *what*, and what each tool does — see [interaction.md](interaction.md) for how the wedge menu surfaces these, and [design.md](design.md) for how the wedges and tool chips look.

## Any file
**Tools:**
* **Compress** — Balanced (~20% size reduction target) or Strong (~50%) preset, plus an optional resize (cap longest edge, e.g. 2560/1920/1280px; preserves aspect ratio, never upscales). Savings are targets, not guarantees.
* **Edit Metadata** — inspect, change, or remove embedded info (location, camera, author, other fields), grouped by file/track/chapter where relevant. Format-preserving: rewrites the container's metadata boxes in place rather than a full decode/re-encode, to avoid quality loss and keep orientation/color-profile/animation fields intact unless the user opts to strip them too.

## Images
**Supported formats:**
* JPG
* PNG
* WebP
* HEIC
* TIFF
* SVG *(input-only — never a conversion target)*
* AVIF
* BMP
* PDF & DOCX export (JPG/PNG also export to PDF or DOCX)

**Tools:**
* Compress *(see Any file)*
* Edit Metadata *(see Any file)*
* **Edit Photo** — exposure, color, detail (dehaze, clarity, grain, noise reduction), and other adjustments with a live preview before export.
* **Add Background** — place the photo on a solid color, gradient, or another image; choose aspect ratio, spacing, corner radius, blur, shadow.
* **Crop Image** — drag a crop region, choose a preset or custom aspect ratio, or enter exact pixel dimensions; saves a copy at full resolution.
* **Redact Photo** — cover sensitive regions with a solid block, blur, or pixelation; supports layering several movable redaction areas. Not claimed to be a secure/irreversible redaction method (blur/pixelation are visual masking, not guaranteed-unrecoverable). Output is a flattened copy with metadata and embedded thumbnails stripped. Multi-page or animated source images are rejected outright rather than redacting one frame/page and silently dropping the rest.

**Multi-file tools:**
* **Create PDF** — combine several selected images (mixed formats, and SVGs, are fine) into one PDF, one page per image.
* **Create Collage** — grid, row, column, or featured layout; adjust order, spacing, corner radius, background.

## Video
**Supported formats:**
* MP4
* MOV
* MKV
* WebM
* AVI
* WMV
* GIF
* MP3 (audio export)

**Tools:**
* Compress — Balanced (~20%, keeps original dimensions, preserves HEVC/HDR) or Strong (~50%, caps height at 1080px). Reuses an existing AAC audio track as-is; only transcodes audio when it isn't already AAC.
* Edit Metadata *(see Any file)*
* **Remove Audio**
* **Trim Video** — frame-by-frame stepping or exact start/end timecodes; produces one sibling clip.
* **Crop Video** — exact output dimensions, preserves the original audio.
* **Change Video Speed** — slower/faster copy; preserves audio pitch, also works on silent video.
* **Video Snapshots** — pick exact frames and export them as full-resolution images.
* **Split Video** — divide at custom points or into equal parts; all output clips land in one folder.
* **Redact Video** — solid/blur/pixelation coverage, with each redacted region carrying its own time range. No subject tracking, no audio bleeping — visual masking only, and not claimed to be secure/irreversible.

**Multi-file tools:**
* **Join Videos** — arrange multiple clips (mixed MP4/MOV/MKV is fine) and combine into a single MP4; the first clip in the order sets the output canvas.

## Audio
**Supported formats:**
* MP3
* M4A
* WAV
* FLAC
* OGG
* Opus
* AIFF
* WMA

**Tools:**
* Compress *(see Any file)*
* Edit Metadata *(see Any file)*
* **Normalize Volume** — control loudness range and true peak, compare input/output levels.
* **Audio Visualizer** — render the audio as a shareable MP4 with an animated waveform or a still image; landscape/portrait/square.
* **Trim Audio** — waveform-level control, exact start/end times, or remove silence only from the start and end.
* **Convert Audio Channels** — mono/stereo conversion with independent left/right volume control and per-channel preview.
* **Bleep Audio** — replace a time range with a bleep tone; move the whole range or fine-tune its edges, with original-vs-bleeped comparison playback.

## Documents (PDF)
> Note: all pages/images rendered at 300 DPI

**Supported formats:**
* PDF → DOCX, JPG, PNG, TXT
* JPG/PNG → PDF, DOCX
* TXT → PDF, JPG, PNG, SRT, VTT

**Behavioral notes:**
* PDF↔image uses native PDF/image frameworks where the platform provides them; DOCX export writes a minimal OOXML package directly (many built-in DOCX writers drop image attachments).
* PDF→DOCX extracts selectable text where present; falls back to an embedded rendered page image otherwise (no layout reconstruction attempted).
* PDF→JPG/PNG renders every page at 300 DPI; a multi-page PDF produces a sibling folder (`Report JPG Pages/Page 001.jpg`, ...), each page keeping its own physical size, rotation, and DPI metadata.
* PDF→TXT requires selectable text; a scanned/image-only PDF reports that OCR is needed rather than attempting OCR itself.
* TXT→PDF produces paginated, selectable text; TXT→JPG/PNG produces one tall image of the full document.
* Office formats (PPTX/XLSX) are explicitly out of scope, both directions.

**Tools:**
* Compress *(see Any file)*
* Edit Metadata *(see Any file)*
* **Organize PDF** — visually reorder, rotate, duplicate, or remove individual pages.

**Multi-file tools:**
* **Merge PDFs** — combine several PDFs into one document, starting from file-manager selection order, reorderable before merging.

## Archives
**Supported formats:**
* ZIP
* TAR
* GZIP
* RAR

**Behavioral notes:**
* Every pairwise conversion between the four formats is supported.
* RAR input: password-protected and multivolume archives are not supported.
* RAR output: written as valid RAR5 using RAR's *store* method (no compression), to stay format-valid without needing RAR's proprietary compression algorithm.
* GZIP is single-stream: archive→GZIP actually produces `name.tar.gz`; a bare `.gz` source is unwrapped to its single payload before repackaging.
* 7Z is explicitly out of scope, both directions.

**Tools:**
* **Extract Archive**

## GIF
Treated as its own category for tools (though it's a video-family conversion target/source — see Video):
**Tools:**
* Edit Metadata *(see Any file)*
* **Read QR Codes**

## Text
> Note: UTF-8 text

**Supported formats:**
* TXT → PDF, JPG, PNG, SRT, VTT

**Tools:** —

## Subtitles
**Supported formats:**
* SRT
* VTT
* TXT

**Tools:** —

---

## Directories and unsupported inputs

Directories and non-file URLs are rejected outright — no menu, no fallback conversion attempt.

## Engineering conventions worth following

A few UX/engineering conventions this matrix implies, worth keeping consistent as tools get built out (see [phases/phase-2.md](phases/phase-2.md) and [phases/phase-3.md](phases/phase-3.md)):

- Progress reporting distinguishes a real percentage (when the engine reports encode timestamps against a known duration) from an indeterminate spinner (when there's no measurable total) — see [interaction.md](interaction.md).
- A conversion's result state stays open until the output is confirmed written via a fresh directory read — not just until the worker process exits — and reports a clear failure (full source filename and error detail) rather than silently disappearing.
- Batch operations combine current-file progress with count-of-files-done for an overall percentage.
- FFmpeg is bundled directly in the app so end users need no separate install — see the [README](../README.md)'s tech stack section.
