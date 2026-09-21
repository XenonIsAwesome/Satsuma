<p align="center"><img src="assets/satsuma-logo.svg" alt="Satsuma logo" width="120" height="120" /></p>

# Satsuma

[![CI](https://github.com/XenonIsAwesome/Satsuma/actions/workflows/ci.yml/badge.svg)](https://github.com/XenonIsAwesome/Satsuma/actions/workflows/ci.yml)
[![docs](https://img.shields.io/endpoint?url=https://xenonisawesome.github.io/Satsuma/docs-coverage-badge.json)](https://xenonisawesome.github.io/Satsuma/)
[![Rust coverage](https://img.shields.io/endpoint?url=https://xenonisawesome.github.io/Satsuma/rust-coverage-badge.json)](https://github.com/XenonIsAwesome/Satsuma/actions/workflows/ci.yml)
[![Frontend coverage](https://img.shields.io/endpoint?url=https://xenonisawesome.github.io/Satsuma/frontend-coverage-badge.json)](https://github.com/XenonIsAwesome/Satsuma/actions/workflows/ci.yml)
[![clippy](https://img.shields.io/endpoint?url=https://xenonisawesome.github.io/Satsuma/clippy-badge.json)](https://github.com/XenonIsAwesome/Satsuma/actions/workflows/ci.yml)
[![cargo-deny](https://img.shields.io/endpoint?url=https://xenonisawesome.github.io/Satsuma/deny-badge.json)](https://github.com/XenonIsAwesome/Satsuma/actions/workflows/ci.yml)
[![cargo-audit](https://img.shields.io/endpoint?url=https://xenonisawesome.github.io/Satsuma/rust-audit-badge.json)](https://github.com/XenonIsAwesome/Satsuma/actions/workflows/ci.yml)
[![npm audit](https://img.shields.io/endpoint?url=https://xenonisawesome.github.io/Satsuma/npm-audit-badge.json)](https://github.com/XenonIsAwesome/Satsuma/actions/workflows/ci.yml)
[![knip](https://img.shields.io/endpoint?url=https://xenonisawesome.github.io/Satsuma/knip-badge.json)](https://github.com/XenonIsAwesome/Satsuma/actions/workflows/ci.yml)

Satsuma is an offline, cross-platform file converter for Windows and Linux, inspired by [Tangerine](https://tangerineformac.com/) (macOS-only). Drag a file onto the app — or select it in your file manager and press Shift — and a radial menu made of citrus-slice-shaped wedges lets you pick an output format or an advanced tool (compress, crop, trim, split, merge). Everything runs locally: no uploads, no network calls for conversion.

"Tangerine" is the name of the app this project draws inspiration from, not this project's name — "Satsuma" is intentionally a different citrus to avoid confusion/trademark overlap.

> **Trademark / no-affiliation notice:** Satsuma is an independent project and is **not affiliated with, endorsed by, or sponsored by** Tangerine, tangerineformac.com, or its developer(s). "Tangerine" is referenced here solely to describe the app that inspired this project's interaction model; no Tangerine trademarks, logos, source code, or other copyrighted assets are used in or distributed with Satsuma. All code, icons, and other assets in this repository are original works of the Satsuma project unless otherwise noted.

## Tech stack

- **App shell:** [Tauri](https://tauri.app/) (Rust backend), v2
- **Frontend:** React + TypeScript, rendered in Tauri's webview (WebView2 on Windows, WebKitGTK on Linux)
- **Conversion engine:** Rust, calling out to FFmpeg (video/audio, bundled as a sidecar binary), the `image` crate (images), and dedicated document/archive libraries (PDF/DOCX/TXT, ZIP/TAR/GZIP/RAR) — real conversion, implemented in Phase 2.
- **UI:** SVG + CSS for the citrus-slice wedge menu and its animations

## Target scope

**Formats** (convert between all of these, within each group):
- Images: JPG, PNG, WebP, HEIC (read at minimum), TIFF, SVG (input-only), AVIF, BMP — plus JPG/PNG export to PDF/DOCX
- Video: MP4, MOV, MKV, WebM, AVI, WMV, GIF — plus MP3 audio export
- Audio: MP3, M4A, WAV, FLAC, OGG, Opus, AIFF, WMA
- Documents/Text/Subtitles: PDF → DOCX/JPG/PNG/TXT, JPG/PNG → PDF/DOCX, TXT → PDF/JPG/PNG/SRT/VTT, and pairwise TXT/SRT/VTT conversion
- Archives: every pairwise ZIP/TAR/GZIP/RAR conversion, plus Extract Archive

**Advanced tools:** Compress, Crop (images/video), Trim (video/audio), Split (video/audio), Merge (same-type files)

**Explicitly out of scope for now** (architecture leaves room to add these later without a rewrite): any tool with its own dedicated settings/GUI page (Compress, Edit Metadata, Crop, Edit Photo, Trim, and the rest), multi-file/combined-output tools (Create PDF from multiple images, Create Collage, Join Videos, Merge PDFs), and PDF page-organizing. PPTX/XLSX and 7Z are out of scope entirely.

See [docs/tools.md](docs/tools.md) for the full target format/tool matrix and behavioral details, and [docs/phases/functionality-phases.md](docs/phases/functionality-phases.md) for how the phases get there.

## Interaction model

Satsuma runs tray-resident in the background (with an optional "start at login" setting) rather than needing to already be open — closing its window just hides it; quit from the tray icon. There are two ways to open the wedge menu:

1. **Drag-and-drop** (all platforms): drag a file onto the app window and drop it. Hold **Shift** while dragging (or right after dropping) to open the format wedge menu; add **Alt** to switch to advanced-tools mode. Keyboard-only path: drop a file, press **Enter** to open the menu, arrow keys to move between wedges, **Enter** to apply, **Escape** to cancel.
2. **File-manager trigger** (no drag needed): opens a borderless, transparent overlay window right at the current cursor position — not the normal titled window — with the wedge menu on it directly.
   - **Windows:** select a file in Explorer and press **Shift** — a global hook detects this, reads Explorer's current selection via COM automation, and shows the overlay (**Alt** held too → tools mode). See [Platform verification status](#platform-verification-status).
   - **Linux:** right-click a file for a "Convert with Satsuma" / "Satsuma Tools" context-menu entry (Nautilus scripts, Nemo actions, KDE Dolphin service menu), or use "Open With → Satsuma" everywhere else. Opt in from the in-app "file-manager integration" toggle (Linux only). Overlay positioning needs an X11 session — it falls back to centering on the primary monitor under Wayland, which doesn't generally expose the cursor position to ordinary clients.

On conversion, the output file is written next to the original (configurable output folder planned) with a brief success/failure state.

See [docs/interaction.md](docs/interaction.md) for the full interaction model and [docs/design.md](docs/design.md) for the visual identity behind it.

## Architecture

```
crates/satsuma-core/    Pure Rust conversion/detection engine — no Tauri
                        dependency, unit-testable on its own, reused by
                        both the Tauri command layer and (for pure logic)
                        the platform-integration modules.
  file_type.rs            File extension → category (image/video/audio) detection
  launch_request.rs        `--mode=formats|tools <path>...` argv parsing, shared
                            between the Linux context-menu launch path and
                            (in principle) any other external trigger
  overlay_geometry.rs       Pure screen-rect placement math (cursor/item-rect →
                            top-left position) used to place the overlay window,
                            kept OS-agnostic so it's testable without a Windows
                            toolchain

src-tauri/              The Tauri app shell and command layer.
  src/lib.rs               Tauri commands (detect_file_category, launch-request
                            plumbing, Linux-integration install/uninstall,
                            hide_overlay), tray icon + menu (Show/Start at
                            Login/Quit) and close-to-tray window interception,
                            show_overlay (positions + reveals the borderless
                            overlay window), single-instance plugin wiring
  src/linux_integration.rs Installs/removes the Linux file-manager integration
                            files under the user's XDG data dirs (Linux only);
                            also the X11 cursor_position() query used to place
                            the overlay window (falls back to monitor-center on
                            Wayland)
  src/windows_integration.rs
                           Global Shift hook + Explorer-selection COM query +
                           GetCursorPos-based cursor_position() (Windows only
                           — see verification status below)
  resources/linux/         The actual .desktop / Nautilus script / Nemo action /
                            KDE service menu files installed by linux_integration.rs

src/                    React frontend (Vite + TypeScript).
  main.tsx                 Branches on the `?window=overlay` URL flag to mount
                            either App (normal window) or OverlayApp
  App.tsx / OverlayApp.tsx App: drag-and-drop into the normal titled window.
                            OverlayApp: just the transparent-background
                            WedgeMenu, driven by useLaunchRequest, for the
                            borderless overlay window that hotkey/context-menu
                            triggers open
  components/              DropZone, WedgeMenu (the SVG radial menu),
                            LinuxIntegrationPanel
  hooks/                   useFileDrop (native OS drag-and-drop), useModifierKeys
                            (Shift/Alt tracking), useLaunchRequest (file-manager
                            trigger plumbing, used by OverlayApp)
  data/wedgeOptions.ts      Per-category format/tool lists driving the wedge menu
                            — backend-queried as of Phase 2, via the
                            `list_conversion_targets` Tauri command
  lib/                      Path/extension helpers, wedge geometry math, thin
                            Tauri command wrappers
```

The conversion/detection logic in `satsuma-core` is deliberately independent of the Tauri command layer, so it stays testable on its own and easy to extend with new formats later. FFmpeg is invoked as a bundled sidecar binary (Phase 2), not a system dependency. Drag-and-drop uses Tauri's native file-drop events (not HTML5 drag events) to get real file paths on both Windows and Linux. The wedge menu is a reusable component driven by a list of `{id, label, icon}` objects, so adding formats later is a data change, not a UI rewrite.

## Current status

**Phase 0 (working GUI with fake options) is complete** — see [docs/phase0.md](docs/phase0.md) for the full writeup: what was built, test coverage, and known gaps. In short: the drag-and-drop zone, the styled wedge menu (SVG, keyboard nav, Shift/Alt modes, the Citrus visual identity), tray-resident background mode + the borderless overlay, and file-manager trigger paths (Windows Explorer-selection, Linux context menus) are all wired up.

**Phase 2 (real conversion engine, the launch phase) has landed** — see [docs/phases/phase-2.md](docs/phases/phase-2.md). The wedge menu's format list is now backend-queried (`list_conversion_targets`), and selecting a format wedge runs a real conversion (`convert_file`) instead of Phase 0's byte-copy stub, across all five format families from [docs/tools.md](docs/tools.md):

- **Images** (JPG/PNG/WebP/TIFF/BMP/AVIF, plus JPG/PNG → PDF/DOCX export) — fully real via the `image` crate, with SVG rasterization (input-only). HEIC decoding and AVIF decoding are **not** implemented (no permissively-licensed, no-system-dependency decoder was available for either — see `crates/satsuma-core/src/convert/image.rs`'s module doc comment); HEIC still appears as a source format and reports a clear error, AVIF is never offered as a source at all (only as a target).
- **Video/Audio** — fully real via a bundled FFmpeg sidecar (`scripts/fetch-ffmpeg.sh`/`.ps1` vendor the actual binary — it's gitignored, so run them once per checkout before any `cargo build`/`cargo test`; see AGENTS.md).
- **Documents/Text/Subtitles** — PDF text extraction and PDF/DOCX writing are fully real and native-dependency-free; PDF → JPG/PNG rasterization (and the scanned-page image fallback of PDF → DOCX) needs a vendored pdfium library (`scripts/fetch-pdfium.sh`/`.ps1`; gitignored like the FFmpeg sidecar, and at runtime the resolver also falls back to a system-installed `libpdfium.so` on Linux) and gracefully reports "unavailable" without one. The release pipeline (`scripts/smoke-pdfium.sh`/`.ps1`, run in `deploy.yml` right after fetching) runs a real end-to-end PDF → JPG rasterization against the vendored build before it ships, so a pinned pdfium release that's drifted incompatible with this app's `pdfium-render` bindings fails the release instead of shipping a silent break — it just hasn't been exercised in this repo's own default `cargo test`/CI unit-test run, since that never vendors the library (see that script's comments).
- **Archives** — every pairwise ZIP/TAR/GZIP/RAR conversion plus Extract Archive, including a hand-rolled RAR5 (store-method) writer cross-validated against system `unrar`/`7z`. **RAR reading depends on the `unrar` crate, which bundles RARLAB's own UnRAR source under its own non-OSI license** (not MIT/Apache/BSD) — see `crates/satsuma-core/src/convert/archive.rs`'s module doc comment for the full judgment call. This needs the project owner's explicit sign-off before shipping a build that includes it.

Advanced per-category tools (Compress, Crop, Trim, Edit Metadata, etc.) remain out of scope until Phase 3+; Extract Archive is the one tool wedge that's real today; all others still stay Phase 0-style fake selections.

The FFmpeg and pdfium paths are user-overridable from Settings → External tools (`ffmpeg_path`/`pdfium_path` in `Settings`) — pick your own binary/library file instead of the bundled one, with a live status line showing what's actually in effect. An override that doesn't point at a real file is used as-is (reported as "not found") rather than silently falling back to the bundled copy, so a broken custom path doesn't fail confusingly later.

## Platform verification status

- **Linux**: built, run, and tested on real hardware (Linux Mint/Cinnamon/Nemo), including the tray-resident background app and the borderless overlay window. See [docs/phase0.md](docs/phase0.md) for a known Mutter/Muffin focus-stealing quirk on this platform.
- **Windows** (`src-tauri/src/windows_integration.rs`, the Explorer-selection + Shift trigger): verified via a `windows-latest` CI runner (real native build+link+`cargo test` on every push/PR, though it only exercises the module's pure-logic unit test) plus manual click-through testing in a Windows VM. Cross-compiling this module without a Windows machine (`cargo check --target x86_64-pc-windows-gnu`) type-checks but does not link — a real `cargo build`/`cargo tauri build` needs the MSVC toolchain (`x86_64-pc-windows-msvc`) on actual Windows.
- **Tray icon and autostart-at-login**: exercised on Linux; the autostart entry actually surviving a reboot/login on both platforms, and the tray icon's real rendering/menu interaction, haven't been verified end-to-end on real hardware yet.

Linux tray icon support needs `libayatana-appindicator3-dev` (or your distro's equivalent) installed at build time.

## Development

```bash
npm install
scripts/fetch-ffmpeg.sh   # or fetch-ffmpeg.ps1 on Windows — required once per checkout,
scripts/fetch-pdfium.sh   # or fetch-pdfium.ps1 on Windows — see "Current status" above
npm run tauri dev          # launches the app (Rust + webview)
```

The two fetch scripts vendor the FFmpeg sidecar binary and the pdfium shared library into `src-tauri/lib/` (gitignored, no checked-in placeholder). They're required before `npm run tauri dev` or any `cargo build`/`cargo check`/`cargo test` that compiles the `satsuma` crate — without them, Tauri's build script fails with `resource path 'lib/ffmpeg' doesn't exist`. Commands that only touch `satsuma-core` or the frontend (`cargo test -p satsuma-core`, `npm run test`) don't need this.

### Testing

Every phase requires test coverage on both sides before moving to the next one.

```bash
npm run test          # frontend: Vitest + React Testing Library
cargo test             # backend: Rust unit tests (workspace-wide)
```

To type-check the Windows-only backend module without a Windows machine (compiles, does not link). The build script needs `windres`/`gcc` for the `x86_64-pc-windows-gnu` target — on Debian/Ubuntu, `binutils-mingw-w64-x86-64` and `gcc-mingw-w64-x86-64` (not `mingw-w64-tools`, which despite the name doesn't include `windres`):

```bash
sudo apt-get install -y binutils-mingw-w64-x86-64 gcc-mingw-w64-x86-64
rustup target add x86_64-pc-windows-gnu
cargo check -p satsuma --target x86_64-pc-windows-gnu
```

### Project layout at a glance

- `Cargo.toml` — Cargo workspace root (`src-tauri` + `crates/satsuma-core`)
- `package.json` / `vite.config.ts` / `tsconfig.json` — frontend tooling (Vite, Vitest, TypeScript)

## Recommended IDE setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## Contributing

Contributions are welcome — see [CONTRIBUTING.md](CONTRIBUTING.md) for setup, testing, and PR guidelines, including this project's policy on AI-assisted contributions.

**AI-development notice:** this codebase is developed with the help of AI coding tools (Claude Code). If that matters to you as a user or a potential contributor, see [CONTRIBUTING.md](CONTRIBUTING.md#ai-assisted-contributions) for what that means in practice.

## License

MIT — see [LICENSE](LICENSE). This license covers the Satsuma project's own code and assets only; it does not grant any rights to third-party trademarks (including "Tangerine") referenced for descriptive purposes above. See [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md) for the licenses of bundled dependencies and vendored native components (FFmpeg, pdfium, RAR-reading support).
