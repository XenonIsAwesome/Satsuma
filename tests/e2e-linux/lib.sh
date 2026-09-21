# Shared helpers for tests/e2e-linux/run.sh. Sourced, not executed directly.
#
# Assumes it's already running inside a real X server (run.sh is meant to be
# invoked as `xvfb-run --auto-servernum tests/e2e-linux/run.sh`, which sets
# $DISPLAY for the whole script) — this file only starts the window manager
# within that display, puppets it via xdotool/wmctrl, and launches the
# built satsuma binary directly (`--mode=formats|tools <path>`), the same
# entry point a Nemo/Nautilus/Dolphin context-menu action produces (see
# crates/satsuma-core/src/launch_request.rs).

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
SATSUMA_BIN="${SATSUMA_BIN:-$REPO_ROOT/target/debug/satsuma}"
WEDGE_POINT="$SCRIPT_DIR/wedge_point.py"

# WebKitGTK 2.4x's DMA-BUF renderer (its default GPU compositing path since
# recent libwebkit2gtk-4.1 releases) is a well-documented cause of a
# WebProcess that starts, maps a window, and then never actually paints or
# responds to input at all under Xvfb/CI - no real GPU/DRM device to back
# it, unlike mutter's own compositing (which at least falls back to
# software rendering — see the DRI3 warnings in its own log). Exported
# here, not passed per-launch, so it's set before every satsuma invocation
# this script makes, including the raw `&` launches in run.sh's
# single-instance scenario.
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export WEBKIT_DISABLE_COMPOSITING_MODE=1

# Fixed, deliberately-chosen trigger point: comfortably inside the default
# Xvfb screen (1280x1024) so a LABEL_RADIUS=95 wedge click never clips off
# the virtual screen, matching the cursor position the app itself will read
# via query_cursor_position() before we ever launch it.
CURSOR_X=400
CURSOR_Y=400

WM_PID=""
SATSUMA_PID=""
SATSUMA_LOG=""
FAILURES=0

log() {
  echo "[e2e-linux] $*"
}

# dump_satsuma_log
# Prints the tail of the currently-tracked satsuma process's own stdout/
# stderr (set by launch_satsuma, or manually before a raw `&` launch) so a
# CI failure is self-diagnosing instead of just "the window never
# appeared" with no clue why.
dump_satsuma_log() {
  if [ -n "$SATSUMA_LOG" ] && [ -f "$SATSUMA_LOG" ]; then
    log "--- tail of $SATSUMA_LOG ---"
    tail -n 60 "$SATSUMA_LOG" >&2
    log "--- end of log ---"
  fi
}

fail() {
  echo "[e2e-linux] FAIL: $*" >&2
  FAILURES=$((FAILURES + 1))
}

# wait_until <timeout_secs> <description> <command...>
# Polls `command` every 0.2s until it exits 0, or fails after timeout_secs.
wait_until() {
  local timeout="$1" desc="$2"
  shift 2
  local waited=0
  while ! "$@" >/dev/null 2>&1; do
    sleep 0.2
    waited="$(awk "BEGIN{print $waited+0.2}")"
    if awk "BEGIN{exit !($waited > $timeout)}"; then
      log "timed out waiting for: $desc"
      return 1
    fi
  done
  return 0
}

start_wm() {
  if command -v mutter >/dev/null 2>&1; then
    log "starting mutter (X11 mode) as the window manager"
    mutter --x11 --replace --sm-disable >/tmp/e2e-linux-mutter.log 2>&1 &
    WM_PID=$!
  elif command -v openbox >/dev/null 2>&1; then
    log "mutter not found, falling back to openbox"
    openbox >/tmp/e2e-linux-openbox.log 2>&1 &
    WM_PID=$!
  else
    echo "[e2e-linux] neither mutter nor openbox is installed" >&2
    exit 1
  fi
  # A window manager announces itself by adopting _NET_SUPPORTING_WM_CHECK;
  # wmctrl -m only succeeds once one is running.
  if ! wait_until 10 "window manager ready" wmctrl -m; then
    fail "window manager never became ready"
    exit 1
  fi
}

stop_wm() {
  if [ -n "$WM_PID" ]; then
    kill "$WM_PID" >/dev/null 2>&1 || true
    wait "$WM_PID" 2>/dev/null || true
    WM_PID=""
  fi
}

VITE_PID=""
VITE_PORT=1420 # tauri.conf.json's devUrl / vite.config.ts's server.port

# start_vite
# `cargo build -p satsuma` (what this harness uses, deliberately, to mirror
# the exact context-menu/Explorer-trigger invocation) is a plain debug
# build with no `custom-protocol` feature - there's no such feature
# declared anywhere in src-tauri/Cargo.toml at all. That means the webview
# always loads tauri.conf.json's `devUrl` (http://localhost:1420), the
# same as a real `cargo tauri dev` session - `frontendDist`/`dist/` is
# irrelevant here regardless of whether it's been built. Ordinarily
# `cargo tauri dev` starts this dev server itself via `beforeDevCommand`;
# calling `cargo build` directly (bypassing the tauri-cli entirely) skips
# that, so this harness has to start it explicitly - otherwise the webview
# loads a connection-refused blank page and is permanently inert: the
# native window still opens, focuses, and is positioned correctly (all
# Rust-side), but nothing in it is ever interactive, exactly what every
# prior CI run here showed regardless of window-targeting or timing fixes.
start_vite() {
  log "starting vite dev server (npm run dev) on port $VITE_PORT"
  (cd "$REPO_ROOT" && npm run dev) >/tmp/e2e-linux-vite.log 2>&1 &
  VITE_PID=$!
  # "localhost" (not the literal 127.0.0.1) to match what tauri.conf.json's
  # devUrl and vite.config.ts's unset `server.host` both actually resolve
  # to - on a host where "localhost" prefers IPv6, vite/Node bind only
  # ::1, and a literal-127.0.0.1 probe would report connection-refused
  # (and time out this whole wait) even while the server is genuinely up
  # and ready, as a first real CI run showed.
  if ! wait_until 30 "vite dev server ready on port $VITE_PORT" \
    bash -c "exec 3<>/dev/tcp/localhost/$VITE_PORT"; then
    log "--- tail of /tmp/e2e-linux-vite.log ---"
    tail -n 60 /tmp/e2e-linux-vite.log >&2
    log "--- end of log ---"
    fail "vite dev server never became ready"
    exit 1
  fi
  # Best-effort warm-up: the TCP check above only proves the listener is up,
  # but vite prebundles deps and transforms modules on the FIRST http request
  # (cold, can take 20-60s on a fresh runner), and the app's very first
  # webview navigation is exactly that request - on Windows a first cold
  # launch has timed out waiting for the overlay window while the webview
  # sat on vite's cold prebundle. Force that first request to complete here,
  # before the app ever asks, so the webview's first navigation is warm.
  # Best-effort on purpose: if the warm-up itself never completes the app
  # waits below still apply and will surface any real regression.
  if ! wait_until 60 "vite first-request warm-up" \
    curl -sS --max-time 5 -o /dev/null "http://localhost:$VITE_PORT/"; then
    log "warning: vite first-request warm-up never completed - continuing anyway"
  fi
}

stop_vite() {
  if [ -n "$VITE_PID" ]; then
    # `npm run dev` may fork a child node process rather than exec'ing
    # into it depending on the npm version, so also kill any children of
    # $VITE_PID directly - otherwise the real vite server can outlive this
    # script and hold port $VITE_PORT for a subsequent local run.
    pkill -P "$VITE_PID" >/dev/null 2>&1 || true
    kill "$VITE_PID" >/dev/null 2>&1 || true
    wait "$VITE_PID" 2>/dev/null || true
    VITE_PID=""
  fi
}

# make_scratch_dir <fixture-name>
# Copies one named fixture (from tests/fixtures/, shared with
# tests/e2e-windows - not duplicated per platform) into a fresh temp dir
# and prints that dir's path, so each scenario gets its own isolated input
# file (and destination for convert_file's sibling-file write) with no
# cross-talk between runs.
make_scratch_dir() {
  local fixture="$1"
  local dir
  dir="$(mktemp -d /tmp/satsuma-e2e-linux.XXXXXX)"
  cp "$REPO_ROOT/tests/fixtures/$fixture" "$dir/$fixture"
  echo "$dir"
}

# is_valid_jpeg <path>
# Checks that <path> starts with the JPEG SOI marker (0xFFD8). Phase 2's
# real conversion re-encodes the source rather than copying its bytes (see
# crates/satsuma-core/src/convert/image.rs), so a converted PNG -> JPG
# output is expected to differ byte-for-byte from the source - this checks
# the output is a genuine JPEG rather than comparing it against the
# source at all.
is_valid_jpeg() {
  local magic
  magic="$(head -c 2 "$1" | od -An -tx1 | tr -d ' \n')"
  [ "$magic" = "ffd8" ]
}

