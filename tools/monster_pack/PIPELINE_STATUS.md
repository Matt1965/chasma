# Monster pack pipeline status (feature/monster-import-fidelity)

## Bufomorph (U-0029)

| Item | Status |
|------|--------|
| Unique clips (21) | Complete — see `diagnostics/bufomorph/CLIP_INVENTORY.md` |
| Export manifest | `assets/units/bufomorph.export_manifest.json` (`complete: true`) |
| Orientation | Workbook Y=180 insufficient; measured Z=90 required — `diagnostics/bufomorph/ORIENTATION_REPORT.md` |
| Source vs GLB renders | `diagnostics/bufomorph/SOURCE_COMPARISON.md` |
| Runtime preview | Dev catalog placement (`preview_model` + `unit_visual_rotation`) after workbook reload |

## Pack-wide recovery

See `diagnostics/AFFECTED_ASSETS_RECOVERY.md` and `audit_glb_animation_fidelity.py`.

## Tools

- `blender_audit_clip_imports.py` — per-FBX import audit
- `blender_export_monster.py` — faithful export + manifest
- `audit_glb_animation_fidelity.py` — detect stripped exports
