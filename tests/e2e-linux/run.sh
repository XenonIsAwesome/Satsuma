#!/usr/bin/env bash
# Linux virtual-desktop integration tests for Satsuma.
#
# Boots a real (virtual) X11 desktop, launches the actual compiled
# `satsuma` binary the same way a Nemo/Nautilus/Dolphin context-menu action
# does (`--mode=formats|tools <path>`, see
# crates/satsuma-core/src/launch_request.rs), puppets the real mouse
# cursor via xdotool to pick a wedge option, and asserts on real,
# observable side effects: the overlay window actually appearing/hiding,
# and convert_file's real sibling-file write — a genuine re-encode as of
# Phase 2, not Phase 0's byte-copy stub — actually happening on disk. No
# test-only hooks in the app itself — see the doc comment in lib.sh for
# how the click coordinates are derived without a window-geometry query.
#
# Usage: dbus-run-session -- xvfb-run --auto-servernum tests/e2e-linux/run.sh
# (xvfb-run provides the X server / $DISPLAY; this script starts the
# window manager within it. dbus-run-session provides a real D-Bus session
# bus - without one, the tray icon's setup can fail and take the whole app
# down with it before it ever shows the overlay, since setup_tray()'s `?`
# in src-tauri/src/lib.rs's .setup() closure short-circuits everything
# after it, including the initial-launch show_overlay call.)
#
# Requires: xdotool, wmctrl, xwininfo (x11-utils), mutter (or openbox as a
# fallback), `npm ci` already run (this script starts its own vite dev
# server - see start_vite in lib.sh for why a plain `cargo build` needs
# one at all), and a built `satsuma` binary at target/debug/satsuma (built
# here unless SATSUMA_SKIP_BUILD=1 is set).

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "$SCRIPT_DIR/lib.sh"

# --- scenarios -----------------------------------------------------------
#
# Each scenario is a plain function in this same shell (no subshells): the
# `wait_until` calls in lib.sh already bound how long any single wait can
# take, so a stuck focus/activation regression surfaces as a prompt,
# specific failure rather than an indefinite hang. `trap kill_satsuma
# RETURN` at the top of each ensures that scenario's satsuma process is
# always killed when the function returns, success or failure, without
# touching the window manager shared across scenarios.

# Image formats: the real backend list for a .png source, per
# crates/satsuma-core/src/convert/image.rs's supported_targets("png") ->
# [jpg, webp, tiff, avif, bmp, pdf, docx] (7 entries; png is already
# excluded from its own target list, and every one of those has a
# EXTENSION_DISPLAY entry in src/data/wedgeOptions.ts so all 7 render).
IMAGE_FORMATS_MINUS_PNG_COUNT=7
JPG_INDEX=0 # jpg is still first in that list

# Image tools, from TOOL_OPTIONS in the same file: [compress, crop].
IMAGE_TOOLS_COUNT=2
COMPRESS_INDEX=0

scenario_formats_select_writes_sibling_file() {
  trap kill_satsuma RETURN
  local dir; dir="$(make_scratch_dir photo.png)"
  local src="$dir/photo.png"
  local expected_dest="$dir/photo.jpg"

  place_cursor
  launch_satsuma formats "$src"

  local winid
  # 30s (not the 10s the later scenarios use): this is the very first app
  # launch on a cold runner - vite's warm-up is handled in start_vite, but
  # the webview/OS side of a first launch (browser-process init, cold page
  # navigation) is still slower than subsequent launches, and a first real
  # CI run flaked right here on Windows (overlay never appeared within 10s,
  # same sources, next scenarios fine).
  if ! winid="$(wait_for_overlay_window 30)"; then
    fail "formats-select: overlay window never appeared"
    rm -rf "$dir"
    return 1
  fi
  focus_window "$winid"

  if ! click_wedge_until_hidden "$winid" "$JPG_INDEX" "$IMAGE_FORMATS_MINUS_PNG_COUNT" 5; then
    fail "formats-select: overlay did not hide after clicking a wedge"
  fi

  if ! wait_until 5 "converted sibling file appears" test -f "$expected_dest"; then
    fail "formats-select: $expected_dest was never written"
  elif ! is_valid_jpeg "$expected_dest"; then
    fail "formats-select: $expected_dest was written but isn't a valid JPEG"
  else
    log "PASS: formats-select wrote a real re-encoded JPEG at $expected_dest"
  fi

  rm -rf "$dir"
}

