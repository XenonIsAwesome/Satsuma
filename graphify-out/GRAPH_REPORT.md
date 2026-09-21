# Graph Report - satsuma-pre-launch-audit-ce7264  (2026-09-21)

## Corpus Check
- 122 files · ~130,601 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 1413 nodes · 3056 edges · 106 communities (78 shown, 28 thin omitted)
- Extraction: 97% EXTRACTED · 3% INFERRED · 0% AMBIGUOUS · INFERRED: 89 edges (avg confidence: 0.82)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `ec6495f9`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- src-tauri/src/lib.rs
- modifiers.ts
- linux_integration.rs
- devDependencies
- theme_store.rs
- compilerOptions
- HotkeyRecorder.tsx
- bundle
- windows_integration.rs
- dependencies
- scripts
- Format & tool matrix
- default.json
- compilerOptions
- Satsuma File Converter
- color.ts
- Satsuma Application
- Satsuma App
- Satsuma
- Satsuma
- settings.rs
- Convert with Satsuma
- Satsuma Tools
- LinuxIntegrationPanel Component
- document.rs
- archive.rs
- Satsuma App Icon (128x128 resolution)
- Satsuma Windows Store Tile Logo (Turquoise-Yellow Yin-Yang Design)
- Satsuma Store Tile Logo
- Satsuma App Logo (Windows Store 89x89 Tile)
- wedgeOptions.ts
- Two-agent PR review and fixing loop
- settings_store.rs
- Visual identity — "Citrus"
- audio.rs
- App.tsx
- CLAUDE.md
- main.tsx
- watch
- eslint-plugin-react-hooks
- package.json
- vitest
- video.rs
- @types/react
- image.rs
- vite
- Design
- useSettings.ts
- types.ts
- tauri.ts
- launch_request.rs
- WedgeMenu.tsx
- lib.ps1
- lib.sh
- typedoc.json
- naming.rs
- functionality-phases.md
- Satsuma vX.Y.Z
- ignoreDependencies
- ffmpeg.rs
- generate-serve-json.mjs
- run.sh
- globals
- Interaction model
- Phase 1 — Settings screen & hotkey remapping
- Phase 3 — advanced tools, wave 1
- Phase 4 — advanced tools, wave 2
- install.sh script
- Phase 2 — core conversion engine
- Phase 5 — advanced tools, wave 3
- set-version.mjs
- Release notes generation
- docs-coverage-badge.mjs
- generate-badge.mjs
- wedge_point.py
- pdfium.rs
- convert
- jsdom
- @tauri-apps/cli
- @testing-library/react
- typedoc
- encode_wide
- @vitest/coverage-v8
- fetch-ffmpeg.sh script
- fetch-pdfium.sh script
- file_type.rs
- mod.rs
- smoke-pdfium.sh script
- overlay_geometry.rs
- Third-party licenses
- Linux desktop coverage hardening
- typescript
- x11_window_id
- AutostartMenuItem

## God Nodes (most connected - your core abstractions)
1. `ConvertError` - 55 edges
2. `convert()` - 35 edges
3. `write()` - 34 edges
4. `convert()` - 33 edges
5. `write()` - 21 edges
6. `extract()` - 19 edges
7. `install()` - 17 edges
8. `compilerOptions` - 17 edges
9. `Entry` - 16 edges
10. `find_ffmpeg()` - 16 edges

## Surprising Connections (you probably didn't know these)
- `vitest` --semantically_similar_to--> `React Testing Library`  [INFERRED] [semantically similar]
  package.json → docs/phase0.md
- `detect_file_category()` --calls--> `detect_category()`  [INFERRED]
  src-tauri/src/lib.rs → crates/satsuma-core/src/file_type.rs
- `handle_trigger()` --calls--> `detect_category()`  [INFERRED]
  src-tauri/src/windows_integration.rs → crates/satsuma-core/src/file_type.rs
- `run()` --calls--> `parse_launch_args()`  [INFERRED]
  src-tauri/src/lib.rs → crates/satsuma-core/src/launch_request.rs
- `mode_flag_and_paths_round_trip_into_a_launch_request()` --calls--> `parse_launch_args()`  [INFERRED]
  src-tauri/src/windows_integration.rs → crates/satsuma-core/src/launch_request.rs

## Import Cycles
- None detected.

## Hyperedges (group relationships)
- **Frontend Interaction System** — use_file_drop, use_modifier_keys, use_launch_request, keyboard_navigation [INFERRED 0.85]
- **Phase 0 File Manager Integration** — windows_explorer_hook, linux_context_menu, launch_request_data [INFERRED 0.85]
- **UI Component Suite** — drop_zone_tsx, wedge_menu_tsx, linux_integration_panel [INFERRED 0.85]

