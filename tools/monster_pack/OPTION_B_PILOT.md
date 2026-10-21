# Monster pack export (Bufomorph + Muscomorph pilot)

**Runtime worktree:** `C:\BevyFiles\chasma-agent-c`  
**Owner run command:**

```text
cd C:\BevyFiles\chasma-agent-c
cargo run --features dev
```

## Production path (generic — no per-creature exceptions)

1. `export_unity_monster.ps1` runs Blender headless.
2. `monster_export_core.py` imports the SK FBX (Blender FBX; FBX2glTF SK only if that import has no armature/mesh).
3. Each clip FBX is imported, **object scale is applied, and pose-bone location fcurves are multiplied by that same scale** (Unity cm → metres). This is the pack-wide converter fix; it is not a Bufomorph special case.
4. Clips are renamed onto the SK armature. No channel strip, tongue tuck, root-motion strip, or IBM rewrite.
5. `rebase_gltf_animation_starts_to_zero` only (NLA start offset).
6. Scene-root +90 X only when the SK itself came through FBX2glTF (Blender FBX already applied armature orientation).

Entry: `export_unity_monster.ps1`  
Pilot both creatures: `run_option_b_pilot.ps1`

## Why FBX2glTF glTF-merge is not the production path

`export_monster_fbx2gltf.py` keeps FBX2glTF’s per-mesh skins. Those skins disagree on inverse bind matrices for the same bone (`Muscomorph_Body`, `Muscomorph_WingLeft_A`). Bevy then evaluates `joint_global * IBM` with the wrong IBM. Live `logs/dev_startup.log` skin probe on that path:

- wings / tail: `max_dev=0.000m`
- body: `max_dev=42m`
- horns: `max_dev=14m`
- bufomorph: `max_dev=95m` (joints at scale 1.507, IBM at 1.0)

That is an exporter/bind contract failure, not a defect in the paid Unity files.

`gltf_unify_skin.py` / clip rest-anchoring are not used on the production path.

## Manual check

1. Spawn Muscomorph (U-0007) and Bufomorph (U-0029): body is a creature, not a ribbon.
2. `logs/dev_startup.log` `unit animation identity` matches the new GLB size.
3. Skin probe lines: body `max_dev` on the order of the mesh (centimetres to a couple of metres for authored motion), not tens of metres of spikes.