scenario_escape_cancels_without_side_effect() {
  trap kill_satsuma RETURN
  local dir; dir="$(make_scratch_dir photo.png)"
  local src="$dir/photo.png"
  local unexpected_dest="$dir/photo.jpg"

  place_cursor
  launch_satsuma formats "$src"

  local winid
  if ! winid="$(wait_for_overlay_window 10)"; then
    fail "escape-cancel: overlay window never appeared"
    rm -rf "$dir"
    return 1
  fi
  focus_window "$winid"

  if ! send_escape_until_hidden 5; then
    fail "escape-cancel: overlay did not hide after Escape"
  fi

  if [ -f "$unexpected_dest" ]; then
    fail "escape-cancel: $unexpected_dest was written despite cancelling"
  else
    log "PASS: escape-cancel wrote no file"
  fi

  rm -rf "$dir"
}

scenario_tools_mode_opens_and_dismisses() {
  trap kill_satsuma RETURN
  local dir; dir="$(make_scratch_dir photo.png)"
  local src="$dir/photo.png"

  place_cursor
  launch_satsuma tools "$src"

  local winid
  if ! winid="$(wait_for_overlay_window 10)"; then
    fail "tools-mode: overlay window never appeared"
    rm -rf "$dir"
    return 1
  fi
  focus_window "$winid"

  if ! click_wedge_until_hidden "$winid" "$COMPRESS_INDEX" "$IMAGE_TOOLS_COUNT" 5; then
    fail "tools-mode: overlay did not hide after clicking a tools wedge"
  else
    log "PASS: tools-mode opened and dismissed cleanly"
  fi

  rm -rf "$dir"
}

scenario_single_instance_forwarding() {
  trap kill_satsuma RETURN
  local dir; dir="$(make_scratch_dir photo.png)"
  local src="$dir/photo.png"
  local expected_dest="$dir/photo.jpg"

  # First launch: no path, stays resident (the tray-resident design) with
  # no overlay to show yet. Set SATSUMA_PID/SATSUMA_LOG immediately so the
  # RETURN trap (kill_satsuma) and any later wait_for_overlay_window
  # failure both target this process, on every return path below.
  SATSUMA_LOG="/tmp/e2e-linux-satsuma-first.log"
  "$SATSUMA_BIN" >"$SATSUMA_LOG" 2>&1 &
  local first_pid=$!
  SATSUMA_PID="$first_pid"
  if ! wait_until 5 "first instance is running" kill -0 "$first_pid"; then
    fail "single-instance: first instance never started"
    rm -rf "$dir"
    return 1
  fi
  # tauri-plugin-single-instance needs a moment to register itself as the
  # instance owner before a second launch can be forwarded to it.
  sleep 1

  place_cursor
  local second_log="/tmp/e2e-linux-satsuma-second.log"
  "$SATSUMA_BIN" "--mode=formats" "$src" >"$second_log" 2>&1 &
  local second_pid=$!

  if ! wait_until 5 "second invocation exits (forwarded, not a second window)" \
    bash -c "! kill -0 $second_pid 2>/dev/null"; then
    fail "single-instance: second process did not exit quickly - was it actually forwarded?"
    log "--- tail of $second_log ---"
    tail -n 60 "$second_log" >&2
    log "--- end of log ---"
  fi
  # Idempotent safety net regardless of the check above: never leak this.
  kill "$second_pid" >/dev/null 2>&1 || true

  if ! kill -0 "$first_pid" >/dev/null 2>&1; then
    fail "single-instance: original process exited unexpectedly"
    rm -rf "$dir"
    return 1
  fi

  local winid
  # This scenario's overlay has repeatedly needed more time than the
  # other three (already-stable) scenarios' fresh-process launches: first
  # to become interactive, and separately (a later CI run) to even appear
  # at all - both consistent with this being the resident process's own
  # first-ever overlay display (its webview is created and its first
  # navigation starts at startup even while hidden, but the overlay window
  # itself has never been shown before), not a one-off fluke worth chasing
  # further. 40s (up from 20s): the Windows twin of this scenario timed
  # out at 20s with a healthy forward and no regression in the sources (a
  # cold-runner WebView2 first-ever-show flake, see
  # tests/e2e-windows/run.ps1), so keep the same generous budget here for
  # parity.
  if ! winid="$(wait_for_overlay_window 40)"; then
    fail "single-instance: overlay never appeared in the original (first) process"
    # wait_for_overlay_window already dumped the first process's log; also
    # dump its still-visible windows (does the overlay exist but never
    # became visible?) and the second process's log (empty on a healthy
    # forward, so if the forward itself silently failed to reach the
    # first process, that is where it shows up).
    dump_visible_windows
    log "--- tail of $second_log ---"
    tail -n 60 "$second_log" >&2
    log "--- end of log ---"
  else
    focus_window "$winid"
    # A first real CI run showed this specific scenario's overlay (this
    # resident process's own webview, never shown before this point) can
    # take noticeably longer to become interactive than a fresh process's
    # - a more generous retry budget than the other, already-proven-stable
    # scenarios need.
    if ! click_wedge_until_hidden "$winid" "$JPG_INDEX" "$IMAGE_FORMATS_MINUS_PNG_COUNT" 15; then
      fail "single-instance: overlay did not hide after clicking"
    fi
    if ! wait_until 5 "converted sibling file appears" test -f "$expected_dest"; then
      fail "single-instance: $expected_dest was never written"
    else
      log "PASS: single-instance forwarding delivered the launch request and converted the file"
    fi
  fi

  if ! kill -0 "$first_pid" >/dev/null 2>&1; then
    fail "single-instance: original process died during the scenario"
  fi

  rm -rf "$dir"
}

