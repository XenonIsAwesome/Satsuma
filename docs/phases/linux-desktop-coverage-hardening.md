# Linux desktop coverage hardening

Broader Linux file-manager coverage — polish beyond the one Linux setup that
actually matters for Phase 0.

Pulled out of the numbered phase sequence: it's a platform-verification
track, not a tool-rollout one, so it doesn't fit
[Phase 3](phase-3.md)/[Phase 4](phase-4.md)/[Phase 5](phase-5.md)'s
simple-vs-complex-tools split. Track it as ongoing hardening, picked up
whenever convenient rather than gated behind a specific tools phase.

Tray-resident mode, the borderless overlay window, and Windows verification
all live in [Phase 0](../phase0.md) — they're core looks/behavior, not later
hardening. What's left here is narrower: **Linux Mint with Nemo** is the
real daily-driver test target and is already required to be solid in
[Phase 0](../phase0.md) (with real-hardware testing behind it). Everything
else Linux-side is lower-priority polish for community setups not primarily
tested against day to day.

## Scope

- **Broader Linux DE coverage** — verifying the already-implemented
  GNOME/Nautilus scripts integration and KDE Dolphin service menu on real
  installs of each (the code exists per [interaction.md](../interaction.md)'s
  "Fallbacks," but only Nemo has real-hardware verification behind it so
  far).
- **The generic `.desktop`/"Open With" fallback** — verified on at least one
  file manager outside the Nautilus/Nemo/Dolphin set, for whatever community
  file managers turn up.
- **Install/uninstall flow hardening** for the XDG-based context-menu files
  across those additional environments — the same install/uninstall/
  is-installed logic Nemo already exercises, just confirmed elsewhere too.

## Why this is lower priority

Nemo is the file manager actually in daily use for testing this project, and
it's already covered by [Phase 0](../phase0.md)'s own completion bar.
Nautilus/Dolphin/everything-else are real, already-implemented code paths
(not missing features) — what's missing is verification on real installs,
which matters far less until/unless Satsuma has users on those desktops.

## Success criteria

- Nautilus (GNOME Files) integration verified on a real GNOME install.
- KDE Dolphin service menu verified on a real KDE install.
- The generic `.desktop`/"Open With" fallback verified on at least one file
  manager outside Nautilus/Nemo/Dolphin.
