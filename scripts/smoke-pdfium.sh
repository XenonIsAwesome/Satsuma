#!/usr/bin/env bash
# Runs a real, end-to-end PDF -> JPG rasterization through the vendored
# pdfium shared library, so the release pipeline proves the pdfium build it
# just downloaded (see fetch-pdfium.sh's pinned PDFIUM_RELEASE) actually
# works with this app's pdfium-render 0.9 bindings - instead of shipping a
# silent PDF -> JPG/PNG / scanned-page-DOCX break.
#
# The smoke test lives in satsuma-core as an `#[ignore]`d test, because
# plain `cargo test` / `cargo build` must never need a vendored library
# (the ignore flag keeps it out of the normal unit runs); this script is
# what runs it for real by pointing it at src-tauri/lib/pdfium. deploy.yml
# calls this right after scripts/fetch-pdfium.sh in build-linux (and
# scripts/smoke-pdfium.ps1 on Windows).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LIB_DIR="$REPO_ROOT/src-tauri/lib/pdfium"

if [[ ! -f "$LIB_DIR/libpdfium.so" ]]; then
  echo "error: $LIB_DIR/libpdfium.so not found - run scripts/fetch-pdfium.sh first" >&2
  exit 1
fi

echo "Smoke-testing the vendored pdfium library at $LIB_DIR ..."
SATSUMA_PDFIUM_SMOKE_LIB_DIR="$LIB_DIR" cargo test -p satsuma-core -- --ignored pdfium_smoke
echo "pdfium smoke test passed."