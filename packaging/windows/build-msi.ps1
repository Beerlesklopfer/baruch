# SPDX-License-Identifier: AGPL-3.0-or-later
# Baut Baruch-<Version>-x64.msi mit dem WiX Toolset 3 (candle/light; auf den GitHub-Runnern windows-2022
# vorinstalliert, Umgebungsvariable WIX). Unsigniert; Signieren ist ein eigener Schritt in release.yml.
#
#   pwsh packaging/windows/build-msi.ps1 [-Binary PFAD] [-OutDir ORDNER] [-SkipValidation]
#
# STAND: noch nie gelaufen (lokal kein Windows). Scheitert light an der ICE-Prüfung des Runners
# (LGHT0217), hilft -SkipValidation.
param(
    [string]$Binary = "target\release\baruch-server.exe",
    [string]$OutDir = "dist",
    [switch]$SkipValidation
)
$ErrorActionPreference = "Stop"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
Set-Location $root

$version = (& bash scripts/version.sh)
if ($LASTEXITCODE -ne 0 -or -not $version) { throw "Version nicht ermittelbar (scripts/version.sh)." }
$version = $version.Trim()
if ($version -notmatch '^\d+\.\d+\.\d+$') { throw "MSI braucht eine Version der Form x.y.z, nicht '$version'." }

$binaryPath = (Resolve-Path $Binary).Path
$licenses = Join-Path $root "packaging\generated\THIRD-PARTY-LICENSES.txt"
if (-not (Test-Path $licenses)) { throw "$licenses fehlt (scripts/third-party-licenses.py)." }
$sourceOffer = Join-Path $root "packaging\generated\QUELLTEXT.txt"
if (-not (Test-Path $sourceOffer)) { throw "$sourceOffer fehlt (scripts/third-party-licenses.py)." }

if (-not $env:WIX) { throw "WiX Toolset 3 nicht gefunden (Umgebungsvariable WIX fehlt)." }
$candle = Join-Path $env:WIX "bin\candle.exe"
$light = Join-Path $env:WIX "bin\light.exe"

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$obj = Join-Path $root "target\wix"
New-Item -ItemType Directory -Force -Path $obj | Out-Null
$wixobj = Join-Path $obj "baruch.wixobj"
$msi = Join-Path $OutDir "Baruch-$version-x64.msi"

& $candle -nologo -arch x64 "-dVersion=$version" "-dBinary=$binaryPath" "-dSourceDir=$root" `
    "-dLicenses=$licenses" "-dSourceOffer=$sourceOffer" -out $wixobj "packaging\windows\baruch.wxs"
if ($LASTEXITCODE -ne 0) { throw "candle meldet Fehler." }

$lightArgs = @("-nologo", "-out", $msi, $wixobj)
if ($SkipValidation) { $lightArgs = @("-sval") + $lightArgs }
& $light @lightArgs
if ($LASTEXITCODE -ne 0) { throw "light meldet Fehler." }

Write-Host "MSI: $msi"
