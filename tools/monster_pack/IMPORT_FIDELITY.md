# Monster pack import fidelity contract

Policy: see **Imported asset fidelity** in `AGENTS.md`.

## Faithful export path (default)

1. **Blender merge** — `monster_export_core.export_monster_glb`: import SK + per-clip FBX, merge actions, texture bind, `export_scene.gltf` (GLB).
2. **No Blender fcurve stripping** unless `--strip-child-channels` (legacy pipelines only).
3. **GLB post-export** — `finish_glb_post_export`:
   - **Always:** `rebase_gltf_animation_starts_to_zero` (NLA timeline offset is a demonstrated exporter defect; does not remove motion).
   - **FBX2glTF SK fallback only:** `apply_fbx2gltf_scene_root_x90` (Godot FBX2glTF drops Blender `io_scene_fbx` armature +90° X; see tyranopode/muscomorph working GLBs).
4. **Runtime presentation** — Units sheet rotation correction and `ModelComposition` offsets (ADR-128), not internal bone rotation hacks.

## Opt-in channel stripping (authorized custom content only)

`glb_sanitize_animations.py --strip-child-channels` removes child-bone **translation** (except named root bones) and all **scale** animation channels.

This is **not** a routine import step. It is **behavior-changing** and permitted only when:

1. A pipeline audit documents erroneous exporter data (e.g. cavecrawler direct converter — see `tyranopode/ANIMATION_REPORT.md` on the agent worktree), and
2. The change is recorded as **authorized custom content** in that audit (not as default monster-pack export).

Default `monster_export_core` / `blender_export_monster.py` must keep `strip_child_channels=false`.

Each export writes `<output>.export_manifest.json` with per-FBX outcomes and a `complete` flag.

## Removed workarounds (do not reintroduce)

| Operation | Was | Why removed |
|-----------|-----|-------------|
| `tuck_skinned_tongue_chain` | Tongue scale collapse + strip tongue channels | Hides bind-pose / clip fidelity issues |
| `align_armature_root_bone` / `align_bufomorph_body_bind_pose` | Force `root` / `Bufomorph_` rest rotation | Upright without conversion evidence |
| `strip_root_motion_from_locomotion_clips` | Drop root/body rot/trans on idle/locomotion | Compensated for bind hacks; deletes authored motion |
| `fix_bufomorph_glb` post-pass | Combined above + scene sink | Superseded by faithful export + runtime offset |
| Default `sanitize_file` on export | Strip translation/scale channels | No per-asset evidence on bufomorph path |

## Repair map (evidence-driven)

| Symptom | Owner | Repair |
|---------|-------|--------|
| Clips start at NLA offset (~10s) | `rebase_gltf_animation_starts_to_zero` | Shift keys to t=0 |
| SK import fails in Blender; FBX2glTF used | `apply_fbx2gltf_scene_root_x90` | Restore armature object +90° X on scene root |
| Creature sideways in game | Units sheet + spawn composition | Catalog yaw/pitch/roll; not deform bone edits |
| Feet floating / buried | `ModelComposition` offset Y | Diagnostic: `bufomorph_glb_helpers.print_recommended_model_offset_y` |
| Jaw track shorter than idle clip | Re-export from pack | Death-paste guard in `validate_bufomorph_idle_jaw_timing` |
| Vertex explosion after IBM edit | Re-export from Blender | Never post-edit inverse bind matrices |

## Diagnostics

Checkpoints and headless renders: `run_bufomorph_diagnostics.ps1` writes under `tools/monster_pack/diagnostics/` (not shipped in `assets/`).

Verify orientation separately from mouth/tongue/jaw animation.