## Communities (106 total, 28 thin omitted)

### Community 0 - "src-tauri/src/lib.rs"
Cohesion: 0.20
Nodes (20): config_base_dir(), ConversionProgress, extract_archive(), extract_archive_blocking(), extract_archive_rejects_a_missing_source(), install_linux_file_manager_integration(), open_themes_dir(), CustomColors (+12 more)

### Community 1 - "modifiers.ts"
Cohesion: 0.18
Nodes (16): NONE_HELD, useHeldModifiers(), ModifierMatchState, useModifierKeys(), comboEquals(), comboLabel(), DEFAULT_FORMAT_MODIFIERS, DEFAULT_TOOLS_MODIFIERS (+8 more)

### Community 2 - "linux_integration.rs"
Cohesion: 0.10
Nodes (37): Connection, current_server_time(), cursor_position(), cursor_position_is_none_on_a_wayland_session(), default_paths(), default_paths_prefers_xdg_data_home(), desktop_quote(), force_activate_window() (+29 more)

### Community 3 - "devDependencies"
Cohesion: 0.10
Nodes (21): eslint, @eslint/js, eslint-plugin-react-refresh, devDependencies, eslint, @eslint/js, eslint-plugin-react-refresh, knip (+13 more)

### Community 4 - "theme_store.rs"
Cohesion: 0.11
Nodes (36): builtin_theme_ids_are_already_valid_slugs(), builtin_themes(), builtin_themes_have_unique_ids(), CustomColors, String, Vec, slugify(), ThemeFile (+28 more)

### Community 5 - "compilerOptions"
Cohesion: 0.08
Nodes (24): DOM, ES2020, @testing-library/jest-dom, vitest/globals, compilerOptions, allowImportingTsExtensions, isolatedModules, jsx (+16 more)

### Community 6 - "HotkeyRecorder.tsx"
Cohesion: 0.13
Nodes (10): HotkeyRecorder(), HotkeyRecorderProps, NONE_HELD, { invokeMock }, NOTHING_HELD, SHIFT, isSupportedKey(), KEY_LABELS (+2 more)

### Community 7 - "bundle"
Cohesion: 0.04
Nodes (44): icons/128x128@2x.png, icons/128x128.png, icons/32x32.png, icons/icon.icns, icons/icon.ico, app, security, windows (+36 more)

### Community 8 - "windows_integration.rs"
Cohesion: 0.08
Nodes (31): detect_file_category Command, file_type.rs, LaunchRequest Data Structure, launch_request.rs, Linux Context Menu Integration, linux_integration.rs, LPARAM, LRESULT (+23 more)

### Community 9 - "dependencies"
Cohesion: 0.13
Nodes (15): @fontsource/nunito, @fontsource/plus-jakarta-sans, dependencies, @fontsource/nunito, @fontsource/plus-jakarta-sans, react, react-dom, @tauri-apps/api (+7 more)

### Community 10 - "scripts"
Cohesion: 0.18
Nodes (11): scripts, build, dev, docs, knip, lint, preview, tauri (+3 more)

### Community 11 - "Format & tool matrix"
Cohesion: 0.17
Nodes (12): Any file, Archives, Audio, Directories and unsupported inputs, Documents (PDF), Engineering conventions worth following, Format & tool matrix, GIF (+4 more)

### Community 12 - "default.json"
Cohesion: 0.18
Nodes (10): core:default, dialog:allow-open, main, opener:default, overlay, description, identifier, permissions (+2 more)

### Community 13 - "compilerOptions"
Cohesion: 0.22
Nodes (8): vite.config.ts, compilerOptions, allowSyntheticDefaultImports, composite, module, moduleResolution, skipLibCheck, include

### Community 14 - "Satsuma File Converter"
Cohesion: 0.25
Nodes (8): Satsuma File Converter, Satsuma App Icon 32x32, Satsuma App Icon, Satsuma Windows Store Tile Logo, Satsuma Windows Store Tile Logo, Satsuma Windows Store Tile Logo, Tauri Desktop Framework, Yin-Yang Visual Design

### Community 15 - "color.ts"
Cohesion: 0.25
Nodes (19): adjust(), clamp(), contrastOn(), contrastRatio(), deriveOutline(), derivePrimaryActive(), derivePrimaryStrong(), hexToRgb() (+11 more)

### Community 16 - "Satsuma Application"
Cohesion: 0.40
Nodes (5): File Converter, Satsuma Store Logo, Satsuma Application, Tauri Framework, Windows Store

