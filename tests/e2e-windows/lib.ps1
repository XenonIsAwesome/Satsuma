# Shared helpers for tests/e2e-windows/run.ps1. Dot-sourced, not executed
# directly.
#
# windows-latest GitHub-hosted runners already provide a real interactive
# desktop session - unlike the Linux harness, nothing here needs to boot a
# virtual display itself. It launches the built satsuma.exe directly
# (`--mode=formats|tools <path>`), the same entry point a Windows
# Explorer/Shift-hook trigger's downstream effect produces (see
# crates/satsuma-core/src/launch_request.rs and show_overlay in
# src-tauri/src/lib.rs, both OS-agnostic), and puppets the cursor/keyboard
# via a handful of P/Invoke'd user32.dll functions.
#
# CAVEAT: this script itself (unlike windows_integration.rs, which has been
# manually verified in a real Windows VM - see that file's own doc comment)
# has never been run - there was no Windows machine available in the
# environment that wrote it. Treat the first few CI runs as the actual
# debugging/hardening pass for this harness, not just a verification step.

Add-Type -Namespace SatsumaE2E -Name Native -MemberDefinition @"
[DllImport("user32.dll")]
public static extern bool SetCursorPos(int x, int y);

[DllImport("user32.dll")]
public static extern void mouse_event(uint dwFlags, uint dx, uint dy, uint dwData, UIntPtr dwExtraInfo);

[DllImport("user32.dll")]
public static extern void keybd_event(byte bVk, byte bScan, uint dwFlags, UIntPtr dwExtraInfo);

[DllImport("user32.dll")]
public static extern bool SetForegroundWindow(IntPtr hWnd);

[DllImport("user32.dll")]
public static extern bool IsWindowVisible(IntPtr hWnd);

[DllImport("user32.dll")]
public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint lpdwProcessId);

[DllImport("user32.dll")]
public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);

[StructLayout(LayoutKind.Sequential)]
public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }

public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

[DllImport("user32.dll")]
public static extern bool EnumWindows(EnumWindowsProc lpEnumFunc, IntPtr lParam);

[DllImport("user32.dll", CharSet = CharSet.Unicode)]
public static extern int GetWindowText(IntPtr hWnd, System.Text.StringBuilder lpString, int nMaxCount);
"@

# Win32 constants used below (mouse_event/keybd_event flags, VK codes).
Set-Variable -Name MOUSEEVENTF_LEFTDOWN -Value 0x0002 -Option Constant -Scope Script
Set-Variable -Name MOUSEEVENTF_LEFTUP -Value 0x0004 -Option Constant -Scope Script
Set-Variable -Name KEYEVENTF_KEYUP -Value 0x0002 -Option Constant -Scope Script
Set-Variable -Name VK_SHIFT -Value 0x10 -Option Constant -Scope Script
Set-Variable -Name VK_ESCAPE -Value 0x1B -Option Constant -Scope Script
Set-Variable -Name VK_CONTROL -Value 0x11 -Option Constant -Scope Script
Set-Variable -Name VK_A -Value 0x41 -Option Constant -Scope Script

# Same fixed, deliberately-chosen trigger point as the Linux harness -
# comfortably inside a default runner display resolution so a
# LABEL_RADIUS=95 wedge click never clips off-screen. Re-check this against
# the runner's actual resolution/DPI scaling once this first runs for real
# (see the CAVEAT above and the Windows section of the plan).
Set-Variable -Name CursorX -Value 400 -Option Constant -Scope Script
Set-Variable -Name CursorY -Value 400 -Option Constant -Scope Script

$Script:RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
# Shared with tests/e2e-linux - not duplicated per platform.
$Script:FixturesDir = Join-Path $Script:RepoRoot "tests\fixtures"
$Script:LabelRadius = 95.0 # (OUTER_RADIUS 150 + INNER_RADIUS 40) / 2, from WedgeMenu.tsx

# Where each launched satsuma.exe's redirected stdout/stderr lands (see
# Start-Satsuma in run.ps1), so a CI failure can dump the relevant log
# instead of just "the window never appeared" with no clue why.
$Script:LogDir = Join-Path $env:TEMP "satsuma-e2e-windows-logs"
New-Item -ItemType Directory -Path $Script:LogDir -Force | Out-Null

