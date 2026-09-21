# Runs a real, end-to-end PDF -> JPG rasterization through the vendored
# pdfium shared library, so the release pipeline proves the pdfium build it
# just downloaded (see fetch-pdfium.ps1's pinned $PdfiumRelease) actually
# works with this app's pdfium-render 0.9 bindings - instead of shipping a
# silent PDF -> JPG/PNG / scanned-page-DOCX break.
#
# The smoke test lives in satsuma-core as an `#[ignore]`d test, because
# plain `cargo test` / `cargo build` must never need a vendored library
# (the ignore flag keeps it out of the normal unit runs); this script is
# what runs it for real by pointing it at src-tauri/lib/pdfium. deploy.yml
# calls this right after scripts/fetch-pdfium.ps1 in build-windows (and
# scripts/smoke-pdfium.sh on Linux).

$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$LibDir = Join-Path $RepoRoot "src-tauri\lib\pdfium"

if (-not (Test-Path (Join-Path $LibDir "pdfium.dll"))) {
    Write-Error "$LibDir\pdfium.dll not found - run scripts/fetch-pdfium.ps1 first"
    exit 1
}

Write-Host "Smoke-testing the vendored pdfium library at $LibDir ..."
$env:SATSUMA_PDFIUM_SMOKE_LIB_DIR = $LibDir
cargo test -p satsuma-core -- --ignored pdfium_smoke
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
Write-Host "pdfium smoke test passed."