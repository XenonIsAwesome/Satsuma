#!/usr/bin/env bash
# Downloads a static FFmpeg binary into src-tauri/lib/ffmpeg/ (the
# resource directory Tauri copies into the app's resource dir — NOT
# next to the executable, NOT into /usr/bin/) together with the
# GPL-3 license text.
#
# `tauri.conf.json`'s `bundle.resources` maps `lib/ffmpeg` into the
# packaged app's resource dir at `ffmpeg`, so the binary lives at
# <resource-dir>/ffmpeg/ffmpeg-x86_64-unknown-linux-gnu in the
# installer — e.g. /usr/lib/satsuma/ffmpeg/ffmpeg-x86_64-unknown-linux-gnu
# on the Debian package, alongside its COPYING.
#
# REQUIRED before any `cargo build`/`cargo check`/`cargo test`/`cargo
# tauri dev` that compiles the satsuma crate (run it once per
# checkout): the resources source directory is gitignored — no
# checked-in placeholder — and tauri-build validates the `resources`
# path on every build, failing with `resource path 'lib/ffmpeg'
# doesn't exist` until this script has run. See AGENTS.md's
# Commands section.
#
# Licensing note: this pulls the "gpl" build variant (needed for libx264/
# libx265, used by Satsuma's MP4/MOV/MKV encoders - see
# crates/satsuma-core/src/convert/video.rs) from BtbN/FFmpeg-Builds, a
# widely-used community project publishing static FFmpeg builds from
# unmodified upstream FFmpeg source. Bundling a GPL-licensed binary and
# invoking it as a separate subprocess (not linking against it) is the
# same approach many commercial and open-source apps take and does not
# require Satsuma itself to be GPL-licensed - but it does mean this
# specific binary's own source must remain available (it is, from FFmpeg's
# own project and BtbN's build scripts) and its license text should ship
# alongside it in release bundles (GPL-3 fetched from
# https://www.gnu.org/licenses/gpl-3.0.txt). Flagged here for visibility
# rather than decided silently - swap to the "lgpl" build variant instead
# if a non-GPL-encumbered sidecar is preferred, at the cost of losing
# libx264/libx265 H.264/HEVC encoding.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST_DIR="$REPO_ROOT/src-tauri/lib/ffmpeg"
TARGET_TRIPLE="x86_64-unknown-linux-gnu"
DEST="$DEST_DIR/ffmpeg-$TARGET_TRIPLE"

mkdir -p "$DEST_DIR"

# A real FFmpeg binary is comfortably larger than a stub (~80MB vs ~1KB)
# - use size as a cheap "is this already the real thing" check so
# re-running this script is a no-op for the binary.
binary_present=false
if [[ -f "$DEST" ]] && [[ "$(stat -c%s "$DEST" 2>/dev/null || stat -f%z "$DEST")" -gt 1048576 ]]; then
  binary_present=true
fi

if [[ "$binary_present" == "false" ]]; then
  ARCHIVE_URL="https://github.com/BtbN/FFmpeg-Builds/releases/latest/download/ffmpeg-master-latest-linux64-gpl.tar.xz"
  echo "Downloading $ARCHIVE_URL ..."
  WORK_DIR="$(mktemp -d)"
  trap 'rm -rf "$WORK_DIR"' EXIT
  curl -fL --retry 3 -o "$WORK_DIR/ffmpeg.tar.xz" "$ARCHIVE_URL"
  tar -xJf "$WORK_DIR/ffmpeg.tar.xz" -C "$WORK_DIR"
  FFMPEG_BIN="$(find "$WORK_DIR" -type f -path '*/bin/ffmpeg' | head -n1)"
  if [[ -z "$FFMPEG_BIN" ]]; then
    echo "error: could not find ffmpeg binary inside the downloaded archive" >&2
    exit 1
  fi
  cp "$FFMPEG_BIN" "$DEST"
  chmod +x "$DEST"
  echo "Installed $DEST"
else
  echo "Already present: $DEST"
fi

# License - small, downloaded every run (fast) or skipped if already there.
LICENSE_URL="https://www.gnu.org/licenses/gpl-3.0.txt"
LICENSE_DEST="$DEST_DIR/COPYING"
if [[ -f "$LICENSE_DEST" ]] && [[ -s "$LICENSE_DEST" ]]; then
  echo "Already present: $LICENSE_DEST"
else
  echo "Downloading $LICENSE_URL ..."
  curl -fL --retry 3 -o "$LICENSE_DEST" "$LICENSE_URL"
  echo "Installed $LICENSE_DEST"
fi