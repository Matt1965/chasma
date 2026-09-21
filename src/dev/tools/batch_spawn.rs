//! Batch spawn execution via WorldData APIs (ADR-044).

use bevy::prelude::*;

use crate::world::{
    BuildingArchetypeCatalog, BuildingArchetypeId, BuildingCatalog, BuildingLifecycleState,
    BuildingNavigationBlueprintCatalog, BuildingOwnership, BuildingPlacementConfig,
    BuildingPlacementContext, BuildingSource, DoodadCatalog, DoodadPlacementOverrides,
    DoodadSource, FootprintCatalog, InteriorProfileCatalog, InventoryCatalogCtx, ItemCatalog,
    OccupancyCatalogs, UnitArchetypeCatalog, UnitArchetypeId, UnitCatalog, UnitSource, WorldData,
    WorldPosition, apply_unit_archetype_spawn_overrides, create_dev_complete_building,
    create_dev_complete_building_with_inventory, create_doodad, create_unit_with_inventory,
    apply_building_archetype_placement, definition_requires_inventory_allocation,
    place_player_building, place_player_building_with_inventory,
    remove_building, resolve_authoritative_building_placement, resolve_building_spawn_spec,
    resolve_unit_spawn_spec, try_activate_interior_if_complete, BuildingArchetypeReconstructCtx,
    OperationCatalog,
};

use super::super::dev_mode::DefinitionId;
use super::brush::{
    BrushPointBuffer, BrushSettings, MAX_BRUSH_SPAWN_COUNT, generate_brush_positions,
};
use super::placement_rules::{
    PlacementRules, PlacementValidateContext, PlacementValidation, validate_placement,
};

/// Request to place many instances from one brush click.
#[derive(Debug, Clone, PartialEq)]
pub struct BatchSpawnRequest {
    pub definition: DefinitionId,
    pub brush: BrushSettings,
    pub anchor: WorldPosition,
    pub line_direction: Vec2,
    pub terrain_conforming: bool,
    pub rules: PlacementRules,
    pub world_seed: u64,
    pub layout: crate::world::ChunkLayout,
    /// Runtime affiliation for dev unit spawns (O1).
    pub spawn_affiliation: crate::world::Affiliation,
    /// Initial placement yaw (degrees) for doodads/buildings (Slice 4).
    pub placement_yaw_deg: f32,
    /// Initial uniform scale for doodads/buildings when supported (Slice 4).
    pub placement_uniform_scale: f32,
    /// Terrain mesh vertical exaggeration for building terrain placement.
    pub terrain_vertical_scale: f32,
    /// Optional unit archetype preset for dev spawns.
    pub unit_archetype: Option<UnitArchetypeId>,
    /// Optional building archetype preset for dev spawns.
    pub building_archetype: Option<BuildingArchetypeId>,
}

/// Summary of a committed batch spawn.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BatchSpawnReport {
    pub attempted: u32,
    pub spawned: u32,
    pub rejected: u32,
    pub failures: u32,
}

/// Reusable scratch for batch placement (avoid per-click allocations).
#[derive(Debug, Default)]
pub struct BatchSpawnScratch {
    brush_buffer: BrushPointBuffer,
    accepted: Vec<WorldPosition>,
}

impl BatchSpawnScratch {
    pub fn clear(&mut self) {
        self.brush_buffer.clear();
        self.accepted.clear();
    }

    pub fn candidate_positions(&self) -> &[WorldPosition] {
        self.brush_buffer.positions()
    }
}

/// Plan validated positions without mutating world (preview / tests).
pub fn plan_batch_spawn(
    request: &BatchSpawnRequest,
    definition_key: &str,
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    scratch: &mut BatchSpawnScratch,
) -> (Vec<WorldPosition>, BatchSpawnReport) {
    scratch.clear();
    let mut rules = request.rules;
    rules.snap_to_terrain = request.terrain_conforming || rules.snap_to_terrain;

    generate_brush_positions(
        &request.brush,
        request.anchor,
        request.layout,
        request.line_direction,
        request.world_seed,
        definition_key,
        &mut scratch.brush_buffer,
    );

    let ctx = PlacementValidateContext {
        world,
        unit_catalog,
        doodad_catalog,
        building_catalog,
        footprint_catalog,
        definition: &request.definition,
        rules: &rules,
    };

    let mut report = BatchSpawnReport::default();
    report.attempted = scratch
        .brush_buffer
        .positions()
        .len()
        .min(MAX_BRUSH_SPAWN_COUNT as usize) as u32;

    for &candidate in scratch.brush_buffer.positions() {
        match validate_placement(&ctx, candidate, &scratch.accepted) {
            PlacementValidation::Accepted(position) => {
                scratch.accepted.push(position);
            }
            PlacementValidation::Rejected(_) => {
                report.rejected += 1;
            }
        }
    }

    let planned = scratch.accepted.clone();
    (planned, report)
}