# launch_satsuma <mode> <path>
# Starts the built binary the same way a context-menu action would
# (`--mode=formats|tools <path>`), backgrounds it, and records its PID.
launch_satsuma() {
  local mode="$1" path="$2"
  SATSUMA_LOG="/tmp/e2e-linux-satsuma.log"
  "$SATSUMA_BIN" "--mode=$mode" "$path" >"$SATSUMA_LOG" 2>&1 &
  SATSUMA_PID=$!
}

kill_satsuma() {
  if [ -n "$SATSUMA_PID" ] && kill -0 "$SATSUMA_PID" >/dev/null 2>&1; then
    kill "$SATSUMA_PID" >/dev/null 2>&1 || true
    wait "$SATSUMA_PID" 2>/dev/null || true
  fi
  SATSUMA_PID=""
}

# find_overlay_window
# Among all currently visible windows owned by $SATSUMA_PID, prints the id
# of the one whose size roughly matches the overlay's fixed OVERLAY_SIZE
# (480x480 — src-tauri/src/lib.rs / tauri.conf.json), or fails if none
# match. A first real CI run (on the Windows side, but applied here too
# defensively) showed the same process can legitimately have more than one
# visible top-level window at once — blindly taking "the first match"
# isn't safe, since that turned out to sometimes be a tiny helper window or
# the (much larger) main window instead of the actual overlay.
find_overlay_window() {
  local id
  for id in $(xdotool search --pid "$SATSUMA_PID" --onlyvisible 2>/dev/null); do
    local geometry
    local X Y WIDTH HEIGHT SCREEN
    geometry="$(xdotool getwindowgeometry --shell "$id" 2>/dev/null)" || continue
    eval "$geometry"
    if [ "$WIDTH" -ge 300 ] && [ "$WIDTH" -le 700 ] && [ "$HEIGHT" -ge 300 ] && [ "$HEIGHT" -le 700 ]; then
      echo "$id"
      return 0
    fi
  done
  return 1
}

# wait_for_overlay_window <timeout_secs>
# Prints the overlay window's id once find_overlay_window locates it.
# Fails fast (rather than waiting out the full timeout) if the tracked
# process has already died, and either way dumps its log on failure so a
# CI run is self-diagnosing.
#
# The native window becoming visible (what find_overlay_window checks)
# and the webview's page actually being interactive are two different
# events: OverlayApp only renders the wedge menu once its own
# useLaunchRequest hook resolves an async `invoke("take_launch_request")`
# IPC round trip, and for a fresh process launch (the direct-invocation
# path this harness always uses) that's racing the webview's very first
# page load/bundle-execution/React-mount, not just a quick re-render. A
# first real CI run showed the window being found (right size, right
# position) and clicks/Escape still doing nothing at all afterward - not
# even the wedge menu's own click-outside-to-cancel backdrop responded -
# consistent with interacting before any handler was attached yet, not a
# targeting problem. This settle delay gives that startup sequence room to
# finish before the harness ever touches the window; on a real user's warm
# desktop this window-to-interactive gap is normally imperceptible, but a
# cold CI runner's first launch can plausibly take longer.
wait_for_overlay_window() {
  local timeout="$1"
  if ! kill -0 "$SATSUMA_PID" >/dev/null 2>&1; then
    log "satsuma process (pid $SATSUMA_PID) is not running - it must have exited/crashed already"
    dump_satsuma_log
    return 1
  fi
  if ! wait_until "$timeout" "overlay window mapped" find_overlay_window; then
    dump_satsuma_log
    return 1
  fi
  sleep 1.5
  find_overlay_window
}

overlay_window_absent() {
  ! find_overlay_window >/dev/null 2>&1
}

# dump_visible_windows
# Prints whatever windows are still visible for $SATSUMA_PID, for
# diagnosing a click_wedge_until_hidden/send_escape_until_hidden failure.
dump_visible_windows() {
  log "--- windows still visible for pid $SATSUMA_PID (wmctrl -lp) ---"
  wmctrl -lp | awk -v pid="$SATSUMA_PID" '$3==pid' >&2
  log "--- end of window list ---"
}

# focus_window <winid>
# Makes the given window the one holding X11 input focus before puppeting
# it, via a direct XSetInputFocus (`xdotool windowfocus`) rather than
# `windowactivate`'s EWMH _NET_ACTIVE_WINDOW client message: Mutter/Muffin
# distrusts an untimestamped activation request exactly like the one
# `windowactivate` sends (logged as "Buggy client sent a _NET_ACTIVE_WINDOW
# message with a timestamp of 0" — this project's own
# force_activate_window in src-tauri/src/linux_integration.rs exists to
# work around that same distrust for the app's own window, with a properly
# -timestamped request; windowfocus sidesteps the whole problem by not
# going through the WM at all). Without this, xdotool's synthesized key
# events (send_escape) could silently go to whichever window previously
# held focus instead of the overlay.
focus_window() {
  xdotool windowfocus "$1"
}

