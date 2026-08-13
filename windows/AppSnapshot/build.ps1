$ErrorActionPreference = "Stop"

$msbuild = Join-Path $env:WINDIR "Microsoft.NET\Framework64\v4.0.30319\MSBuild.exe"
if (-not (Test-Path -LiteralPath $msbuild)) {
    $msbuild = Join-Path $env:WINDIR "Microsoft.NET\Framework\v4.0.30319\MSBuild.exe"
}

if (-not (Test-Path -LiteralPath $msbuild)) {
    throw "Windows .NET Framework MSBuild was not found."
}

& $msbuild (Join-Path $PSScriptRoot "AppSnapshot.csproj") /nologo /t:Rebuild /p:Configuration=Release /verbosity:minimal
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

Write-Host "Build complete: $PSScriptRoot\dist\AppSnapshot.exe"