/// Execute batch spawn — mutates [`WorldData`] only through authoring APIs.
///
/// `inventory_ctx` supplies item/inventory catalogs so definitions carrying an
/// inventory profile (unit backpacks, storage buildings) allocate their
/// containers at create time. Definitions without a profile are unaffected.
pub fn execute_batch_spawn(
    request: &BatchSpawnRequest,
    definition_key: &str,
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    unit_archetype_catalog: &UnitArchetypeCatalog,
    appearance_profiles: &crate::world::AppearanceProfileCatalog,
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
    building_archetype_catalog: &BuildingArchetypeCatalog,
    footprint_catalog: &FootprintCatalog,
    interior_catalog: &InteriorProfileCatalog,
    nav_catalog: Option<&BuildingNavigationBlueprintCatalog>,
    inventory_ctx: &InventoryCatalogCtx<'_>,
    item_catalog: &ItemCatalog,
    scratch: &mut BatchSpawnScratch,
) -> BatchSpawnReport {
    let (planned, mut report) = plan_batch_spawn(
        request,
        definition_key,
        world,
        unit_catalog,
        doodad_catalog,
        building_catalog,
        footprint_catalog,
        scratch,
    );

    for position in planned {
        let outcome = spawn_at(
            world,
            unit_catalog,
            unit_archetype_catalog,
            appearance_profiles,
            doodad_catalog,
            building_catalog,
            building_archetype_catalog,
            footprint_catalog,
            interior_catalog,
            nav_catalog,
            inventory_ctx,
            item_catalog,
            &request.definition,
            position,
            request.spawn_affiliation,
            request.placement_yaw_deg,
            request.placement_uniform_scale,
            request.terrain_vertical_scale,
            request.unit_archetype.as_ref(),
            request.building_archetype.as_ref(),
        );
        if outcome {
            report.spawned += 1;
        } else {
            report.failures += 1;
        }
    }

    report
}