### Community 17 - "Satsuma App"
Cohesion: 0.50
Nodes (4): File Converter, Satsuma App, Satsuma App Icon (Retina 2x), Square44x44 Windows Store Tile Logo

### Community 18 - "Satsuma"
Cohesion: 0.50
Nodes (4): File Converter, Satsuma, Tauri Framework, Windows Store Tile

### Community 20 - "settings.rs"
Cohesion: 0.10
Nodes (24): is_supported_key(), overlay_position(), Option, ScreenRect, a_lone_key_with_no_modifiers_is_not_empty(), allows_a_binding_that_is_a_superset_of_the_other(), CustomColors, defaults_match_phase_0s_previously_hardcoded_shift_and_shift_alt() (+16 more)

### Community 26 - "document.rs"
Cohesion: 0.09
Nodes (81): assert_well_formed_xml(), Block, build_text_pdf_fixture(), convert(), convert_from_image(), convert_from_pdf(), convert_from_subtitle(), convert_from_txt() (+73 more)

### Community 27 - "archive.rs"
Cohesion: 0.07
Nodes (104): account_uncompressed(), account_uncompressed_rejects_once_the_total_passes_the_archive_wide_budget(), assert_7z_accepts(), assert_rar_contains_fixture(), assert_tar_contains_fixture(), assert_unrar_accepts(), assert_zip_contains_fixture(), bare_gz_converts_to_zip_with_just_that_one_file() (+96 more)

### Community 36 - "Two-agent PR review and fixing loop"
Cohesion: 0.20
Nodes (9): Definition of done, Fixer protocol, Orchestrator protocol, Project knobs, Reviewer protocol, Setup, Termination contract, The loop contract (order of operations) (+1 more)

### Community 37 - "settings_store.rs"
Cohesion: 0.20
Nodes (17): fs_read_to_string(), load_returns_defaults_for_unparseable_content(), load_returns_defaults_when_no_file_exists_yet(), load_settings_from(), Drop, Option, Path, PathBuf (+9 more)

### Community 38 - "Visual identity — "Citrus""
Cohesion: 0.12
Nodes (16): Buttons & tabs, Colors, Components, Custom theme behavior, Design tokens, Do's and Don'ts, Drop zone, Elevation & Depth (+8 more)

### Community 39 - "audio.rs"
Cohesion: 0.12
Nodes (39): Child, canonicalize(), codec_args(), convert(), convert_rejects_missing_source(), convert_rejects_unsupported_target(), ffmpeg_is_runnable(), ffmpeg_test_lock() (+31 more)

### Community 40 - "App.tsx"
Cohesion: 0.20
Nodes (17): App(), Toast, EXTENSION_DISPLAY, formatWedgeOptions(), getToolOptions(), TOOL_OPTIONS, useLaunchRequest(), comboContains() (+9 more)

### Community 41 - "CLAUDE.md"
Cohesion: 0.06
Nodes (31): AI disclosure, Architecture, Commands, graphify, Project, Real-desktop integration tests, Versioning, When adding a feature, add tests on both tracks (+23 more)

### Community 43 - "watch"
Cohesion: 0.50
Nodes (4): Fn, Send, PathBuf, watch()

### Community 45 - "package.json"
Cohesion: 0.40
Nodes (4): name, private, type, version

### Community 46 - "vitest"
Cohesion: 0.67
Nodes (3): vitest, React Testing Library, vitest

### Community 47 - "video.rs"
Cohesion: 0.13
Nodes (38): BufReader, convert(), convert_rejects_a_target_outside_the_format_matrix(), converts_mp4_to_gif(), converts_mp4_to_mp3_audio_export(), converts_mp4_to_webm(), converts_silent_gif_to_mp4(), ffmpeg_args() (+30 more)

### Community 49 - "image.rs"
Cohesion: 0.14
Nodes (26): assert_quadrants(), avif_target_produces_a_valid_avif_file(), convert(), convert_defensively_rejects_pdf_and_docx_targets(), convert_rejects_a_corrupt_source_file(), convert_rejects_a_missing_source_file(), convert_rejects_a_target_this_module_does_not_support(), heic_and_heif_list_the_full_raster_set_but_never_convert() (+18 more)

### Community 51 - "Design"
Cohesion: 0.18
Nodes (10): 1. Process lifecycle: tray-resident + autostart, 2. Borderless overlay window, 3. Positioning, 4. Trigger-path wiring, 5. Testing & verification, Design, Goals, Non-goals (explicitly deferred) (+2 more)

