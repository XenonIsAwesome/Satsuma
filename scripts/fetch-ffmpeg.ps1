# Downloads a static FFmpeg binary into src-tauri/lib/ffmpeg/ (the
# resource directory Tauri copies into the app's resource dir - NOT
# next to the executable, NOT into /usr/bin/) together with the GPL-3
# license text.
#
# `tauri.conf.json`'s `bundle.resources` maps `lib/ffmpeg` into the
# packaged app's resource directory at `ffmpeg`, so the binary lives at
# <resource-dir>/ffmpeg/ffmpeg-x86_64-pc-windows-msvc.exe in the
# installer, alongside its COPYING.
#
# REQUIRED before any `cargo build`/`cargo check`/`cargo test`/`cargo
# tauri dev` that compiles the satsuma crate (run it once per
# checkout): the resources source directory is gitignored - no
# checked-in placeholder - and tauri-build validates the `resources`
# path on every build, failing with `resource path 'lib/ffmpeg'
# doesn't exist` until this script has run. See AGENTS.md's
# Commands section.
#
# See fetch-ffmpeg.sh for the Linux equivalent and its licensing
# note (this script pulls the same "gpl" build variant, for the same
# libx264/libx265 reason).

$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$DestDir = Join-Path $RepoRoot "src-tauri\lib\ffmpeg"
$TargetTriple = "x86_64-pc-windows-msvc"
$Dest = Join-Path $DestDir "ffmpeg-$TargetTriple.exe"

New-Item -ItemType Directory -Force -Path $DestDir | Out-Null

# A real FFmpeg binary is ~80MB; use size as a cheap "is this already
# the real thing" check so re-running this script is a no-op for the
# binary.
if ((Test-Path $Dest) -and ((Get-Item $Dest).Length -gt 1MB)) {
    Write-Host "Already present: $Dest"
} else {
    $WorkDir = Join-Path ([System.IO.Path]::GetTempPath()) ([System.IO.Path]::GetRandomFileName())
    New-Item -ItemType Directory -Force -Path $WorkDir | Out-Null

    try {
        $ArchiveUrl = "https://github.com/BtbN/FFmpeg-Builds/releases/latest/download/ffmpeg-master-latest-win64-gpl.zip"
        $ArchivePath = Join-Path $WorkDir "ffmpeg.zip"
        Write-Host "Downloading $ArchiveUrl ..."
        Invoke-WebRequest -Uri $ArchiveUrl -OutFile $ArchivePath

        Expand-Archive -Path $ArchivePath -DestinationPath $WorkDir

        $FfmpegBin = Get-ChildItem -Path $WorkDir -Recurse -Filter "ffmpeg.exe" | Select-Object -First 1
        if (-not $FfmpegBin) {
            Write-Error "could not find ffmpeg.exe inside the downloaded archive"
            exit 1
        }

        Copy-Item -Path $FfmpegBin.FullName -Destination $Dest
        Write-Host "Installed $Dest"
    } finally {
        Remove-Item -Recurse -Force $WorkDir -ErrorAction SilentlyContinue
    }
}

# License - GPL-3, needed alongside the binary in the resource directory.
# Small, downloaded every run (fast) or skipped if already present.
$LicenseUrl = "https://www.gnu.org/licenses/gpl-3.0.txt"
$LicenseDest = Join-Path $DestDir "COPYING"
if ((Test-Path $LicenseDest) -and ((Get-Item $LicenseDest).Length -gt 0)) {
    Write-Host "Already present: $LicenseDest"
} else {
    Write-Host "Downloading $LicenseUrl ..."
    Invoke-WebRequest -Uri $LicenseUrl -OutFile $LicenseDest
    Write-Host "Installed $LicenseDest"
}