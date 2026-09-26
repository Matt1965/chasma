# Monster pack import (Unity FBX -> Chasma GLB)

External source (not in repo):

`C:\Users\matt1\My project\Assets\Monsters Full Pack Vol 1`

## Steps

1. **Inventory** (read-only scan of source tree):

   ```text
   py -3 tools/monster_pack/inventory_source_pack.py
   ```

   Writes `tools/monster_pack/inventory.json` (29 monsters; excludes Cavecrawler).

2. **Batch export** (requires Blender 4.x):

   ```powershell
   powershell -ExecutionPolicy Bypass -File tools/monster_pack/run_batch_export.ps1 `
     -BlenderExe "C:\BevyFiles\Temp\blender-4.2.3\blender-4.2.3-windows-x64\blender.exe"
   ```

   Output: `assets/units/<render_key>.glb` and `tools/monster_pack/import_manifest.json`.

   Per-monster logs: `C:\BevyFiles\Temp\monster-pack-convert\<render_key>.log`

3. **List GLB clips** (for animation profile authoring):

   ```text
   py -3 tools/monster_pack/glb_clips.py assets/units/tyranopode.glb
   ```

4. **Workbook integration**:

   ```text
   py -3 tools/integrate_monster_pack_workbook.py
   ```

   Backup: `Chasma Design.xlsx.monster_pack.bak`

5. **Validate**:

   ```text
   cargo check --features dev
   cargo test --features dev --lib data_import
   ```

## Single monster (debug)

```text
blender --background --python tools/monster_pack/blender_export_monster.py -- ^
  --source-dir "<monster folder>" ^
  --monster-name Tyranopode ^
  --anim-prefix Tyranopode ^
  --sk-fbx "<path to SK_*.FBX>" ^
  --output assets/units/tyranopode.glb
```

Use `animation_prefix` and `sk_fbx` from `inventory.json` when names differ from the folder name.

## Known limits

Some pack `SK_*.FBX` meshes fail Blender's FBX importer (`mesh.armature_setup` KeyError). Those monsters are skipped by the batch script; see `docs/monster-pack-import-manifest.md` for status.
