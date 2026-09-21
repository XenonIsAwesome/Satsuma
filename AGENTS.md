# CLAUDE.md

This file provides guidance to AI coding agents when working with code in this repository.

## AI disclosure

The AI agent should co-author the commits it makes and disclose itself in PR
descriptions — the same way Claude Code does: append a `Co-Authored-By:`
trailer to the commit message, and an AI-generated footer to PR
overviews/comments, identifying the assistant that actually produced the
change.

**This repo has multiple AI coding tools/agents contributing to it, running
different underlying models — there is no one fixed trailer that's correct
for all of them.** Copying an example you saw used by a *different* agent
(e.g. a Claude-based agent copying a trailer that names an OpenCode agent,
or vice versa) is a disclosure bug, not a style choice — it misattributes
the change to the wrong assistant. Before constructing either line, work out
the answers for *yourself*, not for whichever tool/harness happens to be
running you:

1. **What model am I?** Your own model name/version — not the name of the
   harness/CLI running you. ("Claude Sonnet 5", not "Claude Code" or
   "opencode"; "GPT-5", not the name of whatever wraps it.)
2. **Who is my provider, and what is their website?** The company that
   makes the model itself (e.g. Anthropic → anthropic.com), which may be a
   different company than whoever built the harness/tool you're running
   inside.
3. **Does that provider have a publicly known `noreply@` address?** Use it
   if you know of one. If not, don't invent a plausible-looking address —
   derive one from the provider's own domain: `noreply@<provider-domain>`.

Commit trailer:

```text
Co-Authored-By: <Model Name> <noreply@<provider-domain>>
```

Pull request overviews and comments on PRs should disclose that they are AI generated, by appending a footer in this form:

```text
🤖 Generated with [<Model Name>](<Link to provider website>)
```

Worked examples — each only correct for the agent that actually produced it, never copy one verbatim without redoing the three questions above for yourself:

```text
Co-Authored-By: Big Pickle <noreply@anoma.ly>
```

```text
🤖 Generated with [Big Pickle](https://opencode.ai/)
```

```text
Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
```

```text
🤖 Generated with [Claude Sonnet 5](https://anthropic.com/)
```

## graphify

This project has a knowledge graph at graphify-out/ with god nodes, community structure, and cross-file relationships.

Rules:
- For codebase questions, first run `graphify query "<question>"` when graphify-out/graph.json exists. Use `graphify path "<A>" "<B>"` for relationships and `graphify explain "<concept>"` for focused concepts. These return a scoped subgraph, usually much smaller than GRAPH_REPORT.md or raw grep output.
- If graphify-out/wiki/index.md exists, use it for broad navigation instead of raw source browsing.
- Read graphify-out/GRAPH_REPORT.md only for broad architecture review or when query/path/explain do not surface enough context.
- After modifying code, run `graphify update .` to keep the graph current (AST-only, no API cost).
- Don't give the `graphify update .` output its own commit. Fold the refreshed `graphify-out/` files into whatever commit contains the code change that triggered the update — a graph refresh isn't a change worth reviewing or bisecting on its own.

## Project

Satsuma is an offline, cross-platform file converter for Windows and Linux (Tauri v2 + React), inspired by Tangerine (macOS-only). Drag a file onto the app — or select it in a file manager and press Shift — and a citrus-slice-shaped radial menu lets you pick an output format or an advanced tool (compress, crop, trim, split, merge). See [README.md](README.md) for the full concept, target format/tool scope, and interaction model, [docs/phase0.md](docs/phase0.md) for the GUI/interaction-model groundwork (Phase 0, complete), and [docs/phases/phase-2.md](docs/phases/phase-2.md) for the real conversion engine that landed on top of it — Phase 2 is the launch phase and has shipped; advanced per-category tools (Compress, Crop, Trim, etc.) remain out of scope until Phase 3+.

## Versioning

