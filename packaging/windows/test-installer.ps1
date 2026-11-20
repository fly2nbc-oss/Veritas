#Requires -Version 7
# Installs the NSIS installer silently, waits for the window and uninstalls again.
$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Set-Location (Resolve-Path "$PSScriptRoot\..\..")

$setup = Get-ChildItem dist\windows\*_x64-setup.exe | Select-Object -First 1
if (-not $setup) { throw "no installer in dist\windows" }
Start-Process $setup.FullName -ArgumentList "/S" -Wait

$dir = "$env:LOCALAPPDATA\Veritas"
$exe = Join-Path $dir "veritas.exe"
if (-not (Test-Path $exe)) { throw "veritas.exe is missing after the installation in $dir" }

$log = Join-Path $env:RUNNER_TEMP "veritas-start.log"
if (Test-Path $log) { Remove-Item $log }
$proc = Start-Process $exe -PassThru -RedirectStandardError $log
$ready = $false
$noGl = $false
foreach ($i in 1..40) {
    Start-Sleep -Milliseconds 500
    if (Test-Path $log) {
        if (Select-String -Path $log -Pattern "start-up: window ready" -Quiet) { $ready = $true; break }
        if (Select-String -Path $log -Pattern "opengl 2.0" -Quiet) { $noGl = $true; break }
    }
    if ($proc.HasExited) { break }
}
if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force; $proc.WaitForExit() }
Get-Content $log -ErrorAction SilentlyContinue | Select-Object -Last 40
if ($noGl) {
    Write-Warning "The runner has no OpenGL 2.0. Installation and uninstallation are checked anyway."
} elseif (-not $ready) {
    throw "Veritas did not report its window"
}

$uninstaller = Join-Path $dir "uninstall.exe"
if (-not (Test-Path $uninstaller)) { throw "uninstall.exe is missing" }
$p = Start-Process $uninstaller -ArgumentList "/S", "_?=$dir" -Wait -PassThru
if ($p.ExitCode) { throw "The uninstallation ended with $($p.ExitCode)" }
if (Test-Path $exe) { throw "veritas.exe is still in $dir after the uninstallation" }
Write-Host "installed, started, uninstalled"
