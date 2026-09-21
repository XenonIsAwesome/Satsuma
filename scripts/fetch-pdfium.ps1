# Downloads a prebuilt pdfium shared library and places it under
# src-tauri/lib/pdfium/ (tauri.conf.json's `bundle.resources` maps that
# directory into the packaged app's resource dir at "pdfium" - see
# src-tauri/src/pdfium.rs's resolver), for the Windows build. See
# fetch-pdfium.sh for the Linux equivalent and its full doc comment
# (same source: bblanchon/pdfium-binaries, BSD-3-Clause).
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
# scripts/smoke-pdfium.ps1 (which deploy.yml runs right after this script),
# and the version bumped here should only move together with a verified
# smoke re-run on both platforms.

$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$DestDir = Join-Path $RepoRoot "src-tauri\lib\pdfium"
$Dest = Join-Path $DestDir "pdfium.dll"

# bblanchon/pdfium-binaries release tag - verified against pdfium-render
# 0.9 by scripts/smoke-pdfium.ps1 (see above). Keep in sync with the .sh
# sibling.
$PdfiumRelease = "chromium/8057"

# A real pdfium library is ~7MB; use size as a cheap "is this already
# the real thing" check so re-running this script is a no-op for the
# library.
if ((Test-Path $Dest) -and ((Get-Item $Dest).Length -gt 1MB)) {
    Write-Host "Already present: $Dest"
} else {
    New-Item -ItemType Directory -Force -Path $DestDir | Out-Null
    $WorkDir = Join-Path ([System.IO.Path]::GetTempPath()) ([System.IO.Path]::GetRandomFileName())
    New-Item -ItemType Directory -Force -Path $WorkDir | Out-Null

    try {
        $ArchiveUrl = "https://github.com/bblanchon/pdfium-binaries/releases/download/$PdfiumRelease/pdfium-win-x64.tgz"
        $ArchivePath = Join-Path $WorkDir "pdfium.tgz"
        Write-Host "Downloading $ArchiveUrl ..."
        Invoke-WebRequest -Uri $ArchiveUrl -OutFile $ArchivePath

        tar -xzf $ArchivePath -C $WorkDir

        $LibFile = Get-ChildItem -Path $WorkDir -Recurse -Filter "pdfium.dll" | Select-Object -First 1
        if (-not $LibFile) {
            Write-Error "could not find pdfium.dll inside the downloaded archive"
            exit 1
        }

        Copy-Item -Path $LibFile.FullName -Destination $Dest
        Write-Host "Installed $Dest"
    } finally {
        Remove-Item -Recurse -Force $WorkDir -ErrorAction SilentlyContinue
    }
}

# License - BSD-3-Clause, needed alongside the library in the resource
# directory. Small, downloaded every run (fast) or skipped if already
# present.
$LicenseUrl = "https://raw.githubusercontent.com/bblanchon/pdfium-binaries/$PdfiumRelease/LICENSE"
$LicenseDest = Join-Path $DestDir "LICENSE-BSD3.txt"
if ((Test-Path $LicenseDest) -and ((Get-Item $LicenseDest).Length -gt 0)) {
    Write-Host "Already present: $LicenseDest"
} else {
    Write-Host "Downloading $LicenseUrl ..."
    Invoke-WebRequest -Uri $LicenseUrl -OutFile $LicenseDest
    Write-Host "Installed $LicenseDest"
}