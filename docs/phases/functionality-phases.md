# Development phases

Satsuma is built phase by phase, each one shippable and fully tested
(frontend + backend) before the next starts — never a half-working format or
tool. Each phase gets its own note in this folder.

**Phase 0 carries almost all of the looks and behavior.** Everything about
how Satsuma looks and how it's triggered/used — the
[design](../design.md)-styled wedge menu, the full
[interaction](../interaction.md) model, tray-resident background mode, the
borderless desktop overlay, Windows/Linux file-manager triggers — is in
scope for Phase 0. The *only* thing Phase 0 doesn't have is real
conversion/tool engines: format wedges write a stub sibling file (same name,
swapped extension, no real transcoding) and advanced-tool wedges are no-ops.
[Phase 1](phase-1.md) adds one more piece of behavior on top — a real
Settings screen, with hotkey remapping as its flagship feature.

**From [Phase 2](phase-2.md) on, the dividing line is complexity of
implementation, not format family, and categories are built in parallel
rather than family-by-family.** Phase 2 is the entire format-conversion
matrix from [tools.md](../tools.md) — images, video, audio, documents,
archives, subtitles — because none of it needs a dedicated tool GUI, just a
wedge-menu format pick backed by FFmpeg/the `image` crate/a document or
archive library. **Phase 2 is the launch phase: Satsuma goes live once it
ships.** Everything that opens its own dedicated settings/GUI page (Compress,
Edit Metadata, Crop, Edit Photo, Trim, and the rest) ships afterward in three
gradually-released waves, [Phase 3](phase-3.md)–[Phase 5](phase-5.md),
prioritized by real post-launch issues/bugs/feature requests rather than a
fixed pre-launch order. Verifying broader Linux desktop-environment coverage
beyond the one setup actually tested day to day (Linux Mint/Nemo) is a
separate, unnumbered ongoing track — see
[Linux desktop coverage hardening](linux-desktop-coverage-hardening.md) —
since it's a platform-verification concern, not a tools-rollout one.

- [Phase 0](../phase0.md) — everything look- and behavior-wise (per
  [design](../design.md) and [interaction](../interaction.md)), including
  tray-resident mode, the borderless overlay, and both platforms'
  file-manager triggers; conversion wedges write a stub sibling file;
  advanced-tool wedges stay fake. **Complete.**
- [Phase 1](phase-1.md) — a real Settings screen: hotkey remapping for the
  format/tools modifier keys, plus a home for the already-designed theme
  picker and Linux integration toggle. **Complete.**
- [Phase 2](phase-2.md) — **launch phase, complete.** The full conversion
  engine and format matrix (images, video, audio, PDF/DOCX/TXT, archives,
  subtitles), replacing the stub file-write with backend-driven, real
  encoding — every category built in parallel, since none of them share a
  GUI or block on each other. See [README.md](../../README.md#current-status)
  for exactly what shipped.
- [Phase 3](phase-3.md) — advanced tools, wave 1: the everyday single-file
  tools (Compress, Edit Metadata, Crop, Trim, Change Speed, Split,
  Snapshots, Normalize Volume).
- [Phase 4](phase-4.md) — advanced tools, wave 2: multi-file/combined-output
  tools (Create PDF, Create Collage, Join Videos, Merge PDFs) and
  PDF-specific page tools (Organize PDF).
- [Phase 5](phase-5.md) — advanced tools, wave 3: niche/specialty tools
  (Edit Photo, Add Background, Redact Photo/Video, Audio Visualizer, Convert
  Audio Channels, Bleep Audio, Read QR Codes).

Rationale for this order: Phase 0 de-risks the entire UI/interaction/
platform-integration surface (the part most unique to this project, and the
part that would otherwise be hardest to retrofit later) before any
conversion code exists at all — including the parts (tray, overlay, OS
triggers) that might otherwise look like "polish," because none of them
depend on a working conversion engine to build or test. Phase 1 is the last
purely-behavioral piece — a self-contained new screen, sequenced right after
Phase 0 because it depends on Phase 0's modifier-key detection already
existing to remap, and sequenced before any engine work so Phase 2 onward
can assume a real, user-configurable trigger setup instead of one hardcoded
into tests. Phase 2 is scoped to the entire conversion matrix, not just a
"core" subset, because every format conversion is equally cheap from a GUI
standpoint (no dedicated tool screen, just the wedge menu Phases 0–1 already
finished) — bundling them all into one launch-gating phase gets the whole
matrix live in one push instead of trickling it out. Phases 3–5 then work
through every tool that *does* need its own dedicated GUI, split into three
waves by how much shared plumbing they need (everyday single-file tools,
then multi-file/PDF-page tools that need the selection-ordering machinery,
then niche tools like the Edit Photo live-preview pipeline) — released
gradually after launch and reprioritized by real user feedback rather than
locked in advance. Linux desktop-environment coverage beyond Mint/Nemo is
deliberately left out of this sequence entirely, tracked separately, because
it verifies code paths that already exist rather than shipping new tools.
