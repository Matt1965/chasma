# Faithful monster-pack export -> assets/units/*.glb (no channel strip; writes .export_manifest.json).
param(
    [string]$RepoRoot = "C:\BevyFiles\chasma",
    [string]$BlenderExe = $env:CHASMA_BLENDER_EXE,
    [string]$LogRoot = "C:\BevyFiles\Temp\monster-pack-faithful-export",
    [string[]]$OnlyRenderKeys = @(),
    [switch]$SkipExistingComplete
)

$ErrorActionPreference = "Continue"
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$inventoryPath = Join-Path $scriptDir "inventory.json"
$exportPy = Join-Path $scriptDir "blender_export_monster.py"
$auditPy = Join-Path $scriptDir "audit_glb_animation_fidelity.py"

$SkipKeys = @("bufomorph") # already faithful reference

if (-not $BlenderExe) {
    $BlenderExe = "C:\BevyFiles\Temp\blender-4.2.3\blender-4.2.3-windows-x64\blender.exe"
}
if (-not (Test-Path $BlenderExe)) { throw "Blender not found: $BlenderExe" }
if (-not (Test-Path $inventoryPath)) { throw "Missing inventory.json (copy from agent-c or run inventory_source_pack.py)" }

New-Item -ItemType Directory -Force -Path $LogRoot | Out-Null
$assetsUnits = Join-Path $RepoRoot "assets\units"
New-Item -ItemType Directory -Force -Path $assetsUnits | Out-Null

$inventory = Get-Content $inventoryPath -Raw | ConvertFrom-Json
$recovery = @()

foreach ($monster in $inventory.monsters) {
    $key = $monster.render_key
    if ($SkipKeys -contains $key) { continue }
    if ($OnlyRenderKeys.Count -gt 0 -and ($OnlyRenderKeys -notcontains $key)) { continue }
    if ($monster.incomplete) {
        $recovery += [ordered]@{ render_key = $key; status = "skipped_incomplete" }
        continue
    }
    if (-not (Test-Path $monster.source_dir)) {
        $recovery += [ordered]@{ render_key = $key; status = "blocked_missing_source"; source_dir = $monster.source_dir }
        continue
    }
    $outGlb = Join-Path $assetsUnits ($key + ".glb")
    $manifest = $outGlb -replace '\.glb$', '.export_manifest.json'
    if ($SkipExistingComplete -and (Test-Path $manifest)) {
        $m = Get-Content $manifest -Raw | ConvertFrom-Json
        if ($m.complete -eq $true) {
            Write-Host "Skip $key (manifest complete)"
            $recovery += [ordered]@{ render_key = $key; status = "skipped_already_complete" }
            continue
        }
    }

    $log = Join-Path $LogRoot ($key + ".log")
    Write-Host "Faithful export $($monster.name) -> $outGlb"
    $blenderArgs = @(
        "--background", "--python", $exportPy, "--",
        "--source-dir", $monster.source_dir,
        "--monster-name", $monster.name,
        "--output", $outGlb
    )
    if ($monster.animation_prefix) { $blenderArgs += @("--anim-prefix", $monster.animation_prefix) }
    if ($monster.sk_fbx) { $blenderArgs += @("--sk-fbx", $monster.sk_fbx) }

    & $BlenderExe @blenderArgs 2>&1 | Tee-Object -FilePath $log
    $row = [ordered]@{
        render_key = $key
        monster = $monster.name
        glb = $outGlb
        log = $log
        blender_exit = $LASTEXITCODE
    }
    if (-not (Test-Path $outGlb)) {
        $row.status = "failed_no_glb"
        $recovery += $row
        continue
    }
    if (-not (Test-Path $manifest)) {
        $row.status = "failed_no_manifest"
        $recovery += $row
        continue
    }
    $mf = Get-Content $manifest -Raw | ConvertFrom-Json
    $row.manifest_complete = $mf.complete
    $row.unique_clips_merged = $mf.unique_clips_merged
    $row.unique_clips_expected = $mf.unique_clips_expected
    $audit = py -3 $auditPy $outGlb 2>&1 | Out-String
    $row.audit = $audit.Trim()
    if ($mf.complete -ne $true) {
        $row.status = "failed_incomplete_manifest"
    } else {
        $row.status = "ok"
    }
    $recovery += $row
}

$outJson = Join-Path $scriptDir "diagnostics\FAITHFUL_BATCH_RECOVERY.json"
$json = $recovery | ConvertTo-Json -Depth 6
[System.IO.File]::WriteAllText($outJson, $json, (New-Object System.Text.UTF8Encoding $false))
Write-Host "Wrote $outJson"
