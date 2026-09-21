# Phase 5 — advanced tools, wave 3

Niche and specialty single-file tools, last to land.

Part of the post-launch, gradually-released tool rollout — see
[Phase 3](phase-3.md) for the shared framing. Sequenced last because these
tools serve narrower use cases (creative editing, privacy redaction,
accessibility/niche formats) than the everyday tools in
[Phase 3](phase-3.md) and the multi-file tools in [Phase 4](phase-4.md) —
actual order is still driven by real post-launch feedback, not fixed in
advance.

## Scope

- **Images:** Edit Photo (exposure/color/detail adjustments with live
  preview), Add Background, Redact Photo.
- **Video:** Redact Video.
- **Audio:** Audio Visualizer, Convert Audio Channels, Bleep Audio.
- **GIF:** Read QR Codes.

## Design notes to carry through implementation (see [tools.md](../tools.md) for full detail)

- Redaction (photo/video) is explicitly *not* claimed to be secure/
  irreversible — blur/pixelation are visual masking only. This should be
  stated in Satsuma's own UI copy too, not just internal docs. Multi-page/
  animated source images are rejected outright rather than redacting one
  frame and silently dropping the rest.
- Edit Photo needs a real-time preview rendering pipeline, the most involved
  engineering lift of any tool in [tools.md](../tools.md) — a fair chunk of
  why it's sequenced last rather than because it's low-value.
- Convert Audio Channels needs independent left/right volume control with
  per-channel preview playback, not just a mono/stereo toggle.

## Explicitly not in this phase

Nothing further deferred within the tools rollout — this is the last wave.
[Linux desktop coverage hardening](linux-desktop-coverage-hardening.md)
remains a separate, unnumbered ongoing track, not part of this sequence.

## Success criteria

- Each tool actually performs its operation on a real file (not a stub),
  styled per [design.md](../design.md).
- Test coverage mirrors [Phase 2](phase-2.md)'s approach: real files in, real
  files out, verified.
- Full tool matrix from [tools.md](../tools.md) is now reachable from the
  wedge menu — this closes out the tools rollout.