Satsuma follows [Semantic Versioning](https://semver.org/) (`MAJOR.MINOR.PATCH`). The **git tag** (`vX.Y.Z`) is the one source of truth for the shipped version, not any committed file: `.github/workflows/deploy.yml` strips the tag's leading `v` and stamps the bare version into `package.json`, `src-tauri/tauri.conf.json`, and the `[package]` version of `src-tauri/Cargo.toml` and `crates/satsuma-core/Cargo.toml` (via `scripts/set-version.mjs`) right before building the release installers — that stamp happens in CI's ephemeral checkout and is never committed back, so those files' checked-in values are just placeholders for local dev builds and don't need to track the current release. The `skills/release-notes` skill (symlinked into `.claude/skills/`) classifies changes since the last tag against the rules below to infer the next version when one isn't given explicitly, and creates the tag itself — it never edits those version fields.

- **MAJOR** — a breaking change to user-visible behavior that requires user action: a dropped OS/format/tool, a changed file-manager trigger contract (the `--mode=formats|tools <path>` CLI contract, the `LaunchRequest`/`show_overlay`/`hide_overlay` command surface), or a config/theme file format migration that isn't backward compatible. Reserved for after Satsuma reaches a stable 1.0 (see below).
- **MINOR** — new, backward-compatible functionality: a new conversion format or advanced tool, a new wedge-menu option, a new setting. Also covers what would otherwise be a MAJOR breaking change while the major version is still `0` (see below).
- **PATCH** — bug fixes, performance improvements, docs, refactors, dependency bumps, and CI/test-only changes — anything with no user-visible behavior change.

Per the SemVer spec's own [§4](https://semver.org/#spec-item-4), major version zero (`0.y.z`) is for initial development, where the public surface isn't yet considered stable: Satsuma is pre-1.0 through the phased build-out described in [docs/phase0.md](docs/phase0.md) and [docs/phases/](docs/phases/), so a breaking change during this period bumps MINOR rather than MAJOR. The project should cut `1.0.0` once real conversion (Phase 2+) is in place and the file-manager trigger contract is considered stable — from that point on, MAJOR is used as described above.

If a tag needs a prerelease suffix (`vX.Y.Z-<prerelease>`), the `<prerelease>` identifier must be **numeric-only and no greater than 65535** — e.g. `v0.0.1-1`, not `v0.0.1-test` or `v0.0.1-rc.1`. SemVer itself allows alphanumeric prerelease identifiers, but `deploy.yml`'s Windows job bundles an MSI via WiX, and WiX's own version scheme rejects anything else for that field; a non-numeric prerelease builds fine on Linux but fails `build-windows` outright (confirmed by testing `v0.0.1-test`, which failed with `optional pre-release identifier in app version must be numeric-only and cannot be greater than 65535 for msi target`), so `publish` never runs and no release goes out.

## Commands

```bash
npm install                                     # install frontend deps
npm run tauri dev                               # launch the app (Rust + webview)
npm run build                                   # tsc + vite production build

npm run test                                    # frontend: Vitest run (all tests)
npx vitest run src/components/WedgeMenu.test.tsx  # frontend: single test file
npm run test:watch                              # frontend: Vitest watch mode

cargo test                                      # backend: all workspace tests
cargo test -p satsuma-core                      # backend: only the core crate
cargo test -p satsuma-core file_type            # backend: tests matching a name filter

npx tsc --noEmit                                # type-check frontend only
```

Phase 2's conversion engine vendors two native dependencies that aren't assumed to be pre-installed, and their files are **gitignored** — there are no checked-in placeholders, and a fresh checkout has none of them. Tauri validates the `resources` source directory on **every** build (not just when bundling), so any `cargo` invocation that compiles the `satsuma` crate (`cargo build`/`cargo check`/`cargo test --workspace`/`cargo clippy`/`cargo doc`/`cargo tauri dev`, plus the e2e harnesses) fails with `resource path 'lib/ffmpeg' doesn't exist` / `resource path 'lib/pdfium' doesn't exist` until these have been run once per checkout:

```bash
scripts/fetch-ffmpeg.sh    # or fetch-ffmpeg.ps1 on Windows - video/audio conversion (FFmpeg binary + GPL-3 license in Satsuma's own resource directory). Linux: also seeds the windows-gnu stub for the type-check below
scripts/fetch-pdfium.sh    # or fetch-pdfium.ps1 on Windows - PDF -> JPG/PNG rasterization (optional feature; the PDF -> TXT/TXT -> anything paths need no native dependency)
```

`cargo test -p satsuma-core`, `npm run test`, and any other frontend-only command never compile the Tauri crate and need none of this. At **runtime**, a missing/broken vendored binary still degrades gracefully rather than failing conversions outright: FFmpeg falls back to a system-installed `ffmpeg` on `PATH` (dev convenience — production installs always bundle the sidecar), and pdfium falls back to a system-installed `libpdfium.so` in the standard Linux library directories.

To type-check the Windows-only backend module (`src-tauri/src/windows_integration.rs`) without a Windows machine — this only compiles, it does not link, and a full `cargo build`/`cargo tauri build` for this target does not work in a Linux/mingw dev environment (mingw's GNU linker fails on this project with "export ordinal too large"; a real build needs the MSVC toolchain on actual Windows). The build script needs `x86_64-w64-mingw32-windres` (to compile the Windows resource/icon build script) and, as its preprocessor, `x86_64-w64-mingw32-gcc` — without both, the build script fails before reaching the Rust code being type-checked. On Debian/Ubuntu that's `binutils-mingw-w64-x86-64` and `gcc-mingw-w64-x86-64` (note: *not* the confusingly-similarly-named `mingw-w64-tools`, which only provides `widl`/`gendef`/etc., not `windres`). Since Phase 2, `satsuma-core`'s `unrar` dependency (RAR reading — see `crates/satsuma-core/src/convert/archive.rs`) also compiles a bundled C++ source tree via its own build script, which needs `x86_64-w64-mingw32-g++` (Debian/Ubuntu: `g++-mingw-w64-x86-64`) as well — without it, that one crate's build script fails with `failed to find tool "x86_64-w64-mingw32-g++"` before reaching the Rust code being type-checked, same failure mode as the `windres`/`gcc` case above. This doesn't affect the real Windows release build (`windows-latest` CI, MSVC toolchain), only this Linux-side cross-compile type-check path:

```bash
sudo apt-get install -y binutils-mingw-w64-x86-64 gcc-mingw-w64-x86-64 g++-mingw-w64-x86-64
```

The type-check itself also validates the (gitignored) resource paths for the cross target, so run the fetch scripts first if you haven't — `scripts/fetch-ffmpeg.sh` seeds the GPL-3.0-licensed FFmpeg binary and its `COPYING` license text into `lib/ffmpeg/` (no windows-gnu stub exists, the binary is the production release), and `scripts/fetch-pdfium.sh` creates `lib/pdfium/` with its BSD-3-Clause license:

```bash
rustup target add x86_64-pc-windows-gnu
cargo check -p satsuma --target x86_64-pc-windows-gnu
```

Building the Linux target (including `cargo check -p satsuma` and `cargo tauri dev`) needs `libayatana-appindicator3-dev` (or your distro's equivalent) installed for the tray icon — same package CI installs, see `.github/workflows/ci.yml`.

Whenever a dependency in `Cargo.toml`/`package.json` is added, removed, or bumps to a version with a different declared license, refresh [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md)'s two autogenerated appendices (needs network access, for `npx license-checker` and a `--locked` `cargo metadata`):

```bash
node scripts/update-third-party-licenses.mjs
```

This only touches the two `AUTOGENERATED`-marked sections — the curated direct-dependency tables and the unrar/ffmpeg/pdfium writeups above them are hand-maintained and never overwritten by this script.

### Real-desktop integration tests

`cargo test`/`npm run test` are unit-level only (jsdom, no real X11/Windows desktop) and can't exercise the actual OS-integration path: a real cursor query, the overlay window really being positioned/focused, a real click landing on the real rendered `WedgeMenu`, and `stub_convert_file`'s sibling-file write really happening. `tests/e2e-linux/run.sh` and `tests/e2e-windows/run.ps1` do, by launching the built binary directly (`--mode=formats|tools <path>`, the same entry point a Nemo/Nautilus/Dolphin action or a Windows Explorer/Shift trigger produces) and puppeting the real cursor. Both run as required CI jobs (`e2e-linux`, `e2e-windows` in `.github/workflows/ci.yml`).

Both need `npm ci` already run first — `cargo build -p satsuma` (what both harnesses run internally) is a plain debug build with no `custom-protocol` feature (there's no such feature declared anywhere in `src-tauri/Cargo.toml`), so the webview always loads `tauri.conf.json`'s `devUrl` (`http://localhost:1420`), the same as a real `cargo tauri dev` session — `frontendDist`/`dist/` is irrelevant here regardless of whether it's been built. Ordinarily `cargo tauri dev` starts that dev server itself via `beforeDevCommand`; calling `cargo build` directly (bypassing the tauri-cli entirely, as both harnesses deliberately do) skips that, so each harness starts its own vite dev server (`start_vite` in `tests/e2e-linux/lib.sh`, `Start-ViteDevServer` in `tests/e2e-windows/lib.ps1`) before launching satsuma. Without it the webview loads a connection-refused blank page and is permanently inert: the native window still opens, focused and positioned correctly (all Rust-side), but nothing in it is ever interactive — no wedge click, no Escape — which looks exactly like "the overlay opened but never responds to anything" no matter how long a test waits or retries. Both `run.sh`/`run.ps1` check for `node_modules` up front and fail fast with this same explanation if `npm ci` was never run.

```bash
npm ci   # node_modules/ - see above; the harness starts its own vite dev server
sudo apt-get install -y xdotool wmctrl mutter dbus-x11 x11-utils   # Linux: on top of the appindicator dep above
dbus-run-session -- xvfb-run --auto-servernum tests/e2e-linux/run.sh
```

```powershell
# Windows only, from a real interactive session (not a headless/remote one)
npm ci   # node_modules/ - see above; the harness starts its own vite dev server
tests/e2e-windows/run.ps1
tests/e2e-windows/run.ps1 -IncludeExplorerScenario  # also drives a real Explorer window + the Shift-hook itself
```

`windows_integration.rs` itself has already been built and manually verified end-to-end in a real Windows VM (see its own doc comment) — it's this new harness script that's unverified until it's actually run somewhere real; see the CAVEAT at the top of `tests/e2e-windows/lib.ps1` and `run.ps1`.

### When adding a feature, add tests on both tracks

Unit tests (Vitest, `cargo test`) and the real-desktop e2e tests above cover different things — a new feature usually needs both, not one or the other:

- Add/extend unit tests for the feature's own logic: pure functions in `satsuma-core` or `lib/`, component behavior in `src/components/*.test.tsx`, new Tauri commands in `lib.rs`'s own `#[cfg(test)]` module. These are what `cargo test`/`npm run test` already cover — keep using them for anything that doesn't need a real OS/window/cursor.
- If the feature touches the overlay/wedge-menu trigger-to-selection pipeline (a new wedge option, a new mode, a change to `show_overlay`/`hide_overlay`/`LaunchRequest` handling, a new OS-integration entry point) — the part unit tests structurally can't reach — add or extend a scenario in `tests/e2e-linux/run.sh` and `tests/e2e-windows/run.ps1`, not just one of the two: the single-instance-forwarding bug fixed in `lib.rs` only ever showed up on Windows (WebView2's lazy content-loading), so parity between the two harnesses is what actually catches platform-specific regressions like that one.
- A change that's purely backend logic with no OS/window/cursor surface (e.g. a new `satsuma-core` helper) only needs the unit-test track — don't add e2e scenarios for things that don't touch a real window.

## Architecture

**Cargo workspace** (`Cargo.toml` at repo root) with two members, split so the conversion/detection engine stays independent of — and testable without — the Tauri command layer:

- `crates/satsuma-core` — pure Rust, no Tauri dependency. `file_type.rs` detects image/video/audio category from a file extension; `launch_request.rs` parses `--mode=formats|tools <path>...` from process args; `overlay_geometry.rs` has pure screen-rect/cursor placement math for the borderless overlay window (kept OS-agnostic so it's testable without a Windows toolchain).
- `src-tauri` — the Tauri app and command layer (`src/lib.rs`), plus two platform-specific, `cfg`-gated modules that are NOT part of `satsuma-core` because they talk to OS-level APIs rather than doing conversion logic:
  - `src/windows_integration.rs` (Windows only): a global low-level keyboard hook watches for Shift while Explorer is foreground, queries the current selection via `Shell.Application` COM automation, and calls `show_overlay` (see below). Also has `cursor_position()` via `GetCursorPos`. Verified via `windows-latest` CI plus manual testing in a Windows VM — see [docs/phase0.md](docs/phase0.md). Cross-compiling without a Windows machine (see the Commands section above) type-checks but does not link.
  - `src/linux_integration.rs` (Linux only): installs/removes the Linux file-manager context-menu integration (Nautilus scripts, Nemo actions, KDE Dolphin service menu, `.desktop` MIME association) under the user's XDG data dirs. The actual files it installs live in `src-tauri/resources/linux/`. Also has `cursor_position()` via an X11 `XQueryPointer` query (`None` on Wayland).
  - Both platform paths converge on the same `LaunchRequest` (`{ mode, paths }`), delivered to the frontend via a `take_launch_request` command (claimed once on startup) and a `launch-request` event (for a second file-manager invocation while the app is already running, forwarded through `tauri-plugin-single-instance`). Either way, `lib.rs`'s `show_overlay` positions the borderless `"overlay"` window at the cursor (or monitor-center, if unavailable) and shows it — it does not touch the normal `"main"` window.
  - `lib.rs` also builds the tray icon (Show Satsuma / Start at Login via `tauri-plugin-autostart` / Quit) and intercepts the main window's close button to hide instead of quit, so the process — and the Windows hook — stay alive in the background. See `docs/tray-resident-overlay-design.md` for the full design rationale.

**Frontend** (`src/`, React + TypeScript + Vite): two window-specific entry components, picked in `main.tsx` by a `?window=overlay` URL flag (dynamic `import()`, so each window's bundle only pulls in its own CSS). `App.tsx` is the normal titled window: drag-and-drop with Shift/Alt modifier keys (`hooks/useFileDrop.ts`, `hooks/useModifierKeys.ts`), tracked via a `menuTrigger: "modifier" | null` state so Alt can live-toggle the menu mode during a drag-and-drop session (a real bug found and fixed during Phase 0 — see `docs/phase0.md`). `OverlayApp.tsx` is the borderless overlay window: just `WedgeMenu` on a transparent background, driven entirely by `hooks/useLaunchRequest.ts`, calling the `hide_overlay` command on select/cancel. The wedge menu itself (`components/WedgeMenu.tsx`) is an SVG radial menu whose geometry (`lib/wedgeGeometry.ts`) and options (`data/wedgeOptions.ts`, backend-queried as of Phase 2 via the `list_conversion_targets` Tauri command) are both plain, independently-tested modules shared by both windows. Drag-and-drop uses Tauri's native `onDragDropEvent` (not HTML5 drag events) to get real file paths on both Windows and Linux.

Formats/tools shown in the wedge menu are real, backend-driven data as of Phase 2 (`crates/satsuma-core/src/convert/`, dispatched through `list_conversion_targets`/`convert_file`) — FFmpeg-, `image`-crate-, document-library-, and archive-library-backed conversion across the full format matrix in [docs/tools.md](docs/tools.md). Advanced per-category tools (Compress, Crop, Trim, etc.) remain out of scope until Phase 3+.
