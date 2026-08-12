<#
.SYNOPSIS
    构建 Windows 版应用快照。
.DESCRIPTION
    在 Windows 上执行：发布 self-contained=false 框架依赖版。
    产物路径：windows\dist\WindowSnap-<arch>\WindowSnap.exe
.PARAMETER Arch
    auto（默认，跟随本机架构）/ x64 / arm64 / all
#>

[CmdletBinding()]
param(
    [string]$Configuration = "Release",
    [ValidateSet("auto", "x64", "arm64", "all")]
    [string]$Arch = "auto"
)

$ErrorActionPreference = "Stop"
$Root = Resolve-Path "$PSScriptRoot"
$Project = Join-Path $Root "WindowSnap.Wpf\WindowSnap.Wpf.csproj"

if ($Arch -eq "auto") {
    $Arch = if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "arm64" } else { "x64" }
}
$Targets = if ($Arch -eq "all") { @("x64", "arm64") } else { @($Arch) }

foreach ($a in $Targets) {
    $Dist = Join-Path $Root "dist\WindowSnap-$a"
    Write-Host "==> dotnet restore & publish ($Configuration, win-$a)"
    dotnet publish $Project `
        -c $Configuration `
        -r "win-$a" `
        --self-contained false `
        -p:PublishSingleFile=false `
        -o $Dist

    if ($LASTEXITCODE -ne 0) {
        Write-Error "构建失败：dotnet publish (win-$a) 退出码 $LASTEXITCODE"
        exit 1
    }
    Write-Host "产物: $(Join-Path $Dist 'WindowSnap.exe')"
}

Write-Host ""
Write-Host "==> 构建成功"
Write-Host "运行前请确认："
Write-Host "  1. 目标机器已安装对应架构的 .NET 8 Desktop Runtime（x64 机器装 x64 版，ARM 机器装 arm64 版）"
Write-Host "  2. 首次截图若失败，去 系统设置 → 隐私和安全性 检查屏幕捕获权限后重启应用"
