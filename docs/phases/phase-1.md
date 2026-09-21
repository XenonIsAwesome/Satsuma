# Phase 1 — Settings screen & hotkey remapping

A real Settings screen — starting with making the modifier keys configurable, not hardcoded.

## Scope

[Phase 0](../phase0.md) hardcodes **Shift**/**Shift+Alt** as the format/tools modifiers, with no way to change them. This phase adds a real Settings screen, styled per [design.md](../design.md), and its flagship feature:

- **Hotkey remapping** — per [interaction.md](../interaction.md)'s "Modifier keys" section: let the user rebind the format-menu and tools-menu triggers to any nonempty combination of Ctrl/Alt/Shift/Win, not just the hardcoded Shift/Shift+Alt default. This matters for Satsuma because Windows/Linux desktop environments and users' existing global shortcuts are likely to already have Shift/Alt bound to something else.
  - The two bindings (format menu, tools menu) are independently remappable.
  - Live-validated: reject (or visibly warn on) a combination that collides with the other binding, and surface — where the OS/DE exposes it — a heads-up that the chosen combination is already a global shortcut for something else, rather than silently failing to trigger later.
  - Applies immediately to all three trigger paths from [interaction.md](../interaction.md) (in-app drag-and-drop, the borderless desktop overlay, and both platforms' file-manager triggers) — one setting, not per-path configuration.
- **Settings screen shell** — the actual surface these bindings (and other already-documented, currently-homeless settings) live on: the [design.md](../design.md) theme picker (including the Custom color pickers), the Linux file-manager integration install/uninstall toggle (already implemented per [interaction.md](../interaction.md)'s "Fallbacks," currently exposed via its own panel rather than a unified Settings screen), and "Start at Login" (currently only reachable from the tray menu per [Phase 0](../phase0.md) — mirrored into Settings here, not moved, since the tray shortcut stays for quick access).

## Why this phase, and why here

Settings is real, user-facing behavior — in the same "looks and behavior" category [Phase 0](../phase0.md) otherwise claims entirely — but it's broken out as its own phase rather than folded into Phase 0 because it's a self-contained surface (a whole new screen, not a wedge-menu addition) that depends on Phase 0's modifier-key detection (`useModifierKeys`) already existing to remap. Sequencing it right after Phase 0 and before the real conversion engine means Phase 2 onward can assume a real, user-configurable trigger setup instead of a hardcoded one baked into tests.

## Explicitly not in this phase

- No new format/tool functionality — this phase touches triggers and app-level preferences only, nothing about what the wedge menu can convert or do.
- No per-file or per-conversion settings (those belong to individual tools in [Phase 3](phase-3.md)) — this Settings screen is exclusively app-wide/personalization state, matching [design.md](../design.md)'s theme-picker note that it "persists per-user... it's a personalization setting, not part of the document/file model."

## Success criteria

- Rebinding either modifier combination in Settings changes what triggers the format/tools menu across all three trigger paths, with no restart required.
- A colliding or already-globally-bound combination is caught and surfaced to the user rather than silently accepted and failing later.
- The theme picker and Linux integration toggle are reachable from the same Settings screen as the hotkey remapping, not scattered across separate ad hoc panels.
