<#
.SYNOPSIS
Windows virtual-desktop-adjacent integration tests for Satsuma.

.DESCRIPTION
Launches the actual compiled satsuma.exe the same way a Windows Explorer/
Shift-hook trigger's downstream effect does (`--mode=formats|tools <path>`,
the same entry point launch_request.rs parses - see the doc comment in
lib.ps1), puppets the real mouse cursor/keyboard, and asserts on real,
observable side effects: the overlay window actually appearing/hiding, and
convert_file's real sibling-file write — a genuine re-encode as of Phase 2, not Phase 0's byte-copy stub — actually happening on disk. No
test-only hooks in the app itself - mirrors tests/e2e-linux/run.sh's design
exactly, swapping xdotool/wmctrl for SetCursorPos/mouse_event/EnumWindows.

Runs on a windows-latest GitHub-hosted runner's own real interactive
desktop session - no virtual-display layer is needed there the way Xvfb is
needed on Linux. Requires `npm ci` already run: this script starts its own
vite dev server (Start-ViteDevServer in lib.ps1) - a plain `cargo build`
has no `custom-protocol` feature, so the webview always loads
tauri.conf.json's devUrl (localhost:1420) rather than frontendDist/dist,
same as a real `cargo tauri dev` session, and ordinarily `cargo tauri dev`
is what would start that dev server.

