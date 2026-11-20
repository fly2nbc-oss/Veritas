#Requires -Version 7
<#
.SYNOPSIS
Packs a Windows release build into dist\windows.

.DESCRIPTION
Run after `cargo build --release`. Produces

  Veritas\                              portable folder: exe, LICENSE, NOTICE, THIRD_PARTY.md,
                                        font license texts
  veritas_<version>_x64-portable.zip
  veritas_<version>_x64-setup.exe       NSIS installer (cargo-packager)

The C runtime is linked statically (.cargo/config.toml), so no DLLs are included.
The script needs no Visual Studio tools, only `cargo-packager` for the installer.
#>
param(
    [string]$Target = "target\release",
    [switch]$NoInstaller
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Set-Location (Resolve-Path "$PSScriptRoot\..\..")

$version = (Get-Content Cargo.toml | Select-String -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches.Groups[1].Value
$dist = "dist\windows"
$stage = "$dist\Veritas"
if (Test-Path $dist) { Remove-Item -Recurse -Force $dist }
New-Item -ItemType Directory -Force $stage | Out-Null

$exe = Join-Path $Target "veritas.exe"
if (-not (Test-Path $exe)) { throw "$exe is missing - run: cargo build --release" }

# No imports of the dynamic C runtime: otherwise the setting from .cargo/config.toml did not
# take effect (a RUSTFLAGS environment variable overrides it without the build saying so), and
# the exe would not start on a machine without the VC++ Redistributable. The names are in plain
# text in the import table; the search needs no dumpbin.
$text = [Text.Encoding]::Latin1.GetString([IO.File]::ReadAllBytes($exe))
$dynamic = [regex]::Matches($text, '(?i)\b(vcruntime|msvcp|msvcr|vcomp|concrt|ucrtbase|api-ms-win-crt-)[\w.-]*?\.dll') |
    ForEach-Object { $_.Value } | Sort-Object -Unique
if ($dynamic) {
    throw "veritas.exe imports the dynamic C runtime ($($dynamic -join ', ')). RUSTFLAGS set? Then add: -C target-feature=+crt-static"
}

Copy-Item $exe "$stage\veritas.exe"
Copy-Item LICENSE, NOTICE, THIRD_PARTY.md $stage
Copy-Item assets\fonts\*.txt $stage

Compress-Archive -Path $stage -DestinationPath "$dist\veritas_${version}_x64-portable.zip"
if (-not $NoInstaller) {
    cargo packager --release --formats nsis
    if ($LASTEXITCODE) { throw "cargo packager failed" }
}

Get-ChildItem $dist -File | ForEach-Object {
    "{0}  {1,12:N0}  {2}" -f (Get-FileHash $_.FullName SHA256).Hash.ToLower(), $_.Length, $_.Name
}
