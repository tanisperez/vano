# Merges console.manifest into the manifest resource of vano.exe, with mt.exe from the
# Windows SDK. Run after `cargo build` and before packaging, see console.manifest.
#
#   powershell -File packaging\windows\merge-manifest.ps1 target\x86_64-pc-windows-msvc\release\vano.exe
param([Parameter(Mandatory = $true)][string]$Exe)
$ErrorActionPreference = 'Stop'

$mt = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\mt.exe" |
    Sort-Object FullName | Select-Object -Last 1
if (-not $mt) { throw 'mt.exe not found: install the Windows SDK' }
$fragment = Join-Path $PSScriptRoot 'console.manifest'

& $mt.FullName -nologo "-inputresource:$Exe;#1" -manifest $fragment "-outputresource:$Exe;#1"
if ($LASTEXITCODE -ne 0) { throw "mt.exe failed with exit code $LASTEXITCODE" }

# The merged manifest must keep gpui's settings and carry ours.
& $mt.FullName -nologo "-inputresource:$Exe;#1" "-out:$env:TEMP\vano.manifest"
$merged = Get-Content -Raw "$env:TEMP\vano.manifest"
foreach ($needed in 'consoleAllocationPolicy', 'PerMonitorV2', 'Microsoft.Windows.Common-Controls') {
    if ($merged -notmatch [regex]::Escape($needed)) { throw "the merged manifest lacks $needed" }
}
Write-Host "Merged console.manifest into $Exe"
