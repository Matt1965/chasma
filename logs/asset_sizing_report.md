# Asset Sizing Report

## Building / barn
- asset: `assets\buildings\barn.glb`
- source: 3.625 × 4.280 × 8.621 m (Some(CombinedVisibleMeshes))
- baseline scale: 2.207, 1.028, 0.696
- warning: root node scale (1.812, 1.158, 4.269) is not identity
- warning: inferred desired meters from building footprint — add Desired*M to Excel for explicit authoring

## Building / hut
- asset: `assets\buildings\hut.glb`
- source: 8.400 × 3.450 × 8.400 m (Some(CombinedVisibleMeshes))
- baseline scale: 0.476, 0.870, 0.476
- warning: inferred desired meters from building footprint — add Desired*M to Excel for explicit authoring

## Building / smelter
- asset: `assets\buildings\smelter.glb`
- warning: building footprint sizing inference failed: asset not found: assets\buildings\smelter.glb
- warning: AT1: catalog lacks metric sizing — runtime presentation may be microscopic or enormous until migrated
- error: AT1 MissingSizingData: no Desired meters and no explicit baseline — using scale 1.0; author Desired*M or ExplicitBaselineScale and re-import

## Building / storage_chest
- asset: `assets\buildings\chest.glb`
- source: 1.200 × 0.822 × 0.636 m (Some(CombinedVisibleMeshes))
- baseline scale: 0.833, 1.034, 1.259
- warning: source bounds rescaled ÷1000 (suspected non-meter export vs 1.00 m desired)
- warning: inferred desired meters from building footprint — add Desired*M to Excel for explicit authoring

## Doodad / d_0001
- asset: `assets\doodads\tree/oak.glb`
- source: 4.041 × 6.737 × 4.346 m (Some(CombinedVisibleMeshes))
- baseline scale: 1.187, 1.187, 1.187
- warning: inferred desired meters from doodad kind height — add Desired*M to Excel for explicit authoring
- warning: visual XZ half-extent (~2.58 m) differs from collision radius (1.00 m) — author collision meters to match desired size

## Unit / U-0001
- asset: `assets\units\robot.glb`
- source: 0.524 × 0.996 × 0.809 m (Some(CombinedVisibleMeshes))
- baseline scale: 1.194, 1.194, 1.194
- warning: inferred desired meters from unit height hint — add Desired*M to Excel for explicit authoring

## Unit / U-0002
- asset: `assets\units\fox.glb`
- baseline scale: 1.000, 1.000, 1.000
- warning: unit height hint sizing inference failed: baseline scale out of allowed range
- warning: using legacy explicit render scale — add Desired dimensions to migrate

