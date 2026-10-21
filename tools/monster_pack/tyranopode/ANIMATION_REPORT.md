# Tyranopode (U-0006) — animation investigation

## Symptom

- Stretching / vertex spikes when locomotion clips play (some exports).
- Frozen T-pose or whole-model sliding (pose-bake export).

## Root cause (this asset)

`SK_Tyranopode.FBX` imports with **two** centimeter-scale factors:

| `global_scale` | Armature object scale | Mesh height (approx.) |
|----------------|----------------------|------------------------|
| 0.01 (Cavecrawler default) | **1e-4** | 0.03 m |
| 1.0 (Tyranopode fix) | 0.01 before apply | 3.2 m |

Merging animations imported at one effective scale onto a bind pose at another breaks skinning (stretch). Pose-baking without fixing scale produced nonsense keys (~108 m translations, 0 s duration).

Cavecrawler SK does **not** have this double-scale object hierarchy; the pack-wide `FBX_SCALE=0.01` pipeline is not valid for every monster without per-asset probing.

## Fix (Tyranopode only)

Script: `tools/monster_pack/tyranopode/export_tyranopode.py`

1. Import SK and every `Tyranopode@*.FBX` with **`global_scale=1.0`**.
2. **`apply_object_scale`** on each armature (bake object scale to 1) **before** merging actions.
3. Direct **action merge** (Cavecrawler style) — no pose bake, no `PresentationRoot`.
4. Unity textures via `unity_materials.py`.

Re-export:

```text
blender --background --python tools/monster_pack/tyranopode/export_tyranopode.py
```

Validate clip list:

```text
py -3 tools/monster_pack/glb_clips.py assets/units/tyranopode.glb
```

## Workbook (unchanged)

| Field | Value |
|-------|--------|
| Unit | U-0006 |
| Idle | `Idle` |
| Walk / Run | `CrawlForward` |
| Turn L/R | `CrawlLeft` / `CrawlRight` |
| Hit / Death | `GetHit` / `Death` |
| Attack | `JumpTentaclesSmashAttack` |

Use **non-`_RM`** clips for locomotion in profiles.

## Next step (after you verify Tyranopode)

Probe `armature.scale` after `global_scale=0.01` SK import per monster; only then generalize batch export.
