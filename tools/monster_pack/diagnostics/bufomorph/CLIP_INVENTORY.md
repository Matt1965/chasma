# Bufomorph clip inventory (U-0029)

## Manifest accounting

| Metric | Value |
|--------|------:|
| FBX files on disk (inventory list) | 42 |
| Duplicate manifest rows | 21 (each clip listed twice in `inventory.json`) |
| **Unique clips expected** | **21** |
| Unique clips in shipped `bufomorph.glb` | 21 |
| Per-file Blender import audit (`CLIP_IMPORT_AUDIT.json`) | 42/42 `import_ok` |

## Outcome

The export is **complete for unique clips**. The earlier “21 vs 42” gap was a **duplicate inventory count**, not missing motion.

Export logs may still show Blender `io_scene_fbx` errors during **SK** import before the FBX2glTF fallback; animation FBX files import cleanly in isolation.

## Clip list (unique)

`BiteAttack`, `BiteAttack_RM`, `Death`, `GetHitBack`, `GetHitFront`, `GetHitLeft`, `GetHitRight`, `HopBackwards`, `HopBackwards_RM`, `HopForward`, `HopForward_RM`, `HopLeft`, `HopLeft_RM`, `HopRight`, `HopRight_RM`, `IdleBreathe`, `IdleLookAround`, `Run`, `Run_RM`, `SpitAttack`, `TongueAttack`.

Shipped GLB manifest: `assets/units/bufomorph.export_manifest.json` (written on next export).
