# Shared build gate for the coordinator and all Windows game-agent worktrees.
# Example: & D:\Mashup\tools\build.ps1 build --locked --bin mashup-gtasa
$ErrorActionPreference = 'Stop'
if ($args.Count -eq 0 -or $args[0] -notin @('build', 'prepare')) {
    throw 'Pass build --locked --bin <owned-binary>, or prepare to bootstrap the shared cache.'
}
$prepareOnly = $args[0] -eq 'prepare'
if ($args -contains '--release') { throw 'Mashup workers use dev builds only.' }
$env:CARGO_BUILD_JOBS = '12'
$wrapperPath = Join-Path $PSScriptRoot 'capped-rustc-v2.exe'
$wrapperSource = Join-Path $PSScriptRoot 'capped-rustc.rs'
$buildHost = [Diagnostics.Process]::GetCurrentProcess()
# Conservative bootstrap affinity until the native helper reads physical topology.
$buildHost.ProcessorAffinity = [IntPtr]63
$buildHost.PriorityClass = 'Normal'
$bootstrapGate = [Threading.Mutex]::new($false, 'Local\MashupBuildBootstrap')
$ownsBootstrapGate = $false
try {
    try { $ownsBootstrapGate = $bootstrapGate.WaitOne() }
    catch [Threading.AbandonedMutexException] { $ownsBootstrapGate = $true }
    # Bootstrap a versioned helper without replacing the old running wrapper.
    # No dependencies, no release optimization, no batch argument forwarding.
    if (!(Test-Path -LiteralPath $wrapperPath) -or
        (Get-Item -LiteralPath $wrapperSource).LastWriteTimeUtc -gt (Get-Item -LiteralPath $wrapperPath).LastWriteTimeUtc) {
        & rustc --edition=2024 -C opt-level=0 -C debuginfo=1 $wrapperSource -o $wrapperPath
        if ($LASTEXITCODE -ne 0) { throw 'Unable to build the capped native compiler wrapper.' }
    }
    $policy = (& $wrapperPath --print-policy | ConvertFrom-Json)
    if ($LASTEXITCODE -ne 0) { throw 'Unable to read the native compiler CPU policy.' }
    $buildHost.ProcessorAffinity = [IntPtr]([long]$policy.affinity_mask)
    $buildHost.PriorityClass = 'Normal'
    Write-Output "Build budget: $($policy.budget_cores)/$($policy.physical_cores) physical cores, $($policy.logical_cpus) logical CPUs, 12 jobs, Normal."
    $workspaceRoot = (Get-Location).Path
    $workspaceWrapper = Join-Path $workspaceRoot 'target\mashup-build\workspace-rustc.exe'
    New-Item -ItemType Directory -Force -Path (Split-Path $workspaceWrapper) | Out-Null
    if (!(Test-Path -LiteralPath $workspaceWrapper) -or
        (Get-Item -LiteralPath $wrapperPath).LastWriteTimeUtc -gt (Get-Item -LiteralPath $workspaceWrapper).LastWriteTimeUtc) {
        Copy-Item -LiteralPath $wrapperPath -Destination $workspaceWrapper
    }
    # Cargo hashes this unique wrapper path for workspace crates only. Registry
    # dependencies share their normal hashes; divergent application crates cannot.
    $env:RUSTC_WRAPPER = $wrapperPath
    $env:RUSTC_WORKSPACE_WRAPPER = $workspaceWrapper
    $env:CARGO_BUILD_BUILD_DIR = Join-Path (Split-Path $PSScriptRoot) 'user_data\build-cache'
    if (!$env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR = Join-Path $workspaceRoot 'target' }
    & (Join-Path $PSScriptRoot 'seed-build-cache.ps1') -CachePath $env:CARGO_BUILD_BUILD_DIR
} finally {
    if ($ownsBootstrapGate) { $bootstrapGate.ReleaseMutex() }
    $bootstrapGate.Dispose()
}
if ($prepareOnly) { exit 0 }
$buildGate = [Threading.Mutex]::new($false, 'Local\MashupSharedDependencyBuild')
$ownsBuildGate = $false
try {
    Write-Output 'Dependencies are prepared. Waiting for shared Cargo slot; the active invocation uses up to 12 compiler jobs.'
    try { $ownsBuildGate = $buildGate.WaitOne() }
    catch [Threading.AbandonedMutexException] { $ownsBuildGate = $true }
    & cargo @args
    $buildExitCode = $LASTEXITCODE
} finally {
    if ($ownsBuildGate) { $buildGate.ReleaseMutex() }
    $buildGate.Dispose()
}
exit $buildExitCode
