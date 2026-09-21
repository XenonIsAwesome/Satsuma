#requires -version 5.1
<#
.SYNOPSIS
    Satsuma bootstrap installer for Windows.

.DESCRIPTION
    Downloads the requested (or latest) GitHub Release's Windows installer
    and runs it silently - the .msi (WiX) by default, or the .exe (NSIS)
    bundle with -Type exe.

    Hosted (via .github/workflows/docs-deploy.yml, which copies this file
    verbatim into the published docs/ Pages site - see "Assemble site/" in
    that workflow) at:

        https://xenonisawesome.github.io/Satsuma/install.ps1

.PARAMETER Version
    A release tag (e.g. "v1.2.0" or "1.2.0" - the "v" is optional) or
    "latest" (default). Falls back to $env:SATSUMA_VERSION so the simple
    piped form below can still be pointed at a specific version.

.PARAMETER Type
    "msi" (default) or "exe". Falls back to $env:SATSUMA_TYPE.

.EXAMPLE
    irm https://xenonisawesome.github.io/Satsuma/install.ps1 | iex

.EXAMPLE
    # Passing parameters through a piped invocation:
    &([scriptblock]::Create((irm https://xenonisawesome.github.io/Satsuma/install.ps1))) -Version v1.2.0 -Type exe

.EXAMPLE
    # Or, run locally:
    .\install.ps1 -Version v1.2.0 -Type exe
#>
[CmdletBinding()]
param(
    [string]$Version = $(if ($env:SATSUMA_VERSION) { $env:SATSUMA_VERSION } else { "latest" }),
    [ValidateSet("msi", "exe")]
    [string]$Type = $(if ($env:SATSUMA_TYPE) { $env:SATSUMA_TYPE } else { "msi" })
)

$ErrorActionPreference = "Stop"

$Repo = "XenonIsAwesome/Satsuma"
$ApiBase = "https://api.github.com/repos/$Repo"
$ReleasesUrl = "https://github.com/$Repo/releases"

function Die($Message) {
    # Not Write-Error: with $ErrorActionPreference = "Stop" that would
    # itself become a terminating error, making the "exit 1" below
    # unreachable and the actual exit code non-deterministic.
    Write-Host "error: $Message" -ForegroundColor Red
    exit 1
}

# A .ps1 can be run through pwsh on Linux/macOS too - this bootstrap only
# makes sense on Windows (that's what the .msi/.exe are for). $env:WINDIR
# is reliably Windows-only, unlike $IsWindows (absent on Windows
# PowerShell 5.1, only defined from PowerShell 6+).
if (-not $env:WINDIR) {
    Die "install.ps1 is for Windows. Linux users: curl -fsSL https://xenonisawesome.github.io/Satsuma/install.sh | sh"
}

# Windows PowerShell 5.1 on older Windows builds defaults to a TLS
# protocol list that excludes TLS 1.2, which api.github.com/github.com
# require - without this, Invoke-RestMethod below fails with an opaque
# "underlying connection was closed" error rather than anything
# actionable.
[System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor [System.Net.SecurityProtocolType]::Tls12

# --- 1. Resolve the release and find the matching asset --------------------

if ($Version -eq "latest") {
    $apiUrl = "$ApiBase/releases/latest"
}
else {
    $tag = if ($Version.StartsWith("v")) { $Version } else { "v$Version" }
    $apiUrl = "$ApiBase/releases/tags/$tag"
}

Write-Host "==> Looking up the $Version release..."
try {
    $release = Invoke-RestMethod -Uri $apiUrl -Headers @{ "User-Agent" = "satsuma-install.ps1" }
}
catch {
    Die "couldn't find a release for '$Version' at $apiUrl (check the version exists: $ReleasesUrl)"
}

$asset = $release.assets | Where-Object { $_.name -like "*.$Type" } | Select-Object -First 1
if (-not $asset) {
    Die "the $Version release has no .$Type asset - it may still be building, or try -Type $(if ($Type -eq 'msi') { 'exe' } else { 'msi' })"
}

# --- 2. Download ------------------------------------------------------------

$downloadPath = Join-Path $env:TEMP $asset.name
Write-Host "==> Downloading $($asset.name)..."
try {
    Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $downloadPath -UseBasicParsing
}
catch {
    Die "download failed: $($asset.browser_download_url)"
}

if (-not (Test-Path $downloadPath) -or (Get-Item $downloadPath).Length -eq 0) {
    Die "downloaded file is empty: $downloadPath"
}

# --- 3. Install --------------------------------------------------------------
# msiexec/the NSIS .exe trigger their own UAC elevation prompt if needed
# (same as double-clicking the installer would) - nothing extra required
# here to request admin rights.

try {
    if ($Type -eq "msi") {
        Write-Host "==> Running the MSI installer silently..."
        $proc = Start-Process -FilePath "msiexec.exe" -ArgumentList "/i", "`"$downloadPath`"", "/quiet", "/norestart" -Wait -PassThru
        # 0 = success, 3010 = success, reboot required later - both are a
        # completed install, not a failure.
        if ($proc.ExitCode -ne 0 -and $proc.ExitCode -ne 3010) {
            Die "msiexec exited with code $($proc.ExitCode)"
        }
    }
    else {
        Write-Host "==> Running the NSIS installer silently..."
        $proc = Start-Process -FilePath $downloadPath -ArgumentList "/S" -Wait -PassThru
        if ($proc.ExitCode -ne 0) {
            Die "installer exited with code $($proc.ExitCode)"
        }
    }
}
finally {
    Remove-Item $downloadPath -ErrorAction SilentlyContinue
}

Write-Host "==> Done. Launch Satsuma from the Start Menu."
