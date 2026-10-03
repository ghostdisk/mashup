# Shared build gate for the coordinator and all Windows game-agent worktrees.
# Example: & D:\Mashup\tools\build.ps1 build --locked --bin mashup-gtasa
$ErrorActionPreference = 'Stop'
if ($args.Count -eq 0 -or $args[0] -notin @('build', 'prepare')) {
    throw 'Pass build --locked --bin <owned-binary>, or prepare to bootstrap the shared cache.'
}
$prepareOnly = $args[0] -eq 'prepare'
Write-Output "Build owner: helper PID $PID; workspace $((Get-Location).Path); command $($args -join ' ')"
if ($args -contains '--release') { throw 'Mashup workers use dev builds only.' }
# Two independent dependency lanes share the same CPU mask. Each receives six
# compiler jobs, for at most twelve across newly launched invocations.
$env:CARGO_BUILD_JOBS = '6'
$htmlLane = $args -contains 'mashup-ui' -or ($args -join ' ') -match 'html-ui|--features(?:=|\s)'
$lane = if ($htmlLane) { 'html' } else { 'core' }
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
    Write-Output "Build budget: $($policy.budget_cores)/$($policy.physical_cores) physical cores, $($policy.logical_cpus) logical CPUs, six jobs in $lane lane, Normal."
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
    $cacheName = if ($htmlLane) { 'build-cache' } else { 'build-cache-core' }
    $env:CARGO_BUILD_BUILD_DIR = Join-Path (Split-Path $PSScriptRoot) "user_data\$cacheName"
    if (!$env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR = Join-Path $workspaceRoot 'target' }
    & (Join-Path $PSScriptRoot 'seed-build-cache.ps1') -CachePath $env:CARGO_BUILD_BUILD_DIR
} finally {
    if ($ownsBootstrapGate) { $bootstrapGate.ReleaseMutex() }
    $bootstrapGate.Dispose()
}
if ($prepareOnly) { exit 0 }
# HTML retains the existing gate/cache so an already active CEF build can finish
# without losing its work. Core uses an independent warmed cache and a priority
# queue; Cargo's own build-directory lock no longer couples those two graphs.
$gateName = if ($htmlLane) { 'Local\MashupSharedDependencyBuild' } else { 'Local\MashupCoreBuild' }
$buildGate = [Threading.Mutex]::new($false, $gateName)
$ownsBuildGate = $false
$ticketPath = $null
$queueRoot = Join-Path (Split-Path $PSScriptRoot) 'user_data\build-queue-core'
$activePath = Join-Path (Split-Path $PSScriptRoot) "user_data\build-active-$lane.json"
try {
    if ($htmlLane) {
        Write-Output 'Waiting for HTML slot; core/demo builds use an independent cache and slot.'
        try { $ownsBuildGate = $buildGate.WaitOne() }
        catch [Threading.AbandonedMutexException] { $ownsBuildGate = $true }
    } else {
        New-Item -ItemType Directory -Force -Path $queueRoot | Out-Null
        $request = [guid]::NewGuid().ToString('N')
        $ticketPath = Join-Path $queueRoot "$request.json"
        $priority = if ($args -contains 'mashup-demo') { 0 } elseif ($args -contains '--features') { 20 } else { 10 }
        $ticket = @{ id=$request; pid=$PID; start=$buildHost.StartTime.ToUniversalTime().Ticks; queued=[DateTime]::UtcNow.Ticks; priority=$priority; workspace=$workspaceRoot; command=($args -join ' ') }
        $tempTicket = Join-Path $queueRoot "$request.tmp"
        $ticket | ConvertTo-Json | Set-Content -LiteralPath $tempTicket -Encoding utf8
        Move-Item -LiteralPath $tempTicket -Destination $ticketPath
        Write-Output "Waiting for core slot (priority $priority); demo checkpoints precede ordinary/optional-feature builds."
        while (!$ownsBuildGate) {
            try { $ownsBuildGate = $buildGate.WaitOne(250) }
            catch [Threading.AbandonedMutexException] { $ownsBuildGate = $true }
            if (!$ownsBuildGate) { continue }
            $liveTickets = foreach ($file in Get-ChildItem -LiteralPath $queueRoot -Filter '*.json') {
                try { $candidate = Get-Content -LiteralPath $file.FullName -Raw | ConvertFrom-Json } catch { continue }
                $owner = Get-Process -Id $candidate.pid -ErrorAction SilentlyContinue
                if (!$owner -or $owner.StartTime.ToUniversalTime().Ticks -ne $candidate.start) {
                    Remove-Item -LiteralPath $file.FullName
                    continue
                }
                $candidate
            }
            $first = $liveTickets | Sort-Object priority, queued | Select-Object -First 1
            if ($first.id -ne $request) {
                $buildGate.ReleaseMutex()
                $ownsBuildGate = $false
                Start-Sleep -Milliseconds 250
            }
        }
        Remove-Item -LiteralPath $ticketPath
        $ticketPath = $null
    }
    Write-Output "Build slot granted: lane $lane; helper PID $PID; $($args -join ' '); cache $env:CARGO_BUILD_BUILD_DIR"
    @{ pid=$PID; start=$buildHost.StartTime.ToUniversalTime().Ticks; lane=$lane; workspace=$workspaceRoot; command=($args -join ' '); acquired=[DateTime]::UtcNow.ToString('o') } |
        ConvertTo-Json | Set-Content -LiteralPath $activePath -Encoding utf8
    & cargo @args
    $buildExitCode = $LASTEXITCODE
} finally {
    if ($ticketPath -and (Test-Path -LiteralPath $ticketPath)) { Remove-Item -LiteralPath $ticketPath }
    if ($ownsBuildGate) {
        if (Test-Path -LiteralPath $activePath) { Remove-Item -LiteralPath $activePath }
        $buildGate.ReleaseMutex()
    }
    $buildGate.Dispose()
}
exit $buildExitCode
