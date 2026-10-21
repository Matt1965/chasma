# Generic Unity monster-pack -> Chasma GLB (FBX2glTF + glTF merge; see IMPORT_FIDELITY.md).
# Usage:
#   .\tools\monster_pack\export_unity_monster.ps1 `
#     -SourceDir "...\Bufomorph" `
#     -Output "...\assets\units\bufomorph.glb" `
#     [-Prefab "...\Prefabs\Buformorph_Red.prefab"]

param(
    [Parameter(Mandatory = $true)][string]$SourceDir,
    [Parameter(Mandatory = $true)][string]$Output,
    [string]$Prefab = ""
)

$ErrorActionPreference = "Stop"
$Here = $PSScriptRoot
$Repo = Split-Path (Split-Path $Here -Parent) -Parent
Set-Location $Repo

$args = @(
    "tools\monster_pack\export_monster_fbx2gltf.py",
    "--source-dir", $SourceDir,
    "--output", $Output
)
if ($Prefab) {
    $args += @("--prefab", $Prefab)
}
py -3 @args
if ($LASTEXITCODE -ne 0) {
    throw "export_monster_fbx2gltf.py failed"
}
Write-Host "Exported $Output"
