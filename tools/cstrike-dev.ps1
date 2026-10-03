param(
    [ValidateSet('launch','key','mouse','capture','stop')][string]$Action = 'launch',
    [string]$Binary = 'target/debug/mashup-cstrike.exe',
    [int]$Key = 87,
    [ValidateSet('left','right')][string]$Button = 'left',
    [ValidateRange(40,5000)][int]$DurationMs = 150,
    [switch]$CaptureWhileHeld
)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
$outputDirectory = Join-Path $workspace 'user_data/cstrike'
$sessionPath = Join-Path $outputDirectory 'session.json'

Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class CstrikeDevWindow {
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr window, uint message, IntPtr key, IntPtr data);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr window);
 [DllImport("user32.dll")] public static extern uint MapVirtualKey(uint key, uint type);
 public static void Key(IntPtr window, int key, bool down) {
   long bits=1L | ((long)MapVirtualKey((uint)key,0)<<16);
   if (!down) bits|=0xc0000000L;
   if (!PostMessage(window,down?0x100U:0x101U,(IntPtr)key,(IntPtr)bits))
     throw new InvalidOperationException("Unable to post key to CS window.");
 }
}
'@

function Get-RecordedCstrike {
    if (!(Test-Path -LiteralPath $sessionPath)) { throw 'Launch the CS dev session first.' }
    $session = Get-Content -LiteralPath $sessionPath -Raw | ConvertFrom-Json
    $process = Get-Process -Id $session.pid -ErrorAction Stop
    $started = ([DateTimeOffset]$session.started).UtcDateTime
    if ($process.ProcessName -ne 'mashup-cstrike' -or
        [Math]::Abs(($process.StartTime.ToUniversalTime()-$started).TotalSeconds) -gt 1 -or
        ![string]::Equals($process.Path, $session.binary, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Recorded PID no longer identifies this CS executable/session.'
    }
    return $process
}

if ($Action -eq 'launch') {
    if (Test-Path -LiteralPath $sessionPath) {
        $previous = Get-Content -LiteralPath $sessionPath -Raw | ConvertFrom-Json
        $running = Get-Process -Id $previous.pid -ErrorAction SilentlyContinue
        if ($running -and $running.ProcessName -eq 'mashup-cstrike' -and
            [Math]::Abs(($running.StartTime.ToUniversalTime()-([DateTimeOffset]$previous.started).UtcDateTime).TotalSeconds) -lt 1) {
            throw 'This CS session is already running; stop it before launching another.'
        }
    }
    $binaryPath = [IO.Path]::GetFullPath((Join-Path $workspace $Binary))
    if ([IO.Path]::GetFileName($binaryPath) -ne 'mashup-cstrike.exe' -or
        !$binaryPath.StartsWith($workspace + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Use this worktree''s dedicated mashup-cstrike.exe.'
    }
    if (!(Test-Path -LiteralPath $binaryPath -PathType Leaf)) { throw 'Build mashup-cstrike in dev mode first.' }
    New-Item -ItemType Directory -Force $outputDirectory | Out-Null
    $env:WGPU_BACKEND = 'dx12'
    # The user requested a visible, playable game window.
    $process = Start-Process -FilePath $binaryPath -WorkingDirectory $workspace -WindowStyle Normal -PassThru -RedirectStandardOutput (Join-Path $outputDirectory 'stdout.log') -RedirectStandardError (Join-Path $outputDirectory 'stderr.log')
    @{pid=$process.Id; binary=$binaryPath; started=$process.StartTime.ToUniversalTime().ToString('o')} |
        ConvertTo-Json | Set-Content -LiteralPath $sessionPath
    Write-Output "Started CS dev process $($process.Id): $binaryPath"
    exit
}

$process = Get-RecordedCstrike
if ($Action -eq 'stop') {
    if (!$process.CloseMainWindow()) { throw 'CS window is not ready to close.' }
    exit
}
$window = $process.MainWindowHandle
if ($window -eq 0) { throw 'CS window is not ready.' }
[CstrikeDevWindow]::SetForegroundWindow($window) | Out-Null
Start-Sleep -Milliseconds 100
function Request-HeldCapture {
    if ($CaptureWhileHeld) {
        [CstrikeDevWindow]::Key($window,123,$true)
        Start-Sleep -Milliseconds 80
        [CstrikeDevWindow]::Key($window,123,$false)
    }
}
if ($Action -eq 'mouse') {
    $down = if ($Button -eq 'left') { 0x201 } else { 0x204 }
    $up = if ($Button -eq 'left') { 0x202 } else { 0x205 }
    $flag = if ($Button -eq 'left') { 1 } else { 2 }
    [CstrikeDevWindow]::PostMessage($window,$down,[IntPtr]$flag,[IntPtr]::Zero) | Out-Null
    Start-Sleep -Milliseconds $DurationMs
    Request-HeldCapture
    [CstrikeDevWindow]::PostMessage($window,$up,[IntPtr]::Zero,[IntPtr]::Zero) | Out-Null
} else {
    $pressedKey = if ($Action -eq 'capture') { 123 } else { $Key }
    [CstrikeDevWindow]::Key($window,$pressedKey,$true)
    Start-Sleep -Milliseconds $DurationMs
    if ($Action -eq 'key') { Request-HeldCapture }
    [CstrikeDevWindow]::Key($window,$pressedKey,$false)
    if ($Action -eq 'capture') {
        Write-Output "Requested native F12 capture under $outputDirectory/screenshots (no desktop capture)."
    }
}
