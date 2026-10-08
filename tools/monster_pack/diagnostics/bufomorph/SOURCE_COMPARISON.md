# Source vs Chasma export comparison (Bufomorph)

## Reference tiers

| Tier | Files | Role |
|------|-------|------|
| **Pack source** | `Buformorph@IdleBreathe.FBX`, `Buformorph@BiteAttack.FBX` | Unity pack animation FBX (authoritative motion) |
| **Faithful GLB** | `assets/units/bufomorph.glb` | Blender merge + rebase + FBX2glTF `scene_root_x90` |
| **Diagnostic GLB stages** | `checkpoints/01_*`, `03_*`, `04_previous_shipped.glb` | Pipeline forensics only—not Unity |
| **Previous shipped** | `04_previous_shipped.glb` | Old workaround pipeline (tongue collapse, channel strip) |

## Headless renders (same framing)

Outer rotation `(0, 90, 0)` degrees on wrapper only — `diagnostics/bufomorph/renders/`:

| PNG | Source |
|-----|--------|
| `source_pack_IdleBreathe_t0.png` | Pack FBX `IdleBreathe` @ 0s |
| `source_pack_BiteAttack_t0.35.png` | Pack FBX `BiteAttack` @ 0.35s |
| `checkpoint_03_faithful_IdleBreathe_t0.00.png` | Faithful GLB idle |
| `checkpoint_03_faithful_BiteAttack_t0.35.png` | Faithful GLB bite |
| `previous_shipped_*` | Workaround GLB (not source) |

## Expected differences

- **Faithful GLB vs pack FBX:** should match pose/motion closely when clip times align; material paths may differ (missing textures in Blender batch).
- **Previous shipped vs pack FBX:** diverges on tongue visibility and body orientation channels (workarounds removed).
- **Faithful GLB vs Unity player:** not automated here; use Unity editor on the same FBX clips for final sign-off.