fn spawn_at(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    unit_archetype_catalog: &UnitArchetypeCatalog,
    appearance_profiles: &crate::world::AppearanceProfileCatalog,
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
    building_archetype_catalog: &BuildingArchetypeCatalog,
    footprint_catalog: &FootprintCatalog,
    interior_catalog: &InteriorProfileCatalog,
    nav_catalog: Option<&BuildingNavigationBlueprintCatalog>,
    inventory_ctx: &InventoryCatalogCtx<'_>,
    item_catalog: &ItemCatalog,
    definition: &DefinitionId,
    position: WorldPosition,
    spawn_affiliation: crate::world::Affiliation,
    placement_yaw_deg: f32,
    placement_uniform_scale: f32,
    terrain_vertical_scale: f32,
    unit_archetype: Option<&UnitArchetypeId>,
    building_archetype: Option<&BuildingArchetypeId>,
) -> bool {
    match definition {
        DefinitionId::Unit(definition_id) => {
            let spec = match resolve_unit_spawn_spec(
                definition_id,
                unit_archetype,
                spawn_affiliation,
                unit_catalog,
                unit_archetype_catalog,
            ) {
                Ok(spec) => spec,
                Err(_) => return false,
            };
            match create_unit_with_inventory(
                unit_catalog,
                appearance_profiles,
                world,
                &spec.definition_id,
                position,
                UnitSource::Dev,
                spec.ownership,
                inventory_ctx,
            ) {
                Ok(record) => {
                    apply_unit_archetype_spawn_overrides(
                        world,
                        &record,
                        &spec,
                        inventory_ctx,
                        item_catalog,
                    )
                    .is_ok()
                }
                Err(_) => false,
            }
        }
        DefinitionId::Doodad(definition_id) => {
            let rotation = Quat::from_rotation_y(placement_yaw_deg.to_radians());
            let scale = Vec3::splat(placement_uniform_scale.max(0.01));
            create_doodad(
                doodad_catalog,
                world,
                definition_id,
                position,
                DoodadSource::Dev,
                DoodadPlacementOverrides {
                    rotation: Some(rotation),
                    scale: Some(scale),
                },
                None,
            )
            .is_ok()
        }
        DefinitionId::Building(definition_id) => {
            let spec = match resolve_building_spawn_spec(
                definition_id,
                building_archetype,
                spawn_affiliation,
                building_catalog,
                building_archetype_catalog,
            ) {
                Ok(spec) => spec,
                Err(_) => return false,
            };
            let yaw_deg = spec
                .snapshot
                .as_ref()
                .map(|snapshot| snapshot.placement_yaw_deg)
                .unwrap_or(placement_yaw_deg);
            let rotation = Quat::from_rotation_y(yaw_deg.to_radians());
            let ownership = spec.ownership;
            let placement_ctx = BuildingPlacementContext {
                world,
                building_catalog,
                footprint_catalog,
                doodad_catalog,
                unit_catalog,
                config: BuildingPlacementConfig::default(),
                player_authorized: true,
                terrain_vertical_scale,
            };
            let validation = resolve_authoritative_building_placement(
                &placement_ctx,
                &spec.definition_id,
                position,
                rotation,
                ownership,
            );
            if !validation.valid {
                return false;
            }
            let grounded = validation.grounded_anchor.unwrap_or(position);
            let resolved_rotation = validation.resolved_rotation.unwrap_or(rotation);
            let occupancy = OccupancyCatalogs {
                doodad: doodad_catalog,
                building: building_catalog,
                footprint: footprint_catalog,
            };
            let spawned = if spec.lifecycle_state == BuildingLifecycleState::Planned {
                if building_catalog
                    .get(&spec.definition_id)
                    .is_some_and(definition_requires_inventory_allocation)
                {
                    place_player_building_with_inventory(
                        building_catalog,
                        world,
                        &spec.definition_id,
                        grounded,
                        resolved_rotation,
                        ownership,
                        occupancy,
                        inventory_ctx,
                    )
                } else {
                    place_player_building(
                        building_catalog,
                        world,
                        &spec.definition_id,
                        grounded,
                        resolved_rotation,
                        ownership,
                        occupancy,
                    )
                }
            } else if building_catalog
                .get(&spec.definition_id)
                .is_some_and(definition_requires_inventory_allocation)
            {
                create_dev_complete_building_with_inventory(
                    building_catalog,
                    world,
                    &spec.definition_id,
                    grounded,
                    resolved_rotation,
                    ownership,
                    Some(occupancy),
                    inventory_ctx,
                )
            } else {
                create_dev_complete_building(
                    building_catalog,
                    world,
                    &spec.definition_id,
                    grounded,
                    resolved_rotation,
                    ownership,
                    Some(occupancy),
                )
            };
            match spawned {
                Ok(record) => {
                    if let Some(archetype) = &spec.archetype {
                        let operation_catalog = OperationCatalog::default();
                        let reconstruct_ctx = BuildingArchetypeReconstructCtx {
                            building_catalog,
                            doodad_catalog,
                            item_catalog,
                            operation_catalog: &operation_catalog,
                            interior_catalog,
                            inventory_ctx,
                            occupancy,
                            nav_catalog,
                            created_tick: 0,
                        };
                        if apply_building_archetype_placement(
                            world,
                            record.id,
                            archetype,
                            &reconstruct_ctx,
                        )
                        .is_err()
                        {
                            let _ = remove_building(
                                world,
                                record.id,
                                Some(occupancy),
                                Some(building_catalog),
                                Some(doodad_catalog),
                                None,
                                None,
                            );
                            return false;
                        }
                    } else if let Some(snapshot) = &spec.snapshot {
                        if snapshot.container_locked {
                            let _ = crate::world::set_building_container_locked(
                                world,
                                record.id,
                                true,
                            );
                        }
                        if (snapshot.uniform_scale - 1.0).abs() > 0.001 {
                            if let Ok(scale) =
                                crate::world::FixedScale::from_f32(snapshot.uniform_scale)
                            {
                                world.mutate_building(record.id, |building| {
                                    building.placement.uniform_scale = scale;
                                });
                            }
                        }
                        let _ = try_activate_interior_if_complete(
                            world,
                            building_catalog,
                            interior_catalog,
                            doodad_catalog,
                            occupancy,
                            nav_catalog,
                            record.id,
                        );
                    } else {
                        let _ = try_activate_interior_if_complete(
                            world,
                            building_catalog,
                            interior_catalog,
                            doodad_catalog,
                            occupancy,
                            nav_catalog,
                            record.id,
                        );
                    }
                    true
                }
                Err(_) => false,
            }
        }
        DefinitionId::Item(_) | DefinitionId::InventoryProfile(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dev::tools::brush::BrushMode;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadDefinitionId, Heightfield,
        InteriorProfileCatalog, InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog,
        LocalPosition, UnitDefinitionId, starter_inventory_profile_definitions,
        starter_item_category_definitions, starter_item_definitions,
    };

    fn item_catalogs() -> (ItemCategoryCatalog, ItemCatalog, InventoryProfileCatalog) {
        let categories =
            ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
        let items = ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
        let profiles =
            InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
                .unwrap();
        (categories, items, profiles)
    }

    fn flat_world() -> WorldData {
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let mut world = WorldData::new(layout);
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn anchor() -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(30.0, 0.0, 30.0)),
        )
    }

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    /// Terrain grid aligned with building placement validation (128 m chunk, 64 m spacing).
    fn building_placement_world() -> WorldData {
        let layout = ChunkLayout {
            chunk_size_meters: 128.0,
            units_per_meter: 1.0,
        };
        let mut world = WorldData::new(layout);
        let heightfield = Heightfield::from_samples(3, 64.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn building_placement_layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 128.0,
            units_per_meter: 1.0,
        }
    }

    fn building_placement_anchor() -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(64.0, 0.0, 64.0)),
        )
    }

    #[test]
    fn batch_spawn_produces_expected_world_data_entries() {
        let mut world = flat_world();
        let unit_catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let building_catalog = BuildingCatalog::default();
        let request = BatchSpawnRequest {
            definition: DefinitionId::Unit(UnitDefinitionId::new("wolf")),
            brush: BrushSettings {
                mode: BrushMode::Line,
                count: 4,
                spacing: 3.0,
                ..Default::default()
            },
            anchor: anchor(),
            line_direction: Vec2::X,
            terrain_conforming: true,
            rules: PlacementRules::default(),
            world_seed: 7,
            layout: layout(),
            spawn_affiliation: crate::world::Affiliation::Player,
            placement_yaw_deg: 0.0,
            placement_uniform_scale: 1.0,
            terrain_vertical_scale: 1.0,
            unit_archetype: None,
            building_archetype: None,
        };
        let mut scratch = BatchSpawnScratch::default();
        let footprint_catalog = FootprintCatalog::default();
        let (categories, items, profiles) = item_catalogs();
        let interior_catalog = InteriorProfileCatalog::default();
        let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
        let unit_archetypes = UnitArchetypeCatalog::default();
        let building_archetypes = BuildingArchetypeCatalog::default();
        let report = execute_batch_spawn(
            &request,
            "wolf",
            &mut world,
            &unit_catalog,
            &unit_archetypes,
            &crate::world::AppearanceProfileCatalog::empty(),
            &doodad_catalog,
            &building_catalog,
            &building_archetypes,
            &footprint_catalog,
            &interior_catalog,
            None,
            &ctx,
            &items,
            &mut scratch,
        );
        assert_eq!(report.spawned, 4);
        let store = world
            .units_in_chunk(ChunkId::new(ChunkCoord::new(0, 0)))
            .unwrap();
        assert_eq!(store.len(), 4);
    }

    #[test]
    fn plan_batch_respects_rejection_count() {
        let world = flat_world();
        let unit_catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let building_catalog = BuildingCatalog::default();
        let request = BatchSpawnRequest {
            definition: DefinitionId::Unit(UnitDefinitionId::new("wolf")),
            brush: BrushSettings {
                mode: BrushMode::Line,
                count: 5,
                spacing: 0.5,
                ..Default::default()
            },
            anchor: anchor(),
            line_direction: Vec2::X,
            terrain_conforming: true,
            rules: PlacementRules {
                min_distance_between_entities: 2.0,
                ..PlacementRules::default()
            },
            world_seed: 1,
            layout: layout(),
            spawn_affiliation: crate::world::Affiliation::Player,
            placement_yaw_deg: 0.0,
            placement_uniform_scale: 1.0,
            terrain_vertical_scale: 1.0,
            unit_archetype: None,
            building_archetype: None,
        };
        let mut scratch = BatchSpawnScratch::default();
        let footprint_catalog = FootprintCatalog::default();
        let (planned, report) = plan_batch_spawn(
            &request,
            "wolf",
            &world,
            &unit_catalog,
            &doodad_catalog,
            &building_catalog,
            &footprint_catalog,
            &mut scratch,
        );
        assert!(report.rejected > 0);
        assert!(planned.len() < 5);
    }

    #[test]
    fn preview_plan_does_not_create_world_entries() {
        let world = flat_world();
        let unit_catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let building_catalog = BuildingCatalog::default();
        let request = BatchSpawnRequest {
            definition: DefinitionId::Doodad(DoodadDefinitionId::new("tree_oak")),
            brush: BrushSettings {
                mode: BrushMode::SingleClick,
                count: 1,
                ..Default::default()
            },
            anchor: anchor(),
            line_direction: Vec2::X,
            terrain_conforming: true,
            rules: PlacementRules::default(),
            world_seed: 0,
            layout: layout(),
            spawn_affiliation: crate::world::Affiliation::Player,
            placement_yaw_deg: 0.0,
            placement_uniform_scale: 1.0,
            terrain_vertical_scale: 1.0,
            unit_archetype: None,
            building_archetype: None,
        };
        let mut scratch = BatchSpawnScratch::default();
        let footprint_catalog = FootprintCatalog::default();
        plan_batch_spawn(
            &request,
            "tree_oak",
            &world,
            &unit_catalog,
            &doodad_catalog,
            &building_catalog,
            &footprint_catalog,
            &mut scratch,
        );
        assert_eq!(
            world.doodads_in_chunk(ChunkId::new(ChunkCoord::new(0, 0))),
            None
        );
    }

    #[test]
    fn storage_chest_authoritative_placement_valid_on_test_world() {
        let world = building_placement_world();
        let building_catalog = BuildingCatalog::default();
        let footprint_catalog = FootprintCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let unit_catalog = UnitCatalog::default();
        let placement_ctx = BuildingPlacementContext {
            world: &world,
            building_catalog: &building_catalog,
            footprint_catalog: &footprint_catalog,
            doodad_catalog: &doodad_catalog,
            unit_catalog: &unit_catalog,
            config: BuildingPlacementConfig::default(),
            player_authorized: true,
            terrain_vertical_scale: 1.0,
        };
        let validation = resolve_authoritative_building_placement(
            &placement_ctx,
            &crate::world::BuildingDefinitionId::new("barn"),
            building_placement_anchor(),
            Quat::IDENTITY,
            BuildingOwnership::with_affiliation(crate::world::Affiliation::Player),
        );
        assert!(
            validation.valid,
            "expected valid placement, got {:?}",
            validation.primary_reason
        );
    }

    #[test]
    fn batch_spawn_storage_building_each_instance_has_inventory() {
        let mut world = building_placement_world();
        let unit_catalog = UnitCatalog::default();
        let doodad_catalog = DoodadCatalog::default();
        let building_catalog = BuildingCatalog::default();
        let request = BatchSpawnRequest {
            definition: DefinitionId::Building(crate::world::BuildingDefinitionId::new("barn")),
            brush: BrushSettings {
                mode: BrushMode::Line,
                count: 3,
                spacing: 10.0,
                ..Default::default()
            },
            anchor: building_placement_anchor(),
            line_direction: Vec2::X,
            terrain_conforming: true,
            rules: PlacementRules::default(),
            world_seed: 11,
            layout: building_placement_layout(),
            spawn_affiliation: crate::world::Affiliation::Player,
            placement_yaw_deg: 0.0,
            placement_uniform_scale: 1.0,
            terrain_vertical_scale: 1.0,
            unit_archetype: None,
            building_archetype: None,
        };
        let mut scratch = BatchSpawnScratch::default();
        let footprint_catalog = FootprintCatalog::default();
        let (categories, items, profiles) = item_catalogs();
        let interior_catalog = InteriorProfileCatalog::default();
        let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
        let unit_archetypes = UnitArchetypeCatalog::default();
        let building_archetypes = BuildingArchetypeCatalog::default();
        let report = execute_batch_spawn(
            &request,
            "barn",
            &mut world,
            &unit_catalog,
            &unit_archetypes,
            &crate::world::AppearanceProfileCatalog::empty(),
            &doodad_catalog,
            &building_catalog,
            &building_archetypes,
            &footprint_catalog,
            &interior_catalog,
            None,
            &ctx,
            &items,
            &mut scratch,
        );
        assert_eq!(report.spawned, 3);
        let mut inventory_ids = Vec::new();
        for building_id in world.sorted_building_ids() {
            let record = world.get_building(building_id).unwrap();
            let inventory_id = record.inventory_id.expect("building inventory");
            assert!(world.inventory_store().get(inventory_id).is_some());
            inventory_ids.push(inventory_id);
        }
        assert_eq!(inventory_ids.len(), 3);
        assert_eq!(
            inventory_ids
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            3,
            "each building must own a distinct inventory"
        );
    }
}
