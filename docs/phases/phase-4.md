# Phase 4 — advanced tools, wave 2

Multi-file/combined-output tools and PDF-specific page tools.

Part of the post-launch, gradually-released tool rollout — see
[Phase 3](phase-3.md) for the shared framing. This wave is grouped
separately because these tools share the multi-file selection/ordering
machinery from [interaction.md](../interaction.md), which the single-file
tools in [Phase 3](phase-3.md) don't need.

## Scope

- **Multi-file tools:** Create PDF (combine several images, mixed
  formats/SVGs, one page each), Create Collage (grid/row/column/featured
  layout), Join Videos (combine multiple clips into one MP4, first clip sets
  the canvas), Merge PDFs (combine several PDFs, reorderable from the
  file-manager selection order).
- **PDF-specific tools:** Organize PDF (visually reorder, rotate, duplicate,
  or remove pages).

These require the multi-file selection rules from
[interaction.md](../interaction.md) (per-file-setting tools vs.
combined-output tools) to be wired into the wedge menu's selection handling
— the one piece of shared plumbing this wave depends on that
[Phase 3](phase-3.md) doesn't.

## Design notes to carry through implementation (see [tools.md](../tools.md) for full detail)

- Create PDF/Create Collage/Join Videos/Merge PDFs all start from the
  file-manager's selection order and let the user reorder before finalizing.
- Organize PDF is page-level (reorder/rotate/duplicate/remove), not
  document-level.

## Explicitly not in this phase

- Single-file everyday tools (Compress, Crop, Trim, etc.) —
  [Phase 3](phase-3.md).
- Photo-editing, background, redaction, visualizer, channel-conversion, and
  bleep tools — [Phase 5](phase-5.md).

## Success criteria

- Each tool actually performs its operation on real files (not a stub), with
  reorderable-selection UI styled per [design.md](../design.md).
- Batch/multi-file tools respect the combined-vs-per-file selection rules
  from [interaction.md](../interaction.md).
- Test coverage mirrors [Phase 2](phase-2.md)'s approach: real files in, real
  files out, verified.
