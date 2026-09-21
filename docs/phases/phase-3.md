# Phase 3 — advanced tools, wave 1

The everyday single-file tools, first to land after launch.

[Phase 3](phase-3.md)–[Phase 5](phase-5.md) cover every [tools.md](../tools.md)
entry that opens its own dedicated settings/GUI page, as opposed to
[Phase 2](phase-2.md)'s plain format conversions. They ship gradually after
Phase 2 goes live, prioritized by real post-launch issues/bugs/feature
requests rather than a fixed pre-launch order — the split into three waves
below is a starting prioritization, not a hard commitment.

## Scope

The tools people reach for most, and the ones with the most reusable
engineering underneath (single input file, single output file, no
ordering/arrangement UI):

- **Any file:** Compress (Balanced/Strong presets, optional resize), Edit
  Metadata.
- **Images:** Crop Image.
- **Video:** Crop Video, Trim Video, Change Video Speed, Split Video, Video
  Snapshots ("Save Video Frames").
- **Audio:** Trim Audio, Normalize Volume.

As with [Phase 2](phase-2.md), these are built in parallel by category rather
than sequentially — there's no dependency forcing image tools to land before
video/audio tools.

## Design notes to carry through implementation (see [tools.md](../tools.md) for full detail)

- Metadata editing rewrites container metadata in place rather than
  decode/re-encode, to avoid quality loss.
- Compress is a *target*, not a guarantee — UI copy should say "up to
  ~20%/~50%" rather than promise an exact number.
- Crop Image takes exact pixel dimensions or a preset/custom aspect ratio via
  a drag region; Crop Video takes exact output dimensions and preserves
  original audio (no drag needed, per [tools.md](../tools.md)).
- Trim (video and audio) supports both frame-by-frame/waveform stepping and
  exact start/end timecodes.

## Explicitly not in this phase

- Multi-file/combined-output tools and PDF-specific page tools —
  [Phase 4](phase-4.md).
- Photo-editing, background, redaction, visualizer, channel-conversion, and
  bleep tools — [Phase 5](phase-5.md).

## Success criteria

- Each tool actually performs its operation on a real file (not a stub),
  with a settings UI matching the parameters implied in
  [tools.md](../tools.md) (e.g. Crop takes exact pixel dimensions or a
  preset aspect ratio, not just "crop" with no parameters), styled per
  [design.md](../design.md) (buttons, tabs, cards) rather than
  default/unstyled form controls.
- Test coverage mirrors [Phase 2](phase-2.md)'s approach: real files in, real
  files out, verified.
