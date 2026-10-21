# Monsters Full Pack Vol 1 — import manifest

Source (external, not in repo): `C:\Users\matt1\My project\Assets\Monsters Full Pack Vol 1`

Branch: `agent-c/monster-pack-import`  
Pipeline: `tools/monster_pack/` (see README there)

## Summary

| Metric | Count |
|--------|------:|
| Source monsters (excl. Cavecrawler) | 29 |
| GLB exported | 14 |
| Workbook-integrated units (U-0006–U-0019) | 14 |
| Blocked (Blender SK FBX import) | 15 |

## Integrated units

| Monster | Unit ID | GLB | Animation profile | Default weapon | Primary attack clip |
|---------|---------|-----|-------------------|----------------|---------------------|
| Tyranopode | U-0006 | `assets/units/tyranopode.glb` | `tyranopode` | `weapon_tyranopode_primary` | `JumpTentaclesSmashAttack` |
| Muscomorph | U-0007 | `assets/units/muscomorph.glb` | `muscomorph` | `weapon_muscomorph_primary` | `BiteAttack` |
| Telluropod | U-0008 | `assets/units/telluropod.glb` | `telluropod` | `weapon_telluropod_primary` | (see workbook) |
| Clypeosaurus | U-0009 | `assets/units/clypeosaurus.glb` | `clypeosaurus` | `weapon_clypeosaurus_primary` | (see workbook) |
| Giant Slug | U-0010 | `assets/units/giant_slug.glb` | `giant_slug` | `weapon_giant_slug_primary` | (see workbook) |
| Karcinomorph | U-0011 | `assets/units/karcinomorph.glb` | `karcinomorph` | `weapon_karcinomorph_primary` | (see workbook) |
| Scyver | U-0012 | `assets/units/scyver.glb` | `scyver` | `weapon_scyver_primary` | (see workbook) |
| Scolomorph | U-0013 | `assets/units/scolomorph.glb` | `scolomorph` | `weapon_scolomorph_primary` | (see workbook) |
| Tetrachnide | U-0014 | `assets/units/tetrachnide.glb` | `tetrachnide` | `weapon_tetrachnide_primary` | (see workbook) |
| Vespomorph | U-0015 | `assets/units/vespomorph.glb` | `vespomorph` | `weapon_vespomorph_primary` | (see workbook) |
| Morphorrid | U-0016 | `assets/units/morphorrid.glb` | `morphorrid` | `weapon_morphorrid_primary` | (see workbook) |
| Carcidonte | U-0017 | `assets/units/carcidonte.glb` | `carcidonte` | `weapon_carcidonte_primary` | (see workbook) |
| Entomorane | U-0018 | `assets/units/entomorane.glb` | `entomorane` | `weapon_entomorane_primary` | (see workbook) |
| Laminferox | U-0019 | `assets/units/laminferox.glb` | `laminferox` | `weapon_laminferox_primary` | (see workbook) |

Clip names are exact GLB names in `tools/monster_pack/import_manifest.json`. Locomotion uses non-`_RM` clips where both exist.

**Animation export:** Skeleton scale is baked to final size **before** animation FBXs are merged. Merging animations first and rescaling after caused skinning spikes / vertical stretching in-game.

## Placeholder gameplay data

All new units use **wild** faction, per-monster **Species Key** (sheet row added), and shared placeholder stats from `integrate_monster_pack_workbook.py` (`Level` 3, `Base HP` 40, `Move Speed` 3.5, etc.). Weapon damage/range are placeholders. **Rotation Correction Y Deg** 180 and **Turn Speed Deg/s** 360 unless tuned per asset.

## Blocked monsters (SK mesh FBX)

Blender 4.2.3 `import_scene.fbx` fails on these `SK_*.FBX` files with `mesh.armature_setup` / `KeyError: None` (see logs under `C:\BevyFiles\Temp\monster-pack-convert\`).

| Monster | SK FBX (from inventory) |
|---------|---------------------------|
| Gorosaurus | `...\Gorosaurus\FBX Files\SK_Gorosaurus.FBX` |
| Hellcreeper | `...\Hellcreeper\FBX Files\SK_Hellcreeper.FBX` |
| Marhomorph | `...\Marhomorph\FBX Files\SK_Marhomorph.FBX` |
| Arthromahre | `...\Arthromahre\FBX Files\SK_Arthromahre.FBX` |
| Deinodonte | `...\Deinodonte\FBX Files\SK_Deinodonte.FBX` |
| Skorpmare | `...\Skorpmare\FBX Files\SK_Skorpmare.FBX` |
| Karckmahre | `...\Karckmahre\FBX Files\SK_Karckmahre.FBX` |
| Hideoplast | `...\Hideoplast\FBX Files\SK_Hideoplast.FBX` |
| Letalobrach | `...\Letalobrach\FBX Files\SK_Letalobrach.FBX` |
| Bufomorph | `...\Bufomorph\FBX Files\SK_Buformorph.FBX` |
| Drackmahre | `...\Drackmahre\FBX Files\SK_Drackmahre.FBX` |
| Pardathrox | `...\Pardathrox\FBX Files\SK_Pardathrox.FBX` |
| Perderos | `...\Perderos\FBX Files\SK_Perderos.FBX` |
| Densoptere | `...\Densoptere\FBX Files\SK_Densoptere.FBX` |
| Gryllunguis | `...\Gryllunguis\FBX Files\SK_Gryllunguis.FBX` |

**Note:** Clypeosaurus exported successfully in the final batch despite an earlier failed probe when the wrong SK filename was used.

**Next step for blocked assets:** Re-export SK meshes from Unity (or use an alternate converter such as FBX2glTF / Unity GLB export) using the same Cavecrawler merge workflow, then re-run `refresh_import_manifest.py` and workbook integration for those entries only.

## Validation

- `cargo check --features dev` — pass
- `cargo test --features dev --lib runtime_catalog_tests` — pass (dev workbook unit import path)
- Full Dev Catalog spawn pass for all 14 units — **MANUAL RUNTIME VERIFICATION PENDING**