### Community 52 - "useSettings.ts"
Cohesion: 0.12
Nodes (18): { onDragDropEventMock, invokeMock, listenMock }, SettingsScreenProps, CITRUS, { invokeMock, openFileDialogMock }, THEMES, YUZU, DEFAULT_SETTINGS, CUSTOM_SETTINGS (+10 more)

### Community 53 - "types.ts"
Cohesion: 0.29
Nodes (9): DropZone(), DropZoneProps, { onDragDropEventMock, invokeMock }, useFileDrop(), RawLaunchRequest, detectFileCategory(), DroppedFile, FileCategory (+1 more)

### Community 54 - "tauri.ts"
Cohesion: 0.19
Nodes (17): LinuxIntegrationPanel(), { invokeMock }, CUSTOM_COLOR_FIELDS, SettingsScreen(), currentPlatform(), ffmpegStatus(), installLinuxFileManagerIntegration(), isAutostartEnabled() (+9 more)

### Community 55 - "launch_request.rs"
Cohesion: 0.12
Nodes (15): category_is_computed_from_the_first_path(), collects_multiple_paths_in_order(), LaunchRequest, MenuMode, parse_launch_args(), FileCategory, Option, String (+7 more)

### Community 56 - "WedgeMenu.tsx"
Cohesion: 0.33
Nodes (9): OPTIONS, useShouldShowToolsHint(), WedgeMenu(), WedgeMenuProps, describeWedge(), Point, polarToCartesian(), wedgeAngles() (+1 more)

### Community 57 - "lib.ps1"
Cohesion: 0.20
Nodes (31): Get-OverlayWindowCenter(), Get-OverlayWindowHandle(), Get-VisibleWindowHandlesForProcess(), Get-WedgePoint(), Get-WindowHandlesForProcess(), Invoke-LeftClick(), New-ScratchDir(), Press-ShiftDown() (+23 more)

### Community 58 - "lib.sh"
Cohesion: 0.14
Nodes (22): cleanup_all(), click_wedge_option(), click_wedge_until_hidden(), diag_input_state(), dump_satsuma_log(), dump_visible_windows(), fail(), find_overlay_window() (+14 more)

### Community 59 - "typedoc.json"
Cohesion: 0.10
Nodes (20): src/test/**, src/**/*.test.ts, src/**/*.test.tsx, customCss, customJs, entryPoints, entryPointStrategy, exclude (+12 more)

### Community 60 - "naming.rs"
Cohesion: 0.07
Nodes (35): Command, build_ffmpeg_command(), convert(), convert_rejects_a_missing_source_file(), convert_rejects_a_target_not_in_the_source_format_matrix(), family_targets(), FnMut, Option (+27 more)

### Community 62 - "Satsuma vX.Y.Z"
Cohesion: 0.25
Nodes (7): Artifacts, Breaking Changes, Features, Fixes, Highlights, Known Issues, Satsuma vX.Y.Z

### Community 63 - "ignoreDependencies"
Cohesion: 0.25
Nodes (7): ignoreDependencies, project, $schema, @fontsource/nunito, @fontsource/plus-jakarta-sans, src/**/*.{ts,tsx}, @tauri-apps/plugin-opener

### Community 64 - "ffmpeg.rs"
Cohesion: 0.08
Nodes (32): App.tsx, Drag-and-Drop Interaction, DropZone.tsx Component, FFmpeg, File Converter, File Manager Integration, Keyboard Navigation, Offline Conversion (+24 more)

### Community 65 - "generate-serve-json.mjs"
Cohesion: 0.29
Nodes (5): indexDirs, rewrites, rustDir, serveJson, [siteDir]

### Community 66 - "run.sh"
Cohesion: 0.52
Nodes (6): main(), scenario_escape_cancels_without_side_effect(), scenario_formats_select_writes_sibling_file(), scenario_single_instance_forwarding(), scenario_tools_mode_opens_and_dismisses(), run.sh script

### Community 68 - "Interaction model"
Cohesion: 0.40
Nodes (5): Fallbacks, Interaction model, Menu content rules, Modifier keys, Progress & completion feedback

### Community 69 - "Phase 1 — Settings screen & hotkey remapping"
Cohesion: 0.40
Nodes (5): Explicitly not in this phase, Phase 1 — Settings screen & hotkey remapping, Scope, Success criteria, Why this phase, and why here

### Community 70 - "Phase 3 — advanced tools, wave 1"
Cohesion: 0.40
Nodes (5): Design notes to carry through implementation (see [tools.md](../tools.md) for full detail), Explicitly not in this phase, Phase 3 — advanced tools, wave 1, Scope, Success criteria

