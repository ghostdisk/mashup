# Shared build gate for the coordinator and all Windows game-agent worktrees.
# Example: & D:\Mashup\tools\build.ps1 build --locked --bin mashup-gtasa
$ErrorActionPreference = 'Stop'
if ($args.Count -eq 0 -or $args[0] -ne 'build') {
    throw 'Pass a Cargo build command, for example: build --locked --bin mashup-cstrike'
}
$env:CARGO_BUILD_JOBS = '1'
$wrapperPath = Join-Path $PSScriptRoot 'capped-rustc.exe'
$wrapperSource = Join-Path $PSScriptRoot 'capped-rustc.rs'
$buildHost = [Diagnostics.Process]::GetCurrentProcess()
$buildHost.ProcessorAffinity = [IntPtr]63
$buildHost.PriorityClass = 'BelowNormal'
$buildGate = [Threading.Mutex]::new($false, 'Local\MashupRustBuild')
$ownsBuildGate = $false
try {
    Write-Output 'Waiting for the shared build slot (six CPUs, one build, BelowNormal).'
    try { $ownsBuildGate = $buildGate.WaitOne() }
    catch [Threading.AbandonedMutexException] { $ownsBuildGate = $true }
    # Bootstrap directly under this already capped process, before invoking Cargo.
    # No dependencies, no release optimization, no batch argument forwarding.
    if (!(Test-Path -LiteralPath $wrapperPath) -or
        (Get-Item -LiteralPath $wrapperSource).LastWriteTimeUtc -gt (Get-Item -LiteralPath $wrapperPath).LastWriteTimeUtc) {
        & rustc --edition=2024 -C opt-level=0 -C debuginfo=1 $wrapperSource -o $wrapperPath
        if ($LASTEXITCODE -ne 0) { throw 'Unable to build the capped native compiler wrapper.' }
    }
    $env:RUSTC_WRAPPER = $wrapperPath
    & cargo @args
    $buildExitCode = $LASTEXITCODE
} finally {
    if ($ownsBuildGate) { $buildGate.ReleaseMutex() }
    $buildGate.Dispose()
}
exit $buildExitCode
