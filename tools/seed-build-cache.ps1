# Called only while build.ps1 owns the shared mutex. Copy a completed local
# dependency snapshot once; never move, link, delete or mutate the donor target.
param([Parameter(Mandatory=$true)][string]$CachePath)
$ErrorActionPreference = 'Stop'
$repositoryRoot = [IO.Path]::GetFullPath((Split-Path $PSScriptRoot))
$allowedCaches = @('user_data\build-cache', 'user_data\build-cache-core') |
    ForEach-Object { [IO.Path]::GetFullPath((Join-Path $repositoryRoot $_)) }
if ([IO.Path]::GetFullPath($CachePath) -notin $allowedCaches) { throw 'Unexpected shared cache destination.' }
New-Item -ItemType Directory -Force -Path $CachePath | Out-Null
$seedStamp = Join-Path $CachePath 'mashup-seed.json'
if (Test-Path -LiteralPath $seedStamp) { return }
$donor = Join-Path $repositoryRoot 'target\debug'
$copied = 0
foreach ($area in @('deps', '.fingerprint', 'build')) {
    $sourceArea = Join-Path $donor $area
    if (!(Test-Path -LiteralPath $sourceArea)) { continue }
    $destinationArea = Join-Path $CachePath "debug\$area"
    New-Item -ItemType Directory -Force -Path $destinationArea | Out-Null
    foreach ($entry in Get-ChildItem -LiteralPath $sourceArea -Force) {
        # No workspace/app fingerprint or binary is used to prime another tree.
        if ($entry.Name -match '^(lib)?mashup([_\-.]|$)') { continue }
        $destination = Join-Path $destinationArea $entry.Name
        if (!(Test-Path -LiteralPath $destination)) {
            Copy-Item -LiteralPath $entry.FullName -Destination $destination -Recurse
            $copied++
        }
    }
}
@{ donor=$donor; entries=$copied; seeded_at=[DateTime]::UtcNow.ToString('o') } |
    ConvertTo-Json | Set-Content -LiteralPath $seedStamp -Encoding utf8
Write-Output "Shared dependency cache seeded from completed coordinator artifacts ($copied entries); donor retained."
