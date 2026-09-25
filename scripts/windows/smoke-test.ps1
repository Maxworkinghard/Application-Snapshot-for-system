# 装包冒烟（Windows）：静默安装 NSIS 安装包，启动应用，活过一段时间算通过，最后静默卸载。
# 缺运行库、WebView2 起不来、一启动就崩，都会在这里暴露。安装会写注册表和 %LOCALAPPDATA%，
# 只在 CI 或一次性的机器上用。
#
# 用法：pwsh scripts/windows/smoke-test.ps1 -Installer <安装包路径> [-Seconds 15]
param(
  [Parameter(Mandatory = $true)] [string] $Installer,
  [int] $Seconds = 15
)
$ErrorActionPreference = 'Stop'

$setup = Start-Process -FilePath (Resolve-Path $Installer).Path -ArgumentList '/S' -Wait -PassThru
if ($setup.ExitCode -ne 0) { throw "安装程序退出码 $($setup.ExitCode)" }

# Tauri 的 NSIS 安装包会在卸载注册表里登记 DisplayIcon（主程序）和 UninstallString（带引号的路径）
$roots = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall', 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall'
$entry = Get-ChildItem $roots -ErrorAction SilentlyContinue | Get-ItemProperty |
  Where-Object { $_.DisplayName -eq 'snapshot' } | Select-Object -First 1
if (-not $entry) { throw '装完了，但卸载注册表里找不到 snapshot' }
$exe = ($entry.DisplayIcon -replace '"', '') -replace ',\d+$', ''
if (-not (Test-Path $exe)) { throw "找不到装好的主程序：$exe" }
# HKCU 表示装在当前用户下、不需要管理员权限；HKLM 表示装给所有用户
Write-Host "已安装 $($entry.DisplayVersion)：$exe（登记在 $($entry.PSPath -replace '^.*::', '')）"

$app = Start-Process -FilePath $exe -PassThru
Start-Sleep -Seconds $Seconds
if ($app.HasExited) { throw "应用启动后不到 $Seconds 秒就退出了，退出码 $($app.ExitCode)" }
Write-Host "OK：应用启动后活过了 $Seconds 秒"
Stop-Process -Id $app.Id -Force
Start-Sleep -Seconds 2

$uninstaller = $entry.UninstallString -replace '"', ''
$removal = Start-Process -FilePath $uninstaller -ArgumentList '/S' -Wait -PassThru
if ($removal.ExitCode -ne 0) { Write-Warning "卸载程序退出码 $($removal.ExitCode)" } else { Write-Host '已静默卸载' }
