#!/usr/bin/env bash
# Downloads a prebuilt pdfium shared library and places it under
# src-tauri/lib/pdfium/ (tauri.conf.json's `bundle.resources` maps that
# directory into the packaged app's resource dir at "pdfium" - see
# src-tauri/src/pdfium.rs's resolver), for the Linux build. See
# fetch-pdfium.ps1 for the Windows equivalent.
#
# REQUIRED before any `cargo build`/`cargo check`/`cargo test`/`cargo
# tauri dev` that compiles the satsuma crate (run it once per
# checkout): the directory is gitignored - there is no checked-in
# placeholder - and tauri-codegen validates the `resources` source path
# exists on every build, failing with `resource path 'lib/pdfium'
# doesn't exist` until this script has been run. See AGENTS.md's
# Commands section.
#
# Pinned to a specific release tag rather than "latest": unbounded `latest`
# can drift into a pdfium build that pdfium-render 0.9's bindings were
# never verified against, silently breaking PDF -> JPG/PNG rasterization
# (and the scanned-page PDF -> DOCX fallback) in shipped installers.
# `chromium/8057` is the release whose compatibility this app's
# pdfium-render version is checked against end-to-end in CI by
# scripts/smoke-pdfium.sh (which deploy.yml runs right after this script),
# and the version bumped here should only move together with a verified
# smoke re-run on both platforms.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST_DIR="$REPO_ROOT/src-tauri/lib/pdfium"
DEST="$DEST_DIR/libpdfium.so"

# bblanchon/pdfium-binaries release tag - verified against pdfium-render
# 0.9 by scripts/smoke-pdfium.sh (see above). Keep in sync with the .ps1
# sibling.
PDFIUM_RELEASE="chromium/8057"

# A real pdfium library is ~7MB; use size as a cheap "is this already
# the real thing" check so re-running this script is a no-op for the
# library.
if [[ -f "$DEST" ]] && [[ "$(stat -c%s "$DEST" 2>/dev/null || stat -f%z "$DEST")" -gt 1048576 ]]; then
  echo "Already present: $DEST"
else
  mkdir -p "$DEST_DIR"
  WORK_DIR="$(mktemp -d)"
  trap 'rm -rf "$WORK_DIR"' EXIT

  ARCHIVE_URL="https://github.com/bblanchon/pdfium-binaries/releases/download/${PDFIUM_RELEASE}/pdfium-linux-x64.tgz"
  echo "Downloading $ARCHIVE_URL ..."
  curl -fL --retry 3 -o "$WORK_DIR/pdfium.tgz" "$ARCHIVE_URL"
  tar -xzf "$WORK_DIR/pdfium.tgz" -C "$WORK_DIR"

  LIB_FILE="$(find "$WORK_DIR" -type f -name 'libpdfium.so' | head -n1)"
  if [[ -z "$LIB_FILE" ]]; then
    echo "error: could not find libpdfium.so inside the downloaded archive" >&2
    exit 1
  fi

  cp "$LIB_FILE" "$DEST"
  echo "Installed $DEST"
fi

# License - BSD-3-Clause, needed alongside the library in the resource
# directory. Small, downloaded every run (fast) or skipped if already
# present.
LICENSE_URL="https://raw.githubusercontent.com/bblanchon/pdfium-binaries/${PDFIUM_RELEASE}/LICENSE"
LICENSE_DEST="$DEST_DIR/LICENSE-BSD3.txt"
if [[ -f "$LICENSE_DEST" ]] && [[ -s "$LICENSE_DEST" ]]; then
  echo "Already present: $LICENSE_DEST"
else
  echo "Downloading $LICENSE_URL ..."
  curl -fL --retry 3 -o "$LICENSE_DEST" "$LICENSE_URL"
  echo "Installed $LICENSE_DEST"
fi