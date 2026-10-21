# Bulk export Monsters Full Pack -> assets/units/*.glb (requires Blender).
param(
    [string]$SourceRoot = "C:\Users\matt1\My project\Assets\Monsters Full Pack Vol 1",
    [string]$RepoRoot = "C:\BevyFiles\chasma-agent-c",
    [string]$BlenderExe = "",
    [string]$StagingRoot = "C:\BevyFiles\Temp\monster-pack-convert"
)

# Blender writes tracebacks to stderr; do not treat that as a terminating PowerShell error.
$ErrorActionPreference = "Continue"
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$inventoryPath = Join-Path $scriptDir "inventory.json"
$exportPy = Join-Path $scriptDir "blender_export_monster.py"

if (-not (Test-Path $inventoryPath)) {
    py -3 (Join-Path $scriptDir "inventory_source_pack.py") --source $SourceRoot --out $inventoryPath
}

if (-not $BlenderExe) {
    $candidates = @(
        "C:\BevyFiles\Temp\blender-4.2.3\blender-4.2.3-windows-x64\blender.exe",
        "C:\BevyFiles\Temp\blender-4.2.0\blender.exe",
        "C:\Program Files\Blender Foundation\Blender 4.2\blender.exe"
    )
    foreach ($c in $candidates) {
        if (Test-Path $c) { $BlenderExe = $c; break }
    }
}
if (-not (Test-Path $BlenderExe)) {
    throw "Blender not found. Install Blender or set -BlenderExe to blender.exe"
}

$inventory = Get-Content $inventoryPath -Raw | ConvertFrom-Json
$manifest = @()
$assetsUnits = Join-Path $RepoRoot "assets\units"
New-Item -ItemType Directory -Force -Path $assetsUnits | Out-Null

foreach ($monster in $inventory.monsters) {
    if ($monster.incomplete) {
        Write-Warning "Skipping incomplete monster $($monster.name)"
        continue
    }
    $outGlb = Join-Path $assetsUnits ($monster.render_key + ".glb")
    $log = Join-Path $StagingRoot ($monster.render_key + ".log")
    New-Item -ItemType Directory -Force -Path $StagingRoot | Out-Null
    Write-Host "Export $($monster.name) -> $outGlb"
    $exit = 0
    $blenderArgs = @(
        "--background",
        "--python", $exportPy,
        "--",
        "--source-dir", $monster.source_dir,
        "--monster-name", $monster.name,
        "--output", $outGlb
    )
    if ($monster.animation_prefix) {
        $blenderArgs += @("--anim-prefix", $monster.animation_prefix)
    }
    if ($monster.sk_fbx) {
        $blenderArgs += @("--sk-fbx", $monster.sk_fbx)
    }
    & $BlenderExe @blenderArgs 2>&1 | Tee-Object -FilePath $log
    $exit = $LASTEXITCODE
    if (-not (Test-Path $outGlb)) {
        Write-Warning "FAILED $($monster.name) (exit=$exit, see $log)"
        continue
    }
    $clips = py -3 (Join-Path $scriptDir "glb_clips.py") $outGlb
    $manifest += [ordered]@{
        monster = $monster.name
        render_key = $monster.render_key
        glb_path = "assets\units\$($monster.render_key).glb"
        clips = @($clips)
    }
}

$manifestPath = Join-Path $scriptDir "import_manifest.json"
$json = $manifest | ConvertTo-Json -Depth 6
[System.IO.File]::WriteAllText($manifestPath, $json, (New-Object System.Text.UTF8Encoding $false))
Write-Host "Wrote manifest $manifestPath"
