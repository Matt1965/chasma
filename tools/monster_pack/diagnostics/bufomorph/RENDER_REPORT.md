# Bufomorph render comparison (2026-10-07)

## Setup

- **Blender:** 4.2.3 LTS, `BLENDER_EEVEE_NEXT`, 512px, sun + area light.
- **Framing:** outer wrapper rotation `(0, 90, 0)` degrees only (no internal bone edits).
- **Clips/times:** `IdleBreathe` @ 0.0s, 1.0s, 1.5s; `BiteAttack` @ 0.35s.
- **PNGs:** `tools/monster_pack/diagnostics/bufomorph/renders/`

## Checkpoints

| File | Producer | Notes |
|------|----------|-------|
| `04_previous_shipped.glb` | Old pipeline + `fix_bufomorph_glb` / tuck / strip | 11,669,176 B |
| `01_blender_gltf_export.glb` | Blender `export_scene.gltf` only | 11,840,468 B |
| `03_faithful_postexport.glb` | + rebase NLA starts + `scene_root_x90` (FBX2glTF SK path) | 11,840,528 B |

## Channel evidence (IdleBreathe)

| Checkpoint | Tongue scale | Idle tongue channels | Bufomorph_ rot channels |
|------------|--------------|----------------------|-------------------------|
| previous_shipped | 0.0001 (collapsed) | 0 | 0 (stripped) |
| 01_raw / 03_faithful | ~1.0 | 27 | 1 |

## Earliest verified divergence

1. **Blender export (01) vs faithful post (03):** only timeline rebase and scene +90 X; no meaningful pose change at matched in-clip times (byte delta minimal; renders align).
2. **Faithful export (01/03) vs previous shipped (04):** diverges immediately at **IdleBreathe t=0** — tongue visible/collapsed and body orientation differ. Responsible operations: **`tuck_skinned_tongue_chain`**, **`align_*` bind patches**, **`strip_root_motion_from_locomotion_clips`** on the old fix pass (not present in faithful path).

## Orientation (separate from mouth)

`verify_presentation_upright` on faithful GLB: **ok=False** (catalog 180 Y yields up ~(-1,0,0) without body bind hack). Upright presentation remains a **runtime/catalog** problem, not solved by reintroducing bone overrides.

## Export caveats

- SK import uses **FBX2glTF** fallback (Blender FBX SK `KeyError` on pack mesh).
- Some animation FBX imports logged `io_scene_fbx` errors; export merged **21** clips (not full 42-file manifest). Re-run failed clips after fixing FBX import if full attack set is required.

## Blocked checks

- **Unity** idle reference render: not run in this pass (Unity project not automated here).
- **Chasma runtime** mouth/idle verification: **MANUAL RUNTIME VERIFICATION PENDING** after deploying faithful `bufomorph.glb`.
