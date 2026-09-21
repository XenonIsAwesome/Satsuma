# Phase 2 — core conversion engine

Real conversion, no more stub files — the *entire* format matrix, no
dedicated tool GUIs. **This is the launch phase: Satsuma goes live once
this ships.**

## Scope

Replace [Phase 0](../phase0.md)'s hardcoded `wedgeOptions` *and* its stub
"same name, swapped extension" file-write with an actual conversion engine
and backend-driven format lists — the full format matrix from
[tools.md](../tools.md), not just a "core" subset. The dividing line for
this phase isn't format family, it's **complexity of implementation**:
everything here is a pairwise format conversion reachable straight from the
wedge menu (pick a target format, get a converted sibling file) with no
dedicated tool GUI/settings page of its own. Every [tools.md](../tools.md)
entry that opens its own configuration screen (Compress, Edit Metadata,
Crop, Edit Photo, Trim, and the rest) is explicitly deferred to
[Phase 3](phase-3.md)–[Phase 5](phase-5.md), not part of this phase's
scope.

Because none of these format families share meaningfully more code with
each other than they do standing alone (images use the `image` crate,
video/audio use the FFmpeg sidecar, PDF/DOCX/TXT use a document library,
archives use an archive library), **work proceeds on all of them in
parallel** rather than family-by-family — there's no dependency forcing
images to land before video, or documents before archives.

- **Images:** JPG, PNG, WebP, HEIC (read at minimum), TIFF, SVG
  (input-only), AVIF, BMP — via the Rust `image` crate. Includes JPG/PNG ->
  PDF/DOCX export.
- **Video:** MP4, MOV, MKV, WebM, AVI, WMV, GIF, plus MP3 audio export —
  via a bundled FFmpeg sidecar binary (no system FFmpeg dependency
  required).
- **Audio:** MP3, M4A, WAV, FLAC, OGG, Opus, AIFF, WMA — also via FFmpeg.
- **Documents:** PDF -> DOCX/JPG/PNG/TXT, JPG/PNG -> PDF/DOCX, TXT ->
  PDF/JPG/PNG/SRT/VTT (see [tools.md](../tools.md) for the 300 DPI / OOXML
  / "OCR needed" behavioral notes).
- **Archives:** every pairwise ZIP/TAR/GZIP/RAR conversion, plus Extract
  Archive (no dedicated settings screen — a single action, so it belongs
  here rather than [Phase 4](phase-4.md)).
- **Subtitles/Text:** SRT/VTT/TXT conversions, tied to the TXT conversion
  work since TXT is the common link format.

Conversion/detection logic lives in `crates/satsuma-core` (already the
pattern for `file_type.rs`), kept independent of the Tauri command layer so
it's unit-testable without spinning up the app. The wedge menu component
itself shouldn't need to change — it's already styled per
[design.md](../design.md) from Phase 0 — only `data/wedgeOptions.ts`
becomes real, backend-queried data instead of a static table. The file I/O
contract established in [Phase 0](../phase0.md) (sibling location,
collision-safe naming, and the desktop icon-positioning bonus where the OS
supports it) stays exactly as-is — only what gets written into the output
file changes, from a byte-for-byte stub to a real encode/transcode.

## Explicitly not in this phase

- Any tool that opens its own dedicated settings/GUI page — Compress, Edit
  Metadata, and every per-category tool — [Phase 3](phase-3.md)–[Phase
  5](phase-5.md).
- Multi-file/combined-output tools (Create PDF, Create Collage, Join
  Videos, Merge PDFs) and PDF-specific page tools (Organize PDF) — [Phase
  4](phase-4.md).

## Success criteria

- Every pairwise conversion in the full [tools.md](../tools.md) format
  matrix actually produces a valid output file (not just a UI state
  change), collision-safe named per [interaction.md](../interaction.md) —
  reachable directly from the wedge menu with no intermediate settings
  screen.
- FFmpeg sidecar bundled and invoked without requiring anything
  pre-installed on the user's machine; document/archive libraries likewise
  vendored, not assumed present.
- Test coverage: real conversions verified against known-good sample files
  (not mocked), on both Windows (cross-compiled at minimum, verified via
  VM/CI per [Phase 0](../phase0.md)) and Linux (real hardware, per the
  project's current verification setup).
- Directories and non-file URLs are still rejected outright (no
  partial/best-effort handling), per [tools.md](../tools.md).
- **This is the go-live gate** — once every conversion above is real and
  tested, Satsuma ships. Everything in [Phase 3](phase-3.md)–[Phase
  5](phase-5.md) lands afterward, prioritized by post-launch
  issues/bugs/feature requests rather than pre-committed order.
