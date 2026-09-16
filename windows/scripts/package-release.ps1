param(
    [switch]$SkipBuild
)

# Stage GitHub Release assets:
#   Application-Snapshot-<version>-windows-x64.exe
#   Application-Snapshot-<version>-windows-arm64.exe
# plus SHA256SUMS.txt for those two files. CI rebuilds the full checksum list.

$ErrorActionPreference = "Stop"

function Get-PeMachine {
    param([string]$Path)
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 64 -or $bytes[0] -ne 0x4D -or $bytes[1] -ne 0x5A) {
        throw "Not a PE file: $Path"
    }
    $peOffset = [BitConverter]::ToInt32($bytes, 0x3C)
    $machine = [BitConverter]::ToUInt16($bytes, $peOffset + 4)
    switch ($machine) {
        0x8664 { "x64" }
        0xAA64 { "arm64" }
        0x014C { "x86" }
        default { "unknown(0x$($machine.ToString('X')))" }
    }
}

function Get-ReleaseVersion {
    if ($env:RELEASE_VERSION) {
        return $env:RELEASE_VERSION.Trim().TrimStart('v')
    }
    $versionFile = Join-Path $PSScriptRoot "..\..\VERSION"
    return (Get-Content -LiteralPath $versionFile -Raw).Trim()
}

if (-not $SkipBuild) {
    & (Join-Path $PSScriptRoot "build.ps1")
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

$version = Get-ReleaseVersion
$prefix = "Application-Snapshot-$version"
$repoRoot = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$stage = Join-Path $repoRoot "dist\release"
New-Item -ItemType Directory -Force -Path $stage | Out-Null

$pairs = @(
    @{ Rid = "win-x64";   Expected = "x64";   Asset = "$prefix-windows-x64.exe" },
    @{ Rid = "win-arm64"; Expected = "arm64"; Asset = "$prefix-windows-arm64.exe" }
)

foreach ($pair in $pairs) {
    $source = Join-Path $PSScriptRoot "..\dist\$($pair.Rid)\AppSnapshot.exe"
    if (-not (Test-Path -LiteralPath $source)) {
        throw "Missing $source. Run build.ps1 first."
    }
    $machine = Get-PeMachine -Path $source
    if ($machine -ne $pair.Expected) {
        throw "Architecture mismatch for $($pair.Rid): expected $($pair.Expected), PE is $machine"
    }
    if ($machine -eq "x86") {
        throw "32-bit Windows builds are not shipped."
    }
    $dest = Join-Path $stage $pair.Asset
    Copy-Item -LiteralPath $source -Destination $dest -Force
    $copied = Get-PeMachine -Path $dest
    if ($copied -ne $pair.Expected) {
        throw "Copied asset changed architecture: $dest is $copied"
    }
    Write-Host "OK $($pair.Expected)  $dest"
}

$sumPath = Join-Path $stage "SHA256SUMS.txt"
$lines = foreach ($pair in $pairs) {
    $file = Get-Item -LiteralPath (Join-Path $stage $pair.Asset)
    $hash = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    "{0}  {1}" -f $hash, $file.Name
}
$lines | Set-Content -LiteralPath $sumPath -Encoding ascii
Write-Host "OK checksums  $sumPath"
