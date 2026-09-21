# Interaction model

Satsuma has several ways to open the menu, all converging on the same citrus-slice radial "wedge" menu. This note covers *behavior* — what opens the menu and what it does; see [design.md](design.md) for how it actually looks (the frosted-glass petals, the hub, hover states, colors, and theming).

**Satsuma app:**
The Tauri app includes a drag-and-drop area where you put a file. When you drop a file and press and hold Shift, the conversion radial menu opens; adding Alt replaces it with the tools radial menu. Keyboard-only path: drop a file, press **Enter** to open the menu (formats mode), use arrow keys to move between wedges, **Enter** to apply the highlighted wedge, **Escape** to cancel without acting.

**Tray-resident:**
Satsuma runs tray-resident in the background (with an optional "start at login" setting) rather than needing to already be open — closing its window just hides it; quit from the tray icon. The tray icon has a button to open the Satsuma app.

**Borderless overlay:**
When selecting a file in the desktop/file manager and holding Shift or Shift+Alt, the radial menu opens without the Satsuma app window being shown: a borderless window with no decorations and a transparent background, centered on the cursor position — see [docs/tray-resident-overlay-design.md](tray-resident-overlay-design.md) for the design rationale. The wedge itself renders identically to the in-app version per [design.md](design.md) — same frosted-petal look, just hosted in a transparent window instead of the app's own.

**File manager:**
"Satsuma Tools" and "Convert with Satsuma" entries in the file manager's context menu open the radial menu the same way as the borderless-overlay path, without requiring a drag gesture at all.

**"Open with":**
Opening a file "with" Satsuma opens the radial menu the same way as the borderless-overlay path.

---

## Menu content rules

These govern what the wedge menu actually shows, independent of which trigger opened it (see [tools.md](tools.md) for the underlying format/tool matrix):

- **Capability-aware:** the source file's own format is never offered as a conversion target for itself, and a target only appears if the engine needed to produce it is actually available on the current platform/build.
- **Multi-file selection:** selecting several files of the *same* format keeps that format's normal target list. A mixed selection within the same family (e.g. several image formats) shows only the targets common to *every* selected file. Unrelated families in one selection (e.g. an image plus a video) show nothing — no menu, or a disabled state.
- **Per-file vs. combined tools:** tools needing per-file settings (Crop, Trim, Change Speed, Convert Audio Channels) are only offered for a single selected file. Tools whose settings describe one *combined* output (Join Videos, Create Collage, Create PDF from multiple images, Merge PDFs) accept and require multiple files instead.
- **Non-destructive by default:** the source file is never modified. Every result is written as a new sibling file. Output naming is collision-safe: `cat.jpg` → `cat 2.jpg` → `cat 3.jpg`, never silently overwriting an existing file. A configurable output folder is planned; until then, output lands next to the original.

## Modifier keys

Default bindings: **Shift** opens the format menu, **Shift+Alt** opens the tools menu. Satsuma offers remapping both to any nonempty combination of Ctrl/Alt/Shift/Win as a settings option (see [phases/phase-1.md](phases/phase-1.md)) rather than a hardcoded default, since modifier availability and conventions differ across Windows/Linux desktop environments and users may have conflicting global shortcuts already bound to Shift/Alt.

## Progress & completion feedback

- Batch operations (multiple files) show current-file progress combined with a count-of-files-done overall percentage, not just a single spinner.
- A real percentage is shown when the engine reports measurable progress (e.g. encode timestamps against known duration); an indeterminate spinner is used otherwise — never a fabricated percentage.
- A conversion's result state isn't reported as "done" until the output file is confirmed written via a fresh read of the destination, not merely once the worker process exits — and a failure is reported clearly (source filename + error detail) rather than the operation silently disappearing.

---

## Fallbacks

Satsuma targets both Windows and Linux, and some triggers are necessarily platform-specific:

Linux has many file managers, so it relies on fallbacks and community implementations:
- **GNOME Files (Nautilus):** Scripts integration
- **Nemo (Cinnamon):** Nemo Actions
- **KDE Dolphin:** service menu (`.desktop`-based)
- **Everything else:** standard `.desktop` MIME association, surfaced as "Open With → Satsuma" — the universal fallback since there's no cross-file-manager API for "currently selected file" on Linux.

Windows relies on a global low-level keyboard hook that watches for Shift while Explorer is the foreground window, then queries Explorer's current selection via `Shell.Application` COM automation — implemented, verified via a combination of a `windows-latest` CI runner (real build/link/unit-test pass on every push) and manual click-through testing in a Windows VM. See [phase0.md](phase0.md) for current status.

The **Satsuma app** (drag-and-drop window) is the default fallback in case a given OS/file-manager integration is impossible or not yet implemented.
