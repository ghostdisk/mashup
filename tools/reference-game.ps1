param(
    [ValidateSet('launch','command','key','capture','stop')][string]$Action = 'launch',
    [string]$Install = 'C:\Program Files (x86)\Steam\steamapps\common\Half-Life',
    [string]$Command,
    [string]$Output = 'user_data/reference/reference.png',
    [switch]$ConsoleOpen,
    [int]$Key = 87,
    [ValidateRange(1,5000)][int]$DurationMs = 250
)
$ErrorActionPreference = 'Stop'
# Local developer tooling only. Never attaches to an online game.
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class ReferenceWindow {
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr SendMessage(IntPtr window, uint message, IntPtr key, IntPtr data);
 [DllImport("user32.dll")] public static extern uint MapVirtualKey(uint code, uint type);
 public static void Key(IntPtr window, int key, bool down) {
   long bits=1L | ((long)MapVirtualKey((uint)key,0)<<16);
   if (!down) bits|=0xc0000000L;
   SendMessage(window,down?0x100U:0x101U,(IntPtr)key,(IntPtr)bits);
 }
}
'@
if ($Action -eq 'launch') {
    if (Get-Process hl -ErrorAction SilentlyContinue) { throw 'Half-Life is already running; use that window or close it first.' }
    $config = Join-Path $Install 'cstrike/mashup_reference.cfg'
    $createdConfiguration=!(Test-Path -LiteralPath $config)
    if ($createdConfiguration) {
    @'
sv_lan 1
sv_cheats 0
mp_freezetime 0
mp_roundtime 9
mp_startmoney 16000
mp_autoteambalance 0
mp_limitteams 0
fps_max 100
cl_showfps 1
net_graph 3
developer 1
'@ | Set-Content -LiteralPath $config -Encoding ascii
    }
    # The requested interactive game window must be visible for capture/control.
    $env:SteamAppId = '10'
    $process = Start-Process -FilePath (Join-Path $Install 'hl.exe') -WorkingDirectory $Install -WindowStyle Normal -ArgumentList '-game cstrike -windowed -w 1280 -h 720 -console -condebug -insecure -nomaster +sv_lan 1 +maxplayers 1 +map de_dust2 +exec mashup_reference.cfg' -PassThru
    New-Item -ItemType Directory -Force user_data/reference | Out-Null
    @{pid=$process.Id; installation=$Install; map='de_dust2'; lan=$true; created_configuration=$createdConfiguration; started=$process.StartTime.ToUniversalTime().ToString('o')} | ConvertTo-Json | Set-Content user_data/reference/session.json
    Get-FileHash (Join-Path $Install 'hl.exe'), (Join-Path $Install 'hw.dll'), (Join-Path $Install 'cstrike/dlls/mp.dll'), (Join-Path $Install 'cstrike/cl_dlls/client.dll') | ConvertTo-Json | Set-Content user_data/reference/binaries.json
    Write-Output "Started offline reference process $($process.Id)."
    exit
}
$session = Get-Content user_data/reference/session.json -Raw | ConvertFrom-Json
$process = Get-Process -Id $session.pid
if ($process.ProcessName -ne 'hl' -or [Math]::Abs(($process.StartTime.ToUniversalTime()-[DateTime]::Parse($session.started).ToUniversalTime()).TotalSeconds) -gt 3) { throw 'Recorded PID no longer belongs to this reference session.' }
if ($Action -eq 'stop') {
    $process.CloseMainWindow() | Out-Null
    if ($session.created_configuration) { Remove-Item -LiteralPath (Join-Path $session.installation 'cstrike/mashup_reference.cfg') }
    exit
}
$window = $process.MainWindowHandle
if ($window -eq 0) { throw 'Reference window is not ready.' }
if ($Action -eq 'key') {
    [ReferenceWindow]::Key($window,$Key,$true)
    Start-Sleep -Milliseconds $DurationMs
    [ReferenceWindow]::Key($window,$Key,$false)
} elseif ($Action -eq 'command') {
    if (!$Command) { throw 'Supply -Command with a console command; open the console first using toggleconsole.' }
    if ($Command -eq 'toggleconsole') {
        [ReferenceWindow]::Key($window,192,$true)
        Start-Sleep -Milliseconds 120
        [ReferenceWindow]::Key($window,192,$false)
        Start-Sleep -Milliseconds 150
    } else {
        foreach ($letter in $Command.ToCharArray()) { [ReferenceWindow]::SendMessage($window, 0x102, [IntPtr][int]$letter, [IntPtr]::Zero) | Out-Null }
        [ReferenceWindow]::Key($window,13,$true)
        Start-Sleep -Milliseconds 120
        [ReferenceWindow]::Key($window,13,$false)
    }
} else {
    # GoldSrc renders its own BMP. Never capture desktop screen coordinates.
    $gameDirectory = Join-Path $session.installation 'cstrike'
    $before = @(Get-ChildItem -LiteralPath $gameDirectory -Filter "$($session.map)*.bmp" | Select-Object -ExpandProperty FullName)
    if (!$ConsoleOpen) {
        [ReferenceWindow]::Key($window,192,$true)
        Start-Sleep -Milliseconds 100
        [ReferenceWindow]::Key($window,192,$false)
        Start-Sleep -Milliseconds 100
    }
    foreach ($letter in 'snapshot'.ToCharArray()) { [ReferenceWindow]::SendMessage($window,0x102,[IntPtr][int]$letter,[IntPtr]::Zero) | Out-Null }
    [ReferenceWindow]::Key($window,13,$true)
    Start-Sleep -Milliseconds 100
    [ReferenceWindow]::Key($window,13,$false)
    $snapshot = $null
    for ($attempt=0; $attempt -lt 30; $attempt++) {
        $snapshot=Get-ChildItem -LiteralPath $gameDirectory -Filter "$($session.map)*.bmp" | Where-Object { $_.FullName -notin $before } | Sort-Object LastWriteTime -Descending | Select-Object -First 1
        if ($snapshot) { break }
        Start-Sleep -Milliseconds 100
    }
    if (!$snapshot) { throw 'No native game snapshot appeared. Check console state; use -ConsoleOpen when it is already open. No desktop capture was taken.' }
    [ReferenceWindow]::Key($window,192,$true)
    Start-Sleep -Milliseconds 100
    [ReferenceWindow]::Key($window,192,$false)
    $absolute = [IO.Path]::GetFullPath($Output)
    New-Item -ItemType Directory -Force ([IO.Path]::GetDirectoryName($absolute)) | Out-Null
    $bitmap = [System.Drawing.Image]::FromFile($snapshot.FullName)
    try { $bitmap.Save($absolute, [System.Drawing.Imaging.ImageFormat]::Png) }
    finally { $bitmap.Dispose() }
    Write-Output $absolute
}