### Community 71 - "Phase 4 — advanced tools, wave 2"
Cohesion: 0.40
Nodes (5): Design notes to carry through implementation (see [tools.md](../tools.md) for full detail), Explicitly not in this phase, Phase 4 — advanced tools, wave 2, Scope, Success criteria

### Community 72 - "install.sh script"
Cohesion: 0.80
Nodes (4): log(), need_cmd(), install.sh script, die()

### Community 73 - "Phase 2 — core conversion engine"
Cohesion: 0.50
Nodes (4): Explicitly not in this phase, Phase 2 — core conversion engine, Scope, Success criteria

### Community 74 - "Phase 5 — advanced tools, wave 3"
Cohesion: 0.40
Nodes (5): Design notes to carry through implementation (see [tools.md](../tools.md) for full detail), Explicitly not in this phase, Phase 5 — advanced tools, wave 3, Scope, Success criteria

### Community 76 - "Release notes generation"
Cohesion: 0.50
Nodes (3): Release notes generation, Steps, Usage

### Community 81 - "pdfium.rs"
Cohesion: 0.14
Nodes (9): code_for_windows_vk(), code_for_x11_keycode(), Option, windows_vk_for_code(), windows_vks(), x11_keycode_for_code(), x11_keycodes(), Item (+1 more)

### Community 82 - "convert"
Cohesion: 0.14
Nodes (6): detect_file_category(), list_conversion_targets(), list_conversion_targets_excludes_the_sources_own_format(), list_conversion_targets_intersects_a_mixed_selection(), list_conversion_targets_is_empty_for_unrelated_families(), FileCategory

### Community 87 - "encode_wide"
Cohesion: 0.67
Nodes (3): encode_wide(), enforce(), Vec

### Community 91 - "file_type.rs"
Cohesion: 0.23
Nodes (9): first_runnable(), is_runnable_ffmpeg(), resolve_ffmpeg_bin(), resolve_override(), AppHandle, Option, Path, PathBuf (+1 more)

### Community 96 - "mod.rs"
Cohesion: 0.23
Nodes (16): get_settings(), hide_overlay(), is_autostart_enabled(), list_themes(), load_settings_and_themes(), primary_monitor_center(), reconcile_themes_and_notify(), AppHandle (+8 more)

### Community 99 - "overlay_geometry.rs"
Cohesion: 0.18
Nodes (15): convert_file(), convert_file_blocking(), convert_file_rejects_a_missing_source(), convert_file_writes_a_real_output_and_reports_progress(), display_path(), ffmpeg_status(), load_settings_only(), pdfium_status() (+7 more)

### Community 100 - "Third-party licenses"
Cohesion: 0.19
Nodes (10): content, escapeRegExp(), licensesPath, npmDependencyGroups(), npmGroups, replaceBetweenMarkers(), repoRoot, run() (+2 more)

### Community 102 - "Linux desktop coverage hardening"
Cohesion: 0.50
Nodes (4): Linux desktop coverage hardening, Scope, Success criteria, Why this is lower priority

### Community 104 - "x11_window_id"
Cohesion: 0.83
Nodes (4): force_focus_overlay_linux(), R, x11_window_id(), WebviewWindow

### Community 105 - "AutostartMenuItem"
Cohesion: 0.67
Nodes (3): CheckMenuItem, AutostartMenuItem, Wry

## Knowledge Gaps
- **311 isolated node(s):** `$schema`, `src/**/*.{ts,tsx}`, `@fontsource/nunito`, `@fontsource/plus-jakarta-sans`, `@tauri-apps/plugin-opener` (+306 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **28 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `ConvertError` connect `archive.rs` to `audio.rs`, `video.rs`, `image.rs`, `document.rs`, `naming.rs`?**
  _High betweenness centrality (0.194) - this node is a cross-community bridge._
- **Why does `Rgb` connect `document.rs` to `color.ts`?**
  _High betweenness centrality (0.083) - this node is a cross-community bridge._
- **Why does `Tauri` connect `ffmpeg.rs` to `windows_integration.rs`, `convert`, `file_type.rs`?**
  _High betweenness centrality (0.045) - this node is a cross-community bridge._
- **What connects `$schema`, `src/**/*.{ts,tsx}`, `@fontsource/nunito` to the rest of the system?**
  _311 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `linux_integration.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.10040816326530612 - nodes in this community are weakly interconnected._
- **Should `devDependencies` be split into smaller, more focused modules?**
  _Cohesion score 0.09523809523809523 - nodes in this community are weakly interconnected._
- **Should `theme_store.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.1109936575052854 - nodes in this community are weakly interconnected._