# Monster pack GLB import

**Policy:** `AGENTS.md` (Imported asset fidelity) and `IMPORT_FIDELITY.md`.

## Export (any Unity pack creature)

```text
cd C:\BevyFiles\chasma-agent-c
$env:CHASMA_BLENDER_EXE = "C:\BevyFiles\Temp\blender-4.2.3\blender-4.2.3-windows-x64\blender.exe"
.\tools\monster_pack\export_unity_monster.ps1 `
  -SourceDir "<pack monster folder>" `
  -MonsterName Bufomorph `
  -AnimPrefix Buformorph `
  -SkFbx "<path>\SK_Buformorph.FBX" `
  -Output "C:\BevyFiles\chasma-agent-c\assets\units\bufomorph.glb"
```

Pilot (Bufomorph + Muscomorph): `.\tools\monster_pack\run_option_b_pilot.ps1`

Blender is the only production exporter. `export_monster_fbx2gltf.py` is not used for shipped units.

Post-export **diagnostics only** (does not modify the GLB):

```text
py -3 glb_fbx2gltf_scene_fix.py ..\..\assets\units\bufomorph.glb
py -3 validate_skin_deform.py ..\..\assets\units\bufomorph.glb
```

Do **not** run the removed `fix_bufomorph_glb` orientation/tongue/locomotion strip pass.

## Runtime presentation

Units sheet rotation correction and `ModelComposition` offsets (ADR-128). Ground contact uses catalog/model offset, not GLB scene sink hacks.

## Legacy channel stripping

For pipelines with documented erroneous child translation keys (e.g. tyranopode audit):

```text
py -3 glb_sanitize_animations.py path.glb --strip-child-channels
```

Default monster export does **not** enable this.

## Diagnostics vs shipped assets

Checkpoints and PNG renders live under `tools/monster_pack/diagnostics/` (gitignored GLBs). Shipped game assets are only under `assets/units/`.