# window_center <winid>
# Queries the window's actual on-screen geometry and prints its center
# ("X Y"). Used instead of assuming the window is centered on wherever we
# placed the cursor before launching (place_cursor's CURSOR_X/Y) - Mutter
# can reposition/clamp a newly-shown window to keep it on-screen, which
# would silently invalidate that assumption, so this queries the ground
# truth instead.
window_center() {
  local winid="$1"
  local geometry
  local X Y WIDTH HEIGHT SCREEN
  geometry="$(xdotool getwindowgeometry --shell "$winid")"
  eval "$geometry"
  echo "$((X + WIDTH / 2)) $((Y + HEIGHT / 2))"
}

# diag_input_state <winid> <label>
# Ground-truth diagnostics for exactly what the X server thinks is going
# on with input delivery right now: which window has input focus, this
# window's own map-state/viewability, and the current pointer location.
# Every prior fix here (right window, right coordinates, settle delay,
# retries, WEBKIT_DISABLE_DMABUF_RENDERER) still hasn't gotten a single
# click or Escape to have any effect at all across many retries - which
# rules out a timing race - so the next step is confirming or ruling out
# each layer directly instead of guessing again.
diag_input_state() {
  local winid="$1" label="$2"
  log "[diag $label] active window: $(xdotool getactivewindow 2>&1) ($(xdotool getactivewindow getwindowname 2>&1))"
  log "[diag $label] target window ($winid) info:"
  xwininfo -id "$winid" 2>&1 | grep -E 'Map State|Visibility|Class|Width|Height' | while IFS= read -r line; do
    log "[diag $label]   $line"
  done
  log "[diag $label] pointer location: $(xdotool getmouselocation 2>&1)"
}

# click_wedge_option <winid> <index> <count>
# Moves the real cursor to the option's midpoint (relative to the window's
# actual on-screen center, via wedge_point.py) and performs a real left
# click there.
click_wedge_option() {
  local winid="$1" index="$2" count="$3"
  local center center_x center_y point
  center="$(window_center "$winid")"
  center_x="${center% *}"
  center_y="${center#* }"
  point="$(python3 "$WEDGE_POINT" "$center_x" "$center_y" "$index" "$count")"
  diag_input_state "$winid" "before-click"
  log "[diag before-click] clicking at ${point% *},${point#* }"
  xdotool mousemove "${point% *}" "${point#* }"
  log "[diag after-move] pointer location: $(xdotool getmouselocation 2>&1)"
  xdotool click 1
}

place_cursor() {
  xdotool mousemove "$CURSOR_X" "$CURSOR_Y"
}

send_escape() {
  log "[diag before-escape] active window: $(xdotool getactivewindow 2>&1) ($(xdotool getactivewindow getwindowname 2>&1))"
  xdotool key Escape
}

# click_wedge_until_hidden <winid> <index> <count> <timeout_secs>
# Repeats click_wedge_option every 0.5s until the overlay hides or
# timeout_secs elapses, instead of trusting a single click at some assumed-
# ready moment. Safe to repeat: a formats-mode click just re-runs
# convert_file, collision-safe named (see naming::output_path), and
# hide_overlay is a no-op once already hidden. Hedges against not knowing
# exactly how long the webview takes to become interactive after its
# window is mapped (see wait_for_overlay_window's doc comment) without
# having to guess a single "long enough" number.
click_wedge_until_hidden() {
  local winid="$1" index="$2" count="$3" timeout="$4"
  local waited=0
  while true; do
    click_wedge_option "$winid" "$index" "$count"
    if overlay_window_absent; then
      return 0
    fi
    sleep 0.5
    waited="$(awk "BEGIN{print $waited+0.5}")"
    if awk "BEGIN{exit !($waited > $timeout)}"; then
      dump_visible_windows
      return 1
    fi
  done
}

# send_escape_until_hidden <timeout_secs>
# Same idea as click_wedge_until_hidden, for the Escape-cancel path.
send_escape_until_hidden() {
  local timeout="$1"
  local waited=0
  while true; do
    send_escape
    if overlay_window_absent; then
      return 0
    fi
    sleep 0.5
    waited="$(awk "BEGIN{print $waited+0.5}")"
    if awk "BEGIN{exit !($waited > $timeout)}"; then
      dump_visible_windows
      return 1
    fi
  done
}

cleanup_all() {
  kill_satsuma
  stop_wm
  stop_vite
}