$Script:ViteProcess = $null
$Script:VitePort = 1420 # tauri.conf.json's devUrl / vite.config.ts's server.port

# Start-ViteDevServer
# `cargo build -p satsuma` (what this harness uses, deliberately, to
# mirror the exact Explorer/Shift-hook and direct-invocation launch paths)
# is a plain debug build with no `custom-protocol` feature - there's no
# such feature declared anywhere in src-tauri/Cargo.toml at all. That
# means the webview always loads tauri.conf.json's `devUrl`
# (http://localhost:1420), the same as a real `cargo tauri dev` session -
# `frontendDist`/`dist/` is irrelevant here regardless of whether it's
# been built. Ordinarily `cargo tauri dev` starts this dev server itself
# via `beforeDevCommand`; calling `cargo build` directly (bypassing the
# tauri-cli entirely) skips that, so this harness has to start it
# explicitly - otherwise the webview loads a connection-refused blank page
# and is permanently inert: the native window still opens, focuses, and is
# positioned correctly (all Rust-side, confirmed via this harness's own
# diagnostics), but nothing in it is ever interactive.
function Start-ViteDevServer {
    Write-Log "starting vite dev server (npm run dev) on port $Script:VitePort"
    $stdout = Join-Path $Script:LogDir "vite.out.log"
    $stderr = Join-Path $Script:LogDir "vite.err.log"
    $Script:ViteProcess = Start-Process -FilePath "npm.cmd" -ArgumentList "run", "dev" -WorkingDirectory $Script:RepoRoot -PassThru `
        -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    # "localhost" (not the literal 127.0.0.1) to match what tauri.conf.json's
    # devUrl and vite.config.ts's unset `server.host` both actually resolve
    # to - on a host where "localhost" prefers IPv6, vite/Node bind only
    # ::1, and a literal-127.0.0.1 probe reports connection-refused (and
    # times out this whole wait) even while the server is genuinely up and
    # ready, as a first real CI run showed (identically on the Linux side).
    $ready = Wait-Until -TimeoutSeconds 30 -Description "vite dev server ready on port $Script:VitePort" -Condition {
        try {
            $client = New-Object System.Net.Sockets.TcpClient
            $client.Connect("localhost", $Script:VitePort)
            $client.Close()
            return $true
        } catch {
            return $false
        }
    }
    if (-not $ready) {
        Show-ProcessLogTail -LogPath $stdout
        Show-ProcessLogTail -LogPath $stderr
        throw "vite dev server never became ready"
    }
    # Best-effort warm-up: the TCP check above only proves the listener is
    # up, but vite prebundles deps and transforms modules on the FIRST http
    # request (cold, can take 20-60s on a fresh runner), and the app's very
    # first webview navigation is exactly that request - a first real CI run
    # flaked here (formats-select: overlay window never appeared within 10s,
    # same sources, later scenarios fine) while the webview sat on vite's
    # cold prebundle. Force that first request to complete now, before the
    # app ever asks, so the webview's first navigation is warm. Any HTTP
    # response (even an error status) counts as "vite processed the
    # request"; connection-refused/timeouts mean it's still starting.
    # Best-effort on purpose: if this never completes, the app's waits below
    # still apply and will surface any real regression.
    $warmed = Wait-Until -TimeoutSeconds 60 -Description "vite first-request warm-up" -Condition {
        try {
            $null = Invoke-WebRequest -Uri "http://localhost:$Script:VitePort/" -UseBasicParsing -SkipHttpErrorCheck -TimeoutSec 5
            return $true
        } catch {
            return $false
        }
    }
    if (-not $warmed) {
        Write-Log "warning: vite first-request warm-up never completed - continuing anyway"
    }
}

function Stop-ViteDevServer {
    if ($null -ne $Script:ViteProcess -and -not $Script:ViteProcess.HasExited) {
        # npm.cmd spawns node.exe as a child rather than becoming it, and
        # Windows doesn't kill child processes when a parent is killed -
        # taskkill's /T (tree) is needed so the real vite server (node.exe)
        # doesn't linger and hold port 1420 for a subsequent local run.
        try { taskkill /PID $Script:ViteProcess.Id /T /F 2>$null | Out-Null } catch {}
    }
    $Script:ViteProcess = $null
}

function Write-Log {
    param([string]$Message)
    Write-Host "[e2e-windows] $Message"
}

$Script:Failures = 0

function Write-Failure {
    param([string]$Message)
    Write-Host "[e2e-windows] FAIL: $Message" -ForegroundColor Red
    $Script:Failures++
}

# Get-WedgePoint <centerX> <centerY> <index> <count>
# Mirrors tests/e2e-linux/wedge_point.py / src/lib/wedgeGeometry.ts - see
# that file's doc comment for the full derivation. Returns @{X=...; Y=...}
# relative to the given center - the overlay window's own actual on-screen
# center (see Get-OverlayWindowCenter), not assumed from the cursor
# position we chose before launching: a first real CI run showed Mutter's
# Windows counterpart (window placement/DPI handling) can't be trusted to
# center the window exactly where we expect either.
function Get-WedgePoint {
    param([int]$CenterX, [int]$CenterY, [int]$Index, [int]$Count)
    if ($Count -le 0) { throw "Count must be positive" }
    $angleMidDeg = ($Index + 0.5) * 360.0 / $Count
    $angleRad = ($angleMidDeg - 90) * [Math]::PI / 180
    $x = $CenterX + $Script:LabelRadius * [Math]::Cos($angleRad)
    $y = $CenterY + $Script:LabelRadius * [Math]::Sin($angleRad)
    return @{ X = [int][Math]::Round($x); Y = [int][Math]::Round($y) }
}

# Get-OverlayWindowCenter <hWnd>
# Queries the window's actual on-screen geometry and returns its center -
# the ground truth basis for click math, instead of assuming the window
# ended up centered wherever we placed the cursor before launching.
function Get-OverlayWindowCenter {
    param([IntPtr]$WindowHandle)
    $rect = New-Object SatsumaE2E.Native+RECT
    if (-not [SatsumaE2E.Native]::GetWindowRect($WindowHandle, [ref]$rect)) {
        throw "GetWindowRect failed for window handle $WindowHandle"
    }
    return @{
        X = [int](($rect.Left + $rect.Right) / 2)
        Y = [int](($rect.Top + $rect.Bottom) / 2)
    }
}

function Set-Cursor {
    param([int]$X, [int]$Y)
    [SatsumaE2E.Native]::SetCursorPos($X, $Y) | Out-Null
}

function Invoke-LeftClick {
    param([int]$X, [int]$Y)
    Set-Cursor -X $X -Y $Y
    [SatsumaE2E.Native]::mouse_event($MOUSEEVENTF_LEFTDOWN, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 50
    [SatsumaE2E.Native]::mouse_event($MOUSEEVENTF_LEFTUP, 0, 0, 0, [UIntPtr]::Zero)
}

function Send-WedgeClick {
    param([IntPtr]$WindowHandle, [int]$Index, [int]$Count)
    $center = Get-OverlayWindowCenter -WindowHandle $WindowHandle
    $point = Get-WedgePoint -CenterX $center.X -CenterY $center.Y -Index $Index -Count $Count
    Invoke-LeftClick -X $point.X -Y $point.Y
}

function Send-EscapeKey {
    [SatsumaE2E.Native]::keybd_event($VK_ESCAPE, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 50
    [SatsumaE2E.Native]::keybd_event($VK_ESCAPE, 0, $KEYEVENTF_KEYUP, [UIntPtr]::Zero)
}

function Press-ShiftDown {
    [SatsumaE2E.Native]::keybd_event($VK_SHIFT, 0, 0, [UIntPtr]::Zero)
}

function Press-ShiftUp {
    [SatsumaE2E.Native]::keybd_event($VK_SHIFT, 0, $KEYEVENTF_KEYUP, [UIntPtr]::Zero)
}

function Send-SelectAll {
    [SatsumaE2E.Native]::keybd_event($VK_CONTROL, 0, 0, [UIntPtr]::Zero)
    [SatsumaE2E.Native]::keybd_event($VK_A, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 50
    [SatsumaE2E.Native]::keybd_event($VK_A, 0, $KEYEVENTF_KEYUP, [UIntPtr]::Zero)
    [SatsumaE2E.Native]::keybd_event($VK_CONTROL, 0, $KEYEVENTF_KEYUP, [UIntPtr]::Zero)
}

# New-ScratchDir <fixtureName>
# Copies one named fixture into a fresh temp dir and returns that dir's
# path, so each scenario gets its own isolated input file (and destination
# for convert_file's sibling-file write).
function New-ScratchDir {
    param([string]$FixtureName)
    $dir = Join-Path $env:TEMP ("satsuma-e2e-windows-" + [guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Path $dir | Out-Null
    Copy-Item (Join-Path $Script:FixturesDir $FixtureName) (Join-Path $dir $FixtureName)
    return $dir
}

# Get-WindowHandlesForProcess <processId> [-AllWindows]
# Enumerates all top-level windows owned by the given process id. By
# default only the visible ones are returned (the overlay is the only
# window a direct `--mode=...` launch or Shift-hook trigger should ever
# show); with -AllWindows every top-level window is returned, visible or
# not, so a failure can tell "the overlay window exists but never became
# visible" apart from "the overlay window was never created at all" (see
# Show-AllWindowsForProcess).
function Get-WindowHandlesForProcess {
    param([int]$ProcessId, [switch]$AllWindows)
    $handles = New-Object System.Collections.Generic.List[IntPtr]
    # Explicit delegate cast (not just a bare scriptblock) - EnumWindows'
    # p/invoke signature expects the nested SatsumaE2E.Native+EnumWindowsProc
    # delegate type, and this is the standard, reliable idiom for handing a
    # scriptblock to a native callback from PowerShell.
    $callback = [SatsumaE2E.Native+EnumWindowsProc]{
        param([IntPtr]$hWnd, [IntPtr]$lParam)
        $ownerPid = 0
        [SatsumaE2E.Native]::GetWindowThreadProcessId($hWnd, [ref]$ownerPid) | Out-Null
        if ($ownerPid -eq $ProcessId -and ($AllWindows -or [SatsumaE2E.Native]::IsWindowVisible($hWnd))) {
            $handles.Add($hWnd)
        }
        return $true
    }
    [SatsumaE2E.Native]::EnumWindows($callback, [IntPtr]::Zero) | Out-Null
    return $handles
}

# Get-VisibleWindowHandlesForProcess <processId>
# Visible-window subset of Get-WindowHandlesForProcess - the overlay is
# the only window a direct `--mode=...` launch (or a Shift-hook trigger)
# ever shows (the main window starts hidden and stays that way for the
# tray-resident design, see tauri.conf.json's "visible": false), but a
# first real CI run turned up other transient/unexpected visible windows
# too (see Get-OverlayWindowHandle, the actual overlay-identifying function
# built on top of this one).
function Get-VisibleWindowHandlesForProcess {
    param([int]$ProcessId)
    return Get-WindowHandlesForProcess -ProcessId $ProcessId
}

# Show-AllWindowsForProcess <processId>
# Dumps every top-level window owned by the process - visible AND hidden -
# with its title, visibility, and screen rect. A visible-window-only search
# (Get-OverlayWindowHandle) cannot distinguish "the overlay window exists
# but never became visible" from "the overlay window was never created at
# all": both look identical from its point of view. An overlay-show timeout
# dumps this to tell them apart.
function Show-AllWindowsForProcess {
    param([int]$ProcessId)
    Write-Log "--- all top-level windows for pid $ProcessId (visible and hidden) ---"
    foreach ($handle in (Get-WindowHandlesForProcess -ProcessId $ProcessId -AllWindows)) {
        $title = ""
        $titleBuf = New-Object System.Text.StringBuilder 512
        if ([SatsumaE2E.Native]::GetWindowText($handle, $titleBuf, $titleBuf.Capacity) -gt 0) {
            $title = " title='$($titleBuf.ToString())'"
        }
        $rect = New-Object SatsumaE2E.Native+RECT
        $rectText = "rect=unavailable"
        if ([SatsumaE2E.Native]::GetWindowRect($handle, [ref]$rect)) {
            $rectText = "rect=left:$($rect.Left),top:$($rect.Top),right:$($rect.Right),bottom:$($rect.Bottom) ($($rect.Right - $rect.Left)x$($rect.Bottom - $rect.Top))"
        }
        $visibility = if ([SatsumaE2E.Native]::IsWindowVisible($handle)) { "visible" } else { "hidden" }
        Write-Log "  handle=$handle $visibility $rectText$title"
    }
    Write-Log "--- end of window list ---"
}

# Wait-Until <timeoutSeconds> <description> <scriptblock>
# Polls the scriptblock every 200ms until it returns a truthy value, or
# fails after timeoutSeconds. Bounded like the Linux harness's wait_until,
# so a stuck focus/activation regression surfaces as a specific failure
# rather than an indefinite hang.
function Wait-Until {
    param(
        [double]$TimeoutSeconds,
        [string]$Description,
        [scriptblock]$Condition
    )
    $waited = 0.0
    while (-not (& $Condition)) {
        Start-Sleep -Milliseconds 200
        $waited += 0.2
        if ($waited -gt $TimeoutSeconds) {
            Write-Log "timed out waiting for: $Description"
            return $false
        }
    }
    return $true
}

# Get-OverlayWindowHandle <processId>
# The process can legitimately own more than one top-level window at once
# - a first real CI run turned up both a tiny (16x16) helper window and,
# in the single-instance scenario, the "main" window (800x600 plus Windows'
# own chrome, ~816x639) becoming visible alongside - or instead of - the
# actual borderless overlay. Rather than trust "first visible window",
# filter by size: the overlay is a fixed 480x480 (OVERLAY_SIZE in
# src-tauri/src/lib.rs / tauri.conf.json), so this looks for a visible
# window in that ballpark (some slack for DPI scaling) and ignores
# anything much smaller or much larger. Returns $null if none matches.
function Get-OverlayWindowHandle {
    param([int]$ProcessId)
    foreach ($handle in (Get-VisibleWindowHandlesForProcess -ProcessId $ProcessId)) {
        $rect = New-Object SatsumaE2E.Native+RECT
        if (-not [SatsumaE2E.Native]::GetWindowRect($handle, [ref]$rect)) { continue }
        $width = $rect.Right - $rect.Left
        $height = $rect.Bottom - $rect.Top
        if ($width -ge 300 -and $width -le 700 -and $height -ge 300 -and $height -le 700) {
            return $handle
        }
    }
    return $null
}

# Wait-ForOverlayWindow <processId> <timeoutSeconds>
# The native window becoming visible and the webview's page actually being
# interactive are two different events: OverlayApp only renders the wedge
# menu once its own useLaunchRequest hook resolves an async
# `invoke("take_launch_request")` IPC round trip, racing the webview's very
# first page load/bundle-execution/React-mount for a fresh process launch
# (the direct-invocation path this harness always uses) - not just a quick
# re-render. A first real CI run showed the window being found (right
# size, right position, confirmed via the rect this function logs) and
# clicks/Escape still doing nothing at all afterward - not even the wedge
# menu's own click-outside-to-cancel backdrop responded - consistent with
# interacting before any handler was attached yet, not a targeting
# problem. The settle delay below gives that startup sequence room to
# finish before the harness ever touches the window; on a real user's warm
# desktop this window-to-interactive gap is normally imperceptible, but a
# cold CI runner's first launch can plausibly take longer.
function Wait-ForOverlayWindow {
    param([int]$ProcessId, [double]$TimeoutSeconds)
    if (-not (Wait-Until -TimeoutSeconds $TimeoutSeconds -Description "overlay window visible" -Condition {
        $null -ne (Get-OverlayWindowHandle -ProcessId $ProcessId)
    })) {
        return $false
    }
    Start-Sleep -Seconds 1.5
    return $true
}

# Send-WedgeClickUntilHidden <processId> <index> <count> <timeoutSeconds>
# Repeats Send-WedgeClick every 500ms until the overlay hides or
# timeoutSeconds elapses, instead of trusting a single click at some
# assumed-ready moment. Safe to repeat: a formats-mode click just re-runs
# convert_file, collision-safe named (see naming::output_path), and hide_overlay is a no-op once
# already hidden. Hedges against not knowing exactly how long the webview
# takes to become interactive after its window is mapped (see
# Wait-ForOverlayWindow's doc comment) without having to guess a single
# "long enough" number. Re-resolves the window handle every attempt rather
# than reusing one from before the click, in case it ever changes.
function Send-WedgeClickUntilHidden {
    param([int]$ProcessId, [int]$Index, [int]$Count, [double]$TimeoutSeconds)
    $waited = 0.0
    while ($true) {
        $handle = Get-OverlayWindowHandle -ProcessId $ProcessId
        if ($null -eq $handle) { return $true }
        Send-WedgeClick -WindowHandle $handle -Index $Index -Count $Count
        if ($null -eq (Get-OverlayWindowHandle -ProcessId $ProcessId)) { return $true }
        Start-Sleep -Milliseconds 500
        $waited += 0.5
        if ($waited -gt $TimeoutSeconds) { return $false }
    }
}

# Send-EscapeUntilHidden <processId> <timeoutSeconds>
# Same idea as Send-WedgeClickUntilHidden, for the Escape-cancel path.
function Send-EscapeUntilHidden {
    param([int]$ProcessId, [double]$TimeoutSeconds)
    $waited = 0.0
    while ($true) {
        Send-EscapeKey
        if ($null -eq (Get-OverlayWindowHandle -ProcessId $ProcessId)) { return $true }
        Start-Sleep -Milliseconds 500
        $waited += 0.5
        if ($waited -gt $TimeoutSeconds) { return $false }
    }
}

# Set-WindowFocus <hWnd>
# Brings the overlay window to the foreground before puppeting it.
# keybd_event's synthesized key events (Send-EscapeKey, Press-ShiftDown/Up)
# go to whichever window currently holds keyboard focus, not wherever the
# cursor happens to be - skipping this could silently deliver
# Escape/Shift/Ctrl+A to some other window (e.g. the console host running
# this very script) instead of the overlay. Also logs the window's screen
# rect purely for diagnostics - Send-WedgeClick queries this same rect
# itself (via Get-OverlayWindowCenter) to compute where to click, so it
# isn't relying on the trigger cursor position matching it.
function Set-WindowFocus {
    param([IntPtr]$WindowHandle)
    [SatsumaE2E.Native]::SetForegroundWindow($WindowHandle) | Out-Null
    Start-Sleep -Milliseconds 200
    $rect = New-Object SatsumaE2E.Native+RECT
    if ([SatsumaE2E.Native]::GetWindowRect($WindowHandle, [ref]$rect)) {
        Write-Log "overlay window rect: left=$($rect.Left) top=$($rect.Top) right=$($rect.Right) bottom=$($rect.Bottom) (trigger cursor was $CursorX,$CursorY)"
    }
}

function Stop-ProcessQuietly {
    param([System.Diagnostics.Process]$Process)
    if ($null -ne $Process -and -not $Process.HasExited) {
        try { $Process.Kill() } catch {}
        try { $Process.WaitForExit(5000) | Out-Null } catch {}
    }
}

# Show-ProcessLogTail <logPath>
# Prints the tail of a satsuma process's redirected stdout/stderr so a CI
# failure is self-diagnosing instead of just "the window never appeared"
# with no clue why.
function Show-ProcessLogTail {
    param([string]$LogPath)
    if (Test-Path $LogPath) {
        Write-Log "--- tail of $LogPath ---"
        Get-Content $LogPath -Tail 60 | ForEach-Object { Write-Host $_ }
        Write-Log "--- end of log ---"
    }
}

# Show-SatsumaLogs <name>
# Dumps both halves of a Start-Satsuma-launched process's redirected
# output (see run.ps1), named the same way Start-Satsuma built them.
function Show-SatsumaLogs {
    param([string]$Name)
    Show-ProcessLogTail -LogPath (Join-Path $Script:LogDir "$Name.out.log")
    Show-ProcessLogTail -LogPath (Join-Path $Script:LogDir "$Name.err.log")
}
