# Pilot export: Bufomorph (+ Muscomorph) via FBX2glTF glTF merge (production path).
$ErrorActionPreference = "Stop"
$Root = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
Set-Location $Root

$Pack = "C:\Users\matt1\My project\Assets\Monsters Full Pack Vol 1"
$Export = Join-Path $PSScriptRoot "export_unity_monster.ps1"

& $Export `
    -SourceDir "$Pack\Monsters Pack Vol 5\Bufomorph" `
    -Prefab "$Pack\Monsters Pack Vol 5\Bufomorph\Prefabs\Buformorph_Red.prefab" `
    -Output "$Root\assets\units\bufomorph.glb"

& $Export `
    -SourceDir "$Pack\Monsters Pack Vol 2\Muscomorph" `
    -Output "$Root\assets\units\muscomorph.glb"

Write-Host "Pilot exports complete. Run game from this worktree:"
Write-Host "  cd $Root"
Write-Host "  cargo run --features dev"
