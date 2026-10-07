# Monster pack GLB import

**Policy:** `AGENTS.md` (Imported asset fidelity) and `IMPORT_FIDELITY.md`.

## Faithful export (Bufomorph and pack creatures)

```text
cd C:\BevyFiles\chasma\tools\monster_pack
$env:CHASMA_BLENDER_EXE = "C:\BevyFiles\Temp\blender-4.2.3\blender-4.2.3-windows-x64\blender.exe"
.\run_bufomorph_diagnostics.ps1
```

Or single export:

```text
blender --background --python blender_export_monster.py -- ^
  --source-dir "<pack monster folder>" ^
  --monster-name Bufomorph ^
  --anim-prefix Buformorph ^
  --sk-fbx "<path>\SK_Buformorph.FBX" ^
  --output "C:\BevyFiles\chasma\assets\units\bufomorph.glb"
```

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
