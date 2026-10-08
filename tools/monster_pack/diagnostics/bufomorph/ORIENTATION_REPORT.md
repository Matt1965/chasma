# Bufomorph orientation diagnosis

## Layers compared

| Layer | What was measured | Result |
|-------|---------------------|--------|
| Pack source FBX | Blender import + bind | SK uses FBX2glTF fallback; armature object +90° X applied in Blender after glTF import |
| Faithful GLB | `Bufomorph_` bind + scene `scene_root_x90` | Bone local +Y maps to world **+X** (not +Y) with **no** catalog correction |
| Units workbook (U-0029) | Rotation Correction Y = **180** only | `verify_presentation_upright` → **false** (visual up ≈ **-X**) |
| Measured catalog fix | Same Y=180, add **Roll Z = 90** (YXZ, matches `QuantizedOrientation`) | Visual up ≈ **+Y** (`presentation_up` probe on faithful GLB) |

## Conclusion

The faithful GLB is **not** belly-up because of removed bone hacks; it is **sideways in Chasma +Y-up** when only workbook **Y=180** is applied. A **definition-level rotation correction** (Units sheet pitch/yaw/roll → `unit_visual_rotation`) is the correct owner—not internal bone edits.

**Roll Z = 90** with existing **Y = 180** is the minimal measured correction for this GLB bind chain. It is bind-pose constant, so it applies consistently across clips (locomotion no longer strips root/body channels).

## Runtime verification (owner)

Worktree: `C:\BevyFiles\chasma-agent-c` (or `chasma` on `feature/monster-import-fidelity`).

1. Ensure Units row **U-0029** includes measured roll if authoring agrees (see workbook note in commit).
2. `cargo run --features dev` → Dev placement / catalog model preview for bufomorph (uses `unit_visual_rotation` + `unit_effective_model_offset`).
3. **Expect:** creature standing on +Y with feet toward ground plane; idle mouth/tongue may still differ from Unity until source comparison passes.

Do not treat `verify_presentation_upright` with Y=180 alone as proof of catalog correctness.
