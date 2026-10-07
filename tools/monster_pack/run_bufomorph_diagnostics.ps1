# Bufomorph faithful export checkpoints + headless renders (diagnostics only).
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
$Repo = Resolve-Path (Join-Path $Root "..\..")
$Diag = Join-Path $Root "diagnostics\bufomorph"
$Checkpoints = Join-Path $Diag "checkpoints"
$Renders = Join-Path $Diag "renders"
New-Item -ItemType Directory -Force -Path $Checkpoints, $Renders | Out-Null

$Blender = $env:CHASMA_BLENDER_EXE
if (-not $Blender) {
    $Blender = "C:\BevyFiles\Temp\blender-4.2.3\blender-4.2.3-windows-x64\blender.exe"
}
if (-not (Test-Path $Blender)) {
    throw "Blender not found at $Blender (set CHASMA_BLENDER_EXE)"
}

$SourceDir = "C:\Users\matt1\My project\Assets\Monsters Full Pack Vol 1\Monsters Pack Vol 5\Bufomorph"
$SkFbx = Join-Path $SourceDir "FBX Files\SK_Buformorph.FBX"
$Shipped = Join-Path $Repo "assets\units\bufomorph.glb"
$Output = Join-Path $Repo "assets\units\bufomorph.glb"

if (Test-Path $Shipped) {
    Copy-Item $Shipped (Join-Path $Checkpoints "04_previous_shipped.glb") -Force
}

$manifest = @{
    converter = "Blender 4.2.3 io_scene_gltf2 + monster_export_core"
    sk_import = "FBX2glTF fallback when Blender FBX SK import fails"
    checkpoints = @{
        "01_blender_gltf_export.glb" = "Blender export_scene.gltf GLB, no Python post"
        "02_before_glb_postprocess.glb" = "Copy of 01 (pre rebase/scene-x90)"
        "03_faithful_postexport.glb" = "rebase clip starts + FBX2glTF scene +90 X if SK path used"
        "04_previous_shipped.glb" = "Prior assets/units/bufomorph.glb before regen"
    }
    faithful_post = "finish_glb_post_export(strip=false) + apply_fbx2gltf_scene_root_x90"
}
$manifest | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $Checkpoints "CHECKPOINTS.json") -Encoding utf8

& $Blender --background --python (Join-Path $Root "blender_export_monster.py") -- `
    --source-dir $SourceDir `
    --monster-name Bufomorph `
    --anim-prefix Buformorph `
    --sk-fbx $SkFbx `
    --output $Output `
    --checkpoint-dir $Checkpoints

py -3 (Join-Path $Root "glb_fbx2gltf_scene_fix.py") $Output

$outer = "0,90,0"
foreach ($stage in @(
        @{ glb = "04_previous_shipped.glb"; label = "previous_shipped" },
        @{ glb = "01_blender_gltf_export.glb"; label = "checkpoint_01_raw" },
        @{ glb = "03_faithful_postexport.glb"; label = "checkpoint_03_faithful" }
    )) {
    $path = Join-Path $Checkpoints $stage.glb
    if (-not (Test-Path $path)) { continue }
    & $Blender --background --python (Join-Path $Root "blender_render_compare.py") -- `
        --glb $path `
        --out-dir $Renders `
        --label $stage.label `
        --outer-rotation-deg $outer `
        --clip "IdleBreathe=0.0" `
        --clip "IdleBreathe=1.0" `
        --clip "BiteAttack=0.35" `
        --clip "IdleBreathe=1.5"
}

Write-Host "Diagnostics written under $Diag"
