$ErrorActionPreference = "Stop"

function Get-DotNet {
    $cmd = Get-Command dotnet -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    $fallback = Join-Path $env:ProgramFiles "dotnet\dotnet.exe"
    if (Test-Path -LiteralPath $fallback) { return $fallback }
    throw "dotnet SDK was not found. Install .NET 10 SDK (https://aka.ms/dotnet/download) and retry."
}

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

$dotnet = Get-DotNet
$project = Join-Path $PSScriptRoot "..\AppSnapshot.csproj"
$distRoot = Join-Path $PSScriptRoot "..\dist"
$versionFile = Join-Path $PSScriptRoot "..\..\VERSION"
$version = if ($env:RELEASE_VERSION) {
    $env:RELEASE_VERSION.Trim().TrimStart('v')
} elseif (Test-Path -LiteralPath $versionFile) {
    (Get-Content -LiteralPath $versionFile -Raw).Trim()
} else {
    "0.0.0"
}

$targets = @(
    @{ Rid = "win-x64";   Expected = "x64";   Out = (Join-Path $distRoot "win-x64") },
    @{ Rid = "win-arm64"; Expected = "arm64"; Out = (Join-Path $distRoot "win-arm64") }
)

Write-Host "dotnet : $dotnet"
& $dotnet --info
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

foreach ($target in $targets) {
    Write-Host ""
    Write-Host "Publishing $($target.Rid) -> $($target.Out)"
    if (Test-Path -LiteralPath $target.Out) {
        Remove-Item -LiteralPath $target.Out -Recurse -Force
    }
    & $dotnet publish $project `
        -c Release `
        -r $target.Rid `
        --self-contained true `
        -p:Version=$version `
        -o $target.Out
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    $exe = Join-Path $target.Out "AppSnapshot.exe"
    if (-not (Test-Path -LiteralPath $exe)) {
        throw "Publish succeeded but AppSnapshot.exe is missing: $exe"
    }
    $machine = Get-PeMachine -Path $exe
    if ($machine -ne $target.Expected) {
        throw "Architecture mismatch for $($target.Rid): expected $($target.Expected), PE machine is $machine"
    }
    $sizeMb = [math]::Round((Get-Item -LiteralPath $exe).Length / 1MB, 1)
    Write-Host "OK $($target.Rid) $machine ${sizeMb} MB  $exe"
}

Write-Host ""
Write-Host "Build complete:"
Write-Host "  x64    $distRoot\win-x64\AppSnapshot.exe"
Write-Host "  ARM64  $distRoot\win-arm64\AppSnapshot.exe"
