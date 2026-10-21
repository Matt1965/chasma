# Monster pack import fidelity contract

Policy: see **Imported asset fidelity** in `AGENTS.md`.

Forensics and target process: `diagnostics/bufomorph/INDEPENDENT_REVIEW.md`.

## Production export path (every pack creature)

```text
export_unity_monster.ps1  →  export_monster_fbx2gltf.py
```

Steps (generic, no per-creature Blender branches):

1. **FBX2glTF** — SK and every clip FBX → GLB (same converter, same space).
2. **glTF merge** — `gltf_merge.merge_clips_into_sk` copies channels by node name and **STEP-fills** un-keyed skin joints from each clip file's node defaults when they differ from the SK bind (Unity poses clip-rig rest for un-keyed bones; required for jaw/tongue).
3. **Skin influences** — fold `JOINTS_n`/`WEIGHTS_n` into one normalized four-influence set (`gltf_skin_influences.py`).
4. **Timeline** — `rebase_gltf_animation_starts_to_zero` only.
5. **Materials** — Unity prefab resolve + `gltf_material_apply.py`.
6. **Validate** — `export_validate.py` (weight sum, translation invariance, clip completeness).

Do **not** post-edit inverse bind matrices. Do **not** apply `scene_root_x90` on FBX2glTF SK exports (SK is already Y-up; that roll was compensating a broken Blender round-trip).

`monster_export_core.py` / Blender remain a **fallback** for assets FBX2glTF cannot read; they are not the production path for shipped pack units.

## Runtime presentation (catalog, not conversion hacks)

- **Rotation correction** on the Units sheet (`asset_sizing.rotation_correction`): gameplay forward-axis adaptation only (typical pack: **Y=180**). If a **Z roll** is required to stand the model up, the GLB bind is still wrong — fix export first, then remove the roll.
- **Bufomorph (U-0029)** with the FBX2glTF GLB: measured upright with catalog **Y=180, Z=90** until bind hierarchy matches cavecrawler-style SK orientation (see `monster_orientation.verify_presentation_upright`).
- **ModelComposition** offset Y: ground contact, not jaw/tongue repair.

## Retired workarounds (do not reintroduce)

| Removed | Why |
|---|---|
| `gltf_unify_skin` on shipped assets | Masked per-mesh IBM conflicts; fix bind at export |
| `align_bufomorph_body_bind_pose`, tongue tuck, locomotion strip | Symptom suppression |
| `apply_fbx2gltf_scene_root_x90` on faithful FBX2glTF SK | Rolls an already-upright rig |
| Blender `merge_action` without clip-rig fill | Drops closed-mouth reference pose |