CAVEAT: this script itself is unverified - src-tauri/src/windows_integration.rs
has already been built and manually verified end-to-end in a real Windows
VM (see that file's own doc comment), so the hook/overlay/COM path it
exercises is known-good; what's new and untested here is the harness's own
puppeteering (SetCursorPos/mouse_event/EnumWindows, and Shell.Application
COM automation used to drive Explorer rather than just read its selection).
Expect to iterate after the first genuine windows-latest CI run (see the
Windows section of the implementation plan for known risk areas:
foreground-window restrictions, DPI/resolution assumptions, and COM/thread
timing between this script's synthesized input and the app's own hook).

.PARAMETER SatsumaBin
Path to the built satsuma.exe. Defaults to target\debug\satsuma.exe under
the repo root.

.PARAMETER SkipBuild
Skip `cargo build -p satsuma` and use whatever is already at -SatsumaBin.

.PARAMETER IncludeExplorerScenario
Also run the Explorer/Shift-hook scenario (the only way to exercise
windows_integration.rs's actual keyboard-hook/COM-selection code path).
Off by default: land the shared, direct-invocation scenarios first and
confirm they're stable on real windows-latest CI before turning this on,
per the plan's sequencing note - it drives a real Explorer window and is
the least predictable piece here.

.EXAMPLE
tests/e2e-windows/run.ps1
tests/e2e-windows/run.ps1 -IncludeExplorerScenario
#>
param(
    [string]$SatsumaBin,
    [switch]$SkipBuild,
    [switch]$IncludeExplorerScenario
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "lib.ps1")

if (-not $SatsumaBin) {
    $SatsumaBin = Join-Path $Script:RepoRoot "target\debug\satsuma.exe"
}

# Image formats: the real backend list for a .png source, per
# crates/satsuma-core/src/convert/image.rs's supported_targets("png") ->
# [jpg, webp, tiff, avif, bmp, pdf, docx] (7 entries; png is already
# excluded from its own target list, and every one of those has an
# EXTENSION_DISPLAY entry in src/data/wedgeOptions.ts so all 7 render).
$ImageFormatsMinusPngCount = 7
$JpgIndex = 0 # jpg is still first in that list

# Image tools, from TOOL_OPTIONS in the same file: [compress, crop].
$ImageToolsCount = 2
$CompressIndex = 0

function Start-Satsuma {
    param([string]$Mode, [string]$Path, [string]$Name)
    $stdout = Join-Path $Script:LogDir "$Name.out.log"
    $stderr = Join-Path $Script:LogDir "$Name.err.log"
    return Start-Process -FilePath $SatsumaBin -ArgumentList "--mode=$Mode", "`"$Path`"" -PassThru `
        -RedirectStandardOutput $stdout -RedirectStandardError $stderr
}

function Test-FormatsSelectWritesSiblingFile {
    Write-Log "=== formats-select writes the converted sibling file ==="
    $dir = New-ScratchDir -FixtureName "photo.png"
    $src = Join-Path $dir "photo.png"
    $expectedDest = Join-Path $dir "photo.jpg"
    $proc = $null
    try {
        Set-Cursor -X $CursorX -Y $CursorY
        $proc = Start-Satsuma -Mode "formats" -Path $src -Name "formats-select"

        # 30s (not the 10s the later scenarios use): this is the very first
        # app launch on a cold runner - vite's warm-up is handled in
        # Start-ViteDevServer, but the WebView2/OS side of a first launch
        # (browser-subprocess init, cold page navigation) is still slower
        # than subsequent launches, and a first real CI run flaked right
        # here (overlay never appeared within 10s, same sources, later
        # scenarios fine).
        if (-not (Wait-ForOverlayWindow -ProcessId $proc.Id -TimeoutSeconds 30)) {
            Write-Failure "formats-select: overlay window never appeared"
            Show-SatsumaLogs -Name "formats-select"
            return
        }
        $overlayHandle = Get-OverlayWindowHandle -ProcessId $proc.Id
        Set-WindowFocus -WindowHandle $overlayHandle

        if (-not (Send-WedgeClickUntilHidden -ProcessId $proc.Id -Index $JpgIndex -Count $ImageFormatsMinusPngCount -TimeoutSeconds 8)) {
            Write-Failure "formats-select: overlay did not hide after clicking a wedge"
            Show-SatsumaLogs -Name "formats-select"
        }

        if (-not (Wait-Until -TimeoutSeconds 5 -Description "converted sibling file appears" -Condition { Test-Path $expectedDest })) {
            Write-Failure "formats-select: $expectedDest was never written"
        } else {
            # Phase 2's real conversion re-encodes the source (see
            # crates/satsuma-core/src/convert/image.rs) rather than
            # copying its bytes, so the output is expected to differ
            # byte-for-byte from the source - check it's a genuine JPEG
            # (the SOI marker, 0xFFD8) instead of comparing it to $src.
            $destBytes = [System.IO.File]::ReadAllBytes($expectedDest)
            $isValidJpeg = ($destBytes.Length -ge 2) -and ($destBytes[0] -eq 0xFF) -and ($destBytes[1] -eq 0xD8)
            if (-not $isValidJpeg) {
                Write-Failure "formats-select: $expectedDest was written but isn't a valid JPEG"
            } else {
                Write-Log "PASS: formats-select wrote a real re-encoded JPEG at $expectedDest"
            }
        }
    } finally {
        Stop-ProcessQuietly -Process $proc
        Remove-Item -Recurse -Force $dir -ErrorAction SilentlyContinue
    }
}

function Test-EscapeCancelsWithoutSideEffect {
    Write-Log "=== escape cancels without any side effect ==="
    $dir = New-ScratchDir -FixtureName "photo.png"
    $src = Join-Path $dir "photo.png"
    $unexpectedDest = Join-Path $dir "photo.jpg"
    $proc = $null
    try {
        Set-Cursor -X $CursorX -Y $CursorY
        $proc = Start-Satsuma -Mode "formats" -Path $src -Name "escape-cancel"

        if (-not (Wait-ForOverlayWindow -ProcessId $proc.Id -TimeoutSeconds 10)) {
            Write-Failure "escape-cancel: overlay window never appeared"
            Show-SatsumaLogs -Name "escape-cancel"
            return
        }
        Set-WindowFocus -WindowHandle (Get-OverlayWindowHandle -ProcessId $proc.Id)

        if (-not (Send-EscapeUntilHidden -ProcessId $proc.Id -TimeoutSeconds 8)) {
            Write-Failure "escape-cancel: overlay did not hide after Escape"
            Show-SatsumaLogs -Name "escape-cancel"
        }

        if (Test-Path $unexpectedDest) {
            Write-Failure "escape-cancel: $unexpectedDest was written despite cancelling"
        } else {
            Write-Log "PASS: escape-cancel wrote no file"
        }
    } finally {
        Stop-ProcessQuietly -Process $proc
        Remove-Item -Recurse -Force $dir -ErrorAction SilentlyContinue
    }
}

function Test-ToolsModeOpensAndDismisses {
    Write-Log "=== tools-mode opens and dismisses cleanly ==="
    $dir = New-ScratchDir -FixtureName "photo.png"
    $src = Join-Path $dir "photo.png"
    $proc = $null
    try {
        Set-Cursor -X $CursorX -Y $CursorY
        $proc = Start-Satsuma -Mode "tools" -Path $src -Name "tools-mode"

        if (-not (Wait-ForOverlayWindow -ProcessId $proc.Id -TimeoutSeconds 10)) {
            Write-Failure "tools-mode: overlay window never appeared"
            Show-SatsumaLogs -Name "tools-mode"
            return
        }
        $overlayHandle = Get-OverlayWindowHandle -ProcessId $proc.Id
        Set-WindowFocus -WindowHandle $overlayHandle

        if (-not (Send-WedgeClickUntilHidden -ProcessId $proc.Id -Index $CompressIndex -Count $ImageToolsCount -TimeoutSeconds 8)) {
            Write-Failure "tools-mode: overlay did not hide after clicking a tools wedge"
            Show-SatsumaLogs -Name "tools-mode"
        } else {
            Write-Log "PASS: tools-mode opened and dismissed cleanly"
        }
    } finally {
        Stop-ProcessQuietly -Process $proc
        Remove-Item -Recurse -Force $dir -ErrorAction SilentlyContinue
    }
}

function Test-SingleInstanceForwarding {
    Write-Log "=== single-instance forwarding ==="
    $dir = New-ScratchDir -FixtureName "photo.png"
    $src = Join-Path $dir "photo.png"
    $expectedDest = Join-Path $dir "photo.jpg"
    $firstProc = $null
    $secondProc = $null
    try {
        # First launch: tray-only resident (the `--autostart` shape Satsuma
        # actually uses at login) with no overlay to show yet. Deliberately
        # NOT a plain no-arg launch: a no-arg launch also shows the main
        # window, whose first-ever WebView2 load on a cold Windows runner
        # killed the first process ~1s after start in a real CI run (see
        # single_instance_guard.rs) - leaving nobody to receive the second
        # invocation. `--autostart` skips that main-window show entirely,
        # matching the production resident. (e2e-linux still covers the
        # plain no-arg launch path.)
        $firstStdout = Join-Path $Script:LogDir "single-instance-first.out.log"
        $firstStderr = Join-Path $Script:LogDir "single-instance-first.err.log"
        $firstProc = Start-Process -FilePath $SatsumaBin -ArgumentList "--autostart" -PassThru `
            -RedirectStandardOutput $firstStdout -RedirectStandardError $firstStderr
        if (-not (Wait-Until -TimeoutSeconds 5 -Description "first instance is running" -Condition { -not $firstProc.HasExited })) {
            Write-Failure "single-instance: first instance never started"
            Show-SatsumaLogs -Name "single-instance-first"
            return
        }
        # tauri-plugin-single-instance needs a moment to register itself as
        # the instance owner before a second launch can be forwarded to it.
        Start-Sleep -Seconds 1

        Set-Cursor -X $CursorX -Y $CursorY
        $secondProc = Start-Satsuma -Mode "formats" -Path $src -Name "single-instance-second"

        # 8s (not the 5s used elsewhere): a healthy forward only needs the
        # plugin's quick WM_COPYDATA handshake, but the new Windows-side
        # single-instance guard (src-tauri/src/single_instance_guard.rs)
        # lets a secondary wait up to 5s for the primary's IPC window to
        # appear before proceeding into the bootstrap - so a genuinely
        # forwarded second process can legitimately take longer than 5s to
        # exit without anything being wrong.
        if (-not (Wait-Until -TimeoutSeconds 8 -Description "second invocation exits (forwarded, not a second window)" -Condition { $secondProc.HasExited })) {
            Write-Failure "single-instance: second process did not exit quickly - was it actually forwarded?"
            Show-SatsumaLogs -Name "single-instance-second"
        }
        Stop-ProcessQuietly -Process $secondProc # idempotent safety net either way

        if ($firstProc.HasExited) {
            Write-Failure "single-instance: original process exited unexpectedly"
            Show-SatsumaLogs -Name "single-instance-first"
            return
        }

        # This scenario's overlay has repeatedly needed more time than the
        # other three (already-stable) scenarios' fresh-process launches:
        # first to become interactive, and separately (a later CI run) to
        # even appear at all - both consistent with this being the
        # resident process's own first-ever overlay display (its webview is
        # created and its first navigation starts at startup even while
        # hidden, but the overlay window itself has never been shown
        # before), not a one-off fluke worth chasing further. 40s (up from
        # 20s): a red CI run timed this out at 20s with a healthy forward
        # (second process exited ~1s, first process alive throughout, logs
        # empty) while an earlier green run on near-identical sources saw
        # the overlay appear ~4s in - WebView2's first-ever-show on a cold
        # runner, not the forwarding logic (see e2e-windows run
        # 35474802926 vs. 35473799295), so give it the same generous budget
        # e2e-linux gets for parity.
        if (-not (Wait-ForOverlayWindow -ProcessId $firstProc.Id -TimeoutSeconds 40)) {
            Write-Failure "single-instance: overlay never appeared in the original (first) process"
            # Distinguish "overlay exists but never became visible" from
            # "no overlay window was ever created" by dumping every
            # top-level window of the first process (visible and hidden) -
            # a visible-only search can't tell those apart. Also dump BOTH
            # processes' logs: the second is empty on a healthy forward, so
            # if the forward itself silently failed to reach the first
            # process, that is where it shows up.
            Show-AllWindowsForProcess -ProcessId $firstProc.Id
            Show-SatsumaLogs -Name "single-instance-first"
            Show-SatsumaLogs -Name "single-instance-second"
        } else {
            $overlayHandle = Get-OverlayWindowHandle -ProcessId $firstProc.Id
            Set-WindowFocus -WindowHandle $overlayHandle
            # A first real CI run showed this specific scenario's overlay
            # (this resident process's own webview, never shown before
            # this point) can take noticeably longer to become interactive
            # than a fresh process's - a more generous retry budget than
            # the other, already-proven-stable scenarios need.
            if (-not (Send-WedgeClickUntilHidden -ProcessId $firstProc.Id -Index $JpgIndex -Count $ImageFormatsMinusPngCount -TimeoutSeconds 15)) {
                Write-Failure "single-instance: overlay did not hide after clicking"
                Show-SatsumaLogs -Name "single-instance-first"
            }
            if (-not (Wait-Until -TimeoutSeconds 5 -Description "converted sibling file appears" -Condition { Test-Path $expectedDest })) {
                Write-Failure "single-instance: $expectedDest was never written"
            } else {
                Write-Log "PASS: single-instance forwarding delivered the launch request and converted the file"
            }
        }

        if ($firstProc.HasExited) {
            Write-Failure "single-instance: original process died during the scenario"
        }
    } finally {
        Stop-ProcessQuietly -Process $firstProc
        Stop-ProcessQuietly -Process $secondProc
        Remove-Item -Recurse -Force $dir -ErrorAction SilentlyContinue
    }
}

# Windows-only: the actual Explorer/Shift-hook trigger path
# (windows_integration.rs), which direct binary invocation can never
# exercise. Uses the same Shell.Application COM automation the app itself
# uses to read the selection (foreground_explorer_selection), here to open
# the folder and set the selection instead.
function Test-ExplorerShiftHookTrigger {
    Write-Log "=== Explorer + Shift-hook trigger (real windows_integration.rs path) ==="
    $dir = New-ScratchDir -FixtureName "photo.png"
    $src = Join-Path $dir "photo.png"
    $expectedDest = Join-Path $dir "photo.jpg"
    $satsumaProc = $null
    $shellWindow = $null
    try {
        # satsuma.exe must already be running (resident) for the global
        # keyboard hook to be listening at all - see windows_integration.rs's
        # own module doc comment on why the app is tray-resident.
        $satsumaStdout = Join-Path $Script:LogDir "explorer-trigger.out.log"
        $satsumaStderr = Join-Path $Script:LogDir "explorer-trigger.err.log"
        $satsumaProc = Start-Process -FilePath $SatsumaBin -PassThru `
            -RedirectStandardOutput $satsumaStdout -RedirectStandardError $satsumaStderr
        if (-not (Wait-Until -TimeoutSeconds 5 -Description "satsuma is running" -Condition { -not $satsumaProc.HasExited })) {
            Write-Failure "explorer-trigger: satsuma never started"
            Show-SatsumaLogs -Name "explorer-trigger"
            return
        }
        Start-Sleep -Seconds 1 # let the keyboard hook install

        $shellApp = New-Object -ComObject Shell.Application
        $shellApp.Open($dir) | Out-Null

        $found = Wait-Until -TimeoutSeconds 10 -Description "Explorer window opens on the scratch folder" -Condition {
            $shellWindow = $shellApp.Windows() | Where-Object {
                $_.Document.Folder.Self.Path -eq $dir
            } | Select-Object -First 1
            $null -ne $shellWindow
        }
        if (-not $found -or $null -eq $shellWindow) {
            Write-Failure "explorer-trigger: Explorer never opened the scratch folder"
            return
        }

        [SatsumaE2E.Native]::SetForegroundWindow([IntPtr]$shellWindow.HWND) | Out-Null
        Start-Sleep -Milliseconds 300
        Send-SelectAll # only one item exists in the folder

        Set-Cursor -X $CursorX -Y $CursorY
        Press-ShiftDown
        try {
            if (-not (Wait-ForOverlayWindow -ProcessId $satsumaProc.Id -TimeoutSeconds 10)) {
                Write-Failure "explorer-trigger: overlay never appeared after the Shift trigger"
                Show-SatsumaLogs -Name "explorer-trigger"
                return
            }
            $overlayHandle = Get-OverlayWindowHandle -ProcessId $satsumaProc.Id
            Set-WindowFocus -WindowHandle $overlayHandle

            if (-not (Send-WedgeClickUntilHidden -ProcessId $satsumaProc.Id -Index $JpgIndex -Count $ImageFormatsMinusPngCount -TimeoutSeconds 8)) {
                Write-Failure "explorer-trigger: overlay did not hide after clicking"
                Show-SatsumaLogs -Name "explorer-trigger"
            }
            if (-not (Wait-Until -TimeoutSeconds 5 -Description "converted sibling file appears" -Condition { Test-Path $expectedDest })) {
                Write-Failure "explorer-trigger: $expectedDest was never written"
            } else {
                Write-Log "PASS: the real Explorer/Shift-hook trigger opened the overlay and converted the file"
            }
        } finally {
            Press-ShiftUp
        }
    } finally {
        if ($null -ne $shellWindow) { try { $shellWindow.Quit() } catch {} }
        Stop-ProcessQuietly -Process $satsumaProc
        Remove-Item -Recurse -Force $dir -ErrorAction SilentlyContinue
    }
}

# --- orchestration -----------------------------------------------------

$nodeModules = Join-Path $Script:RepoRoot "node_modules"
if (-not (Test-Path $nodeModules)) {
    Write-Host "[e2e-windows] $nodeModules not found - run 'npm ci' first (this harness starts the vite dev server itself, see Start-ViteDevServer in lib.ps1)" -ForegroundColor Red
    exit 1
}

if (-not $SkipBuild) {
    Write-Log "building satsuma (cargo build -p satsuma)"
    Push-Location $Script:RepoRoot
    try {
        cargo build -p satsuma
        if ($LASTEXITCODE -ne 0) {
            Write-Host "[e2e-windows] build failed. If the error is 'resource path ... doesn't exist', run scripts/fetch-ffmpeg.ps1 and scripts/fetch-pdfium.ps1 first (the sidecars are gitignored - see AGENTS.md)" -ForegroundColor Red
            throw "cargo build failed"
        }
    } finally {
        Pop-Location
    }
}

if (-not (Test-Path $SatsumaBin)) {
    Write-Host "[e2e-windows] binary not found at $SatsumaBin" -ForegroundColor Red
    exit 1
}

try {
    Start-ViteDevServer

    Test-FormatsSelectWritesSiblingFile
    Test-EscapeCancelsWithoutSideEffect
    Test-ToolsModeOpensAndDismisses
    Test-SingleInstanceForwarding

    if ($IncludeExplorerScenario) {
        Test-ExplorerShiftHookTrigger
    }
} finally {
    Stop-ViteDevServer
}

Write-Host ""
if ($Script:Failures -eq 0) {
    Write-Log "all scenarios passed"
    exit 0
} else {
    Write-Log "$($Script:Failures) scenario(s) failed"
    exit 1
}
