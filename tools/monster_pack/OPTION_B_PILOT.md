# Option B pilot (FBX2glTF + glTF merge)

**Runtime worktree:** `C:\BevyFiles\chasma-agent-c`  
**Branch:** `agent-c/monster-pack-import`  
**Owner run command:**

```text
cd C:\BevyFiles\chasma-agent-c
cargo run --features dev
```

Confirm the window title shows `agent-c/monster-pack-import @ <sha>` and the log line  
`unit animation identity: ... workbook ... manifest_dir C:\BevyFiles\chasma-agent-c`.

## Pipeline

1. `fbx2gltf_util.py` — FBX2glTF **0.13.1** (`tools/monster_pack/bin/`), `--binary`, same settings for SK and clips.
2. `gltf_merge.py` — copy channels by node name; fill un-keyed skin joints from **clip file** node defaults (STEP).
3. `rebase_gltf_animation_starts_to_zero` — timeline repair only.
4. `gltf_unify_skin.py` — all skinned mesh nodes share the fullest skeleton skin (fixes multi-skin FBX2glTF wing/tail spikes in Bevy).
5. `unity_material_resolve.py` + `gltf_material_apply.py` — prefab `.mat` / texture GUIDs; multi-slot when FBX2glTF SK already has named materials.
6. `export_validate.py` — translation ratio diagnostic, upright rest pose, clip completeness, sampled source agreement.

Entry point: `export_monster_fbx2gltf.py`  
Batch pilot: `run_option_b_pilot.ps1`

Each output writes `<name>.export_manifest.json` (sources, merge skips, material slots, validation).

## Pilot creatures (regenerated)

| Creature | GLB | Bytes (approx) | Catalog rotation |
|----------|-----|----------------|------------------|
| Bufomorph | `assets/units/bufomorph.glb` | 15,037,712 | Y=180 only (Z cleared) |
| Muscomorph | `assets/units/muscomorph.glb` | 68,493,104 | Y=180 (unchanged) |

Previous shipped bufomorph retained as `assets/units/bufomorph.glb.pre_option_b_backup` when present.

**Not regenerated:** `cavecrawler.glb` (regression reference).

## Blender merge path

`monster_export_core.py` / `blender_export_monster.py` remain for batch recovery and cavecrawler-style assets until pilot review; **do not** use them for new pack exports.

## Remaining pack

~30 creatures still on the Blender merge path (`import_manifest.json`). Regenerate only after pilot sign-off.

## Manual runtime verification (owner)

1. Spawn Bufomorph (U-0029): idle mouth closed, attack lunge without underground flip, textures visible.
2. Spawn Muscomorph (U-0007): crawl/fly clips, wing + body materials.
3. Spot-check human layered animation and Cavecrawler locomotion unchanged.

Log `unit animation identity` must show `fnv1a64` matching the current `.export_manifest.json` `output_sha256` prefix check (use manifest `output_sha256` full hash vs file).