# --- orchestration ---------------------------------------------------------

main() {
  if [ -z "${DISPLAY:-}" ]; then
    echo "[e2e-linux] \$DISPLAY is not set - run this under xvfb-run (see this file's header)" >&2
    exit 1
  fi

  for tool in xdotool wmctrl xwininfo python3; do
    if ! command -v "$tool" >/dev/null 2>&1; then
      echo "[e2e-linux] required tool '$tool' is not installed" >&2
      exit 1
    fi
  done

  if [ ! -d "$REPO_ROOT/node_modules" ]; then
    echo "[e2e-linux] $REPO_ROOT/node_modules not found - run 'npm ci' first (this harness starts the vite dev server itself, see start_vite in lib.sh)" >&2
    exit 1
  fi

  if [ "${SATSUMA_SKIP_BUILD:-0}" != "1" ]; then
    log "building satsuma (cargo build -p satsuma)"
    (cd "$REPO_ROOT" && cargo build -p satsuma) || {
      echo "[e2e-linux] build failed" >&2
      echo "[e2e-linux] if the error is 'resource path ... doesn't exist', run scripts/fetch-ffmpeg.sh and scripts/fetch-pdfium.sh first (the sidecars are gitignored - see AGENTS.md)" >&2
      exit 1
    }
  fi

  if [ ! -x "$SATSUMA_BIN" ]; then
    echo "[e2e-linux] binary not found at $SATSUMA_BIN" >&2
    exit 1
  fi

  start_vite
  start_wm
  trap cleanup_all EXIT

  log "=== formats-select writes the converted sibling file ==="
  scenario_formats_select_writes_sibling_file

  log "=== escape cancels without any side effect ==="
  scenario_escape_cancels_without_side_effect

  log "=== tools-mode opens and dismisses cleanly ==="
  scenario_tools_mode_opens_and_dismisses

  log "=== single-instance forwarding ==="
  scenario_single_instance_forwarding

  echo
  if [ "$FAILURES" -eq 0 ]; then
    log "all scenarios passed"
    exit 0
  else
    log "$FAILURES scenario(s) failed"
    if [ -f /tmp/e2e-linux-mutter.log ]; then
      log "--- tail of /tmp/e2e-linux-mutter.log ---"
      tail -n 40 /tmp/e2e-linux-mutter.log >&2
      log "--- end of log ---"
    fi
    exit 1
  fi
}

main "$@"
