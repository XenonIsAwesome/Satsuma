import type { FileCategory, WedgeOption } from "../types";

/** Display label + icon for every extension the backend's
 * `list_conversion_targets` command can return, across all five Phase 2
 * families (image/video/audio/document/archive) plus the cross-family
 * JPG/PNG -> PDF/DOCX export and the TXT-linked subtitle formats. Backend
 * data drives *which* extensions appear (see `formatWedgeOptions` below);
 * this table only drives how a given extension is displayed, mirroring
 * the icon groupings Phase 0 used per category.
 */
const EXTENSION_DISPLAY: Record<string, { label: string; icon: string }> = {
  // Image
  jpg: { label: "JPG", icon: "🖼️" },
  png: { label: "PNG", icon: "🖼️" },
  webp: { label: "WebP", icon: "🖼️" },
  tiff: { label: "TIFF", icon: "🖼️" },
  avif: { label: "AVIF", icon: "🖼️" },
  bmp: { label: "BMP", icon: "🖼️" },
  // Video
  mp4: { label: "MP4", icon: "🎬" },
  mov: { label: "MOV", icon: "🎬" },
  mkv: { label: "MKV", icon: "🎬" },
  webm: { label: "WebM", icon: "🎬" },
  avi: { label: "AVI", icon: "🎬" },
  wmv: { label: "WMV", icon: "🎬" },
  gif: { label: "GIF", icon: "🎬" },
  // Audio
  mp3: { label: "MP3", icon: "🎵" },
  m4a: { label: "M4A", icon: "🎵" },
  wav: { label: "WAV", icon: "🎵" },
  flac: { label: "FLAC", icon: "🎵" },
  ogg: { label: "OGG", icon: "🎵" },
  opus: { label: "Opus", icon: "🎵" },
  aiff: { label: "AIFF", icon: "🎵" },
  wma: { label: "WMA", icon: "🎵" },
  // Documents (also the JPG/PNG -> PDF/DOCX export targets)
  pdf: { label: "PDF", icon: "📄" },
  docx: { label: "DOCX", icon: "📄" },
  txt: { label: "TXT", icon: "📝" },
  srt: { label: "SRT", icon: "📝" },
  vtt: { label: "VTT", icon: "📝" },
  // Archives
  zip: { label: "ZIP", icon: "🗜️" },
  tar: { label: "TAR", icon: "🗜️" },
  gz: { label: "GZ", icon: "🗜️" },
  rar: { label: "RAR", icon: "🗜️" },
};

/** Turns the raw extension list `list_conversion_targets` (a Tauri
 * command backed by `satsuma_core::supported_targets_for_selection`)
 * returns into the `{id, label, icon}` shape `WedgeMenu` renders,
 * skipping (rather than crashing on) an extension this table doesn't
 * recognize yet. */
export function formatWedgeOptions(targetExtensions: string[]): WedgeOption[] {
  return targetExtensions
    .map((extension) => {
      const display = EXTENSION_DISPLAY[extension];
      return display ? { id: extension, label: display.label, icon: display.icon } : null;
    })
    .filter((option): option is WedgeOption => option !== null);
}

/** Advanced-tool wedges. Per Phase 2's scope, every tool with its own
 * settings/GUI page (Compress, Crop, Trim, Split, Merge, ...) stays a
 * Phase 0-style fake selection (logged/displayed, no file written) —
 * deferred to Phase 3+. The one exception is Extract Archive: Tools.md
 * lists it as a single action with no settings screen, so it's real,
 * wired to the `extract_archive` command (see App.tsx/OverlayApp.tsx's
 * `handleSelect`), not a Phase 3 dedicated-tool-GUI candidate.
 */
const TOOL_OPTIONS: Record<FileCategory, WedgeOption[]> = {
  image: [
    { id: "compress", label: "Compress", icon: "📦" },
    { id: "crop", label: "Crop", icon: "✂️" },
  ],
  video: [
    { id: "compress", label: "Compress", icon: "📦" },
    { id: "crop", label: "Crop", icon: "✂️" },
    { id: "trim", label: "Trim", icon: "⏱️" },
    { id: "split", label: "Split", icon: "🔀" },
    { id: "merge", label: "Merge", icon: "➕" },
  ],
  audio: [
    { id: "compress", label: "Compress", icon: "📦" },
    { id: "trim", label: "Trim", icon: "⏱️" },
    { id: "split", label: "Split", icon: "🔀" },
    { id: "merge", label: "Merge", icon: "➕" },
  ],
  document: [{ id: "compress", label: "Compress", icon: "📦" }],
  archive: [{ id: "extract", label: "Extract Archive", icon: "📂" }],
  unknown: [],
};

/** Returns the advanced-tool wedge options for a given file category —
 * unlike the format list, this table is still local/static (see
 * `TOOL_OPTIONS`'s doc comment for why `extract` is the one real
 * exception). */
export function getToolOptions(category: FileCategory): WedgeOption[] {
  return TOOL_OPTIONS[category];
}
