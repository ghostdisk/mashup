# Shared build gate for the coordinator and all Windows game-agent worktrees.
# Example: & D:\Mashup\tools\build.ps1 build --locked --bin mashup-gtasa
$ErrorActionPreference = 'Stop'
if ($args.Count -eq 0 -or $args[0] -ne 'build') {
    throw 'Pass a Cargo build command, for example: build --locked --bin mashup-cstrike'
}
$env:CARGO_BUILD_JOBS = '1'
$env:RUSTC_WRAPPER = Join-Path $PSScriptRoot 'capped-rustc.cmd'
$buildHost = [Diagnostics.Process]::GetCurrentProcess()
$buildHost.ProcessorAffinity = [IntPtr]63
$buildHost.PriorityClass = 'BelowNormal'
$buildGate = [Threading.Mutex]::new($false, 'Local\MashupRustBuild')
$ownsBuildGate = $false
try {
    Write-Output 'Waiting for the shared build slot (six CPUs, one build, BelowNormal).'
    try { $ownsBuildGate = $buildGate.WaitOne() }
    catch [Threading.AbandonedMutexException] { $ownsBuildGate = $true }
    & cargo @args
    $buildExitCode = $LASTEXITCODE
} finally {
    if ($ownsBuildGate) { $buildGate.ReleaseMutex() }
    $buildGate.Dispose()
}
exit $buildExitCode
