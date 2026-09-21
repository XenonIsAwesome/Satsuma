# Tray-resident background app + borderless overlay window

Status: implemented — see [phase0.md](phase0.md) for current status, the bugs hardening it surfaced, and what's still unverified on real hardware.

## Problem

Two issues reported after testing on real Linux Mint and Windows 10:

1. **Windows**: the global Shift hotkey (with a file selected in Explorer) does nothing unless Satsuma's window is already open. The user has to manually launch the app first, defeating the point of the hotkey.
2. **Both platforms**: the wedge menu opens *inside* Satsuma's normal bordered app window, which has to already be visible/focused. The expectation (per [interaction.md](interaction.md)'s borderless-overlay path) was a popup that appears at/near the selected file, with no full app window involved.

Both trace back to the same gap: no tray/background-resident mode, and no overlay-at-the-cursor visual — the Windows hook only does anything while the process happens to be running, and both trigger paths brought the *main* window forward instead of drawing a separate overlay.

## Goals

- The Windows Shift hotkey and the Linux file-manager context-menu trigger both work without the user having manually opened Satsuma first.
- Both triggers open a borderless, transparent overlay window positioned near the user's cursor, instead of the normal titled app window.
- The existing drag-and-drop-onto-a-window flow keeps working, unchanged, as an optional window reachable from a new tray icon.
- Scope stays realistic about what's verifiable without real hardware vs. what needs real-device testing.

## Non-goals (explicitly deferred)

- **Pixel-accurate positioning on the selected file's icon** (via COM/D-Bus icon-rect queries). V1 positions on the current cursor location instead — see [phase0.md](phase0.md)'s known gaps.
- **Wayland global-cursor-position support.** Most Wayland compositors don't expose this to arbitrary clients for security reasons; the overlay falls back to centering on the primary monitor there. X11 sessions (including the Linux Mint/Cinnamon setup actually tested) get real cursor-based positioning.
- **macOS** (out of scope for Satsuma entirely, per the [README](../README.md)).
- **Any change to actual conversion logic** — this is purely about window/process presentation.

## Design

### 1. Process lifecycle: tray-resident + autostart

A system tray icon (Tauri v2's built-in `tray-icon` support, no separate plugin) with a menu:

- **Show Satsuma** — opens/focuses the normal drag-and-drop window.
- **Start at login** — checkbox, backed by the official `tauri-plugin-autostart` plugin (Windows: registry Run key; Linux: XDG autostart `.desktop` entry).
- **Quit** — the only way to fully exit the process.

The main window's close button ("X") is intercepted (`WindowEvent::CloseRequested`) to hide the window instead of exiting the app. This makes the process tray-resident: once started (manually or via autostart), it stays alive in the background, which is what keeps the Windows keyboard hook installed and able to react to Shift at any time.

New build dependency: Linux tray icons need `libayatana-appindicator3-dev` (or equivalent) at build time.

### 2. Borderless overlay window

A second Tauri window, `"overlay"`, created once at startup (hidden) rather than per-trigger, to avoid window-creation flicker/latency on each invocation:

- Undecorated, transparent, always-on-top, skip-taskbar, no shadow, not resizable.
- Sized to just fit the wedge menu.
- Loads the same frontend bundle as the main window, but with a URL flag that mounts a small `OverlayApp` component instead of the full app — rendering just the wedge menu on a transparent background. The wedge menu's existing Escape/click-outside-cancel behavior doubles as the overlay's dismiss mechanism.
- On wedge selection or cancel, the overlay window is hidden (not destroyed), ready for the next trigger.
- If the overlay window loses OS focus while open, it auto-hides — matches popup-menu expectations (click elsewhere to dismiss). (This specific mechanism was later found unreliable on Linux/Mutter and replaced — see [phase0.md](phase0.md)'s hardening notes.)

The normal window's drag-and-drop flow is untouched — it's just no longer the thing that appears for hotkey/context-menu triggers.

### 3. Positioning

Centers on the current cursor position, with a monitor-center fallback when no cursor position is available:

- **Windows**: queried via the Win32 cursor-position API at the moment Shift is detected.
- **Linux**: an X11 pointer-position query at the moment the context-menu-triggered process starts (or the single-instance event arrives), skipped on Wayland (detected via `XDG_SESSION_TYPE`) per the non-goal above.

### 4. Trigger-path wiring

- **Windows**: the Shift-hotkey handler stops calling show/focus on the main window. Instead it computes the cursor position, positions the overlay window there, emits the launch request as today, and shows+focuses the overlay window.
- **Linux**: the same redirection for both the fresh-process-argv and single-instance-forwarded launch-request paths. The context-menu scripts/service-menu files themselves don't change; only what the running app does with the resulting launch request changes.
- The existing single-instance forwarding (a second context-menu click while Satsuma is already running) also targets the overlay window instead of the main window.

### 5. Testing & verification

- Pure-logic Rust code (cursor-position fallback, any new argument/state handling) gets unit tests.
- Frontend: the overlay component gets the same kind of integration coverage the main app has — renders the wedge menu directly from a launch request, selection/cancel behavior — using the existing mocked Tauri API pattern.
- What can't be verified without real hardware: the tray icon's actual rendering, the autostart registry/XDG entries actually taking effect end-to-end, the Windows hook's cursor-position + overlay-show behavior, and X11 pointer querying. See [phase0.md](phase0.md) for where each of these actually landed.
