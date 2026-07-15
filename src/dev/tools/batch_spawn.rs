//! Batch spawn execution via WorldData APIs (ADR-044).

use bevy::prelude::*;

use crate::world::{
    BuildingCatalog, BuildingOwnership, BuildingSource, DoodadCatalog, DoodadPlacementOverrides,
    DoodadSource, FootprintCatalog, InteriorProfileCatalog, InventoryCatalogCtx, OccupancyCatalogs,
    UnitCatalog, UnitOwnership, UnitSource, WorldData, WorldPosition,
    create_dev_complete_building, create_dev_complete_building_with_inventory, create_doodad,
    create_unit_with_inventory, try_activate_interior_if_complete,
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
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    interior_catalog: &InteriorProfileCatalog,
    inventory_ctx: &InventoryCatalogCtx<'_>,
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
            doodad_catalog,
            building_catalog,
            footprint_catalog,
            interior_catalog,
            inventory_ctx,
            &request.definition,
            position,
            request.spawn_affiliation,
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
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    interior_catalog: &InteriorProfileCatalog,
    inventory_ctx: &InventoryCatalogCtx<'_>,
    definition: &DefinitionId,
    position: WorldPosition,
    spawn_affiliation: crate::world::Affiliation,
) -> bool {
    match definition {
        DefinitionId::Unit(definition_id) => create_unit_with_inventory(
            unit_catalog,
            world,
            definition_id,
            position,
            UnitSource::Dev,
            UnitOwnership::with_affiliation(spawn_affiliation),
            inventory_ctx,
        )
        .is_ok(),
        DefinitionId::Doodad(definition_id) => create_doodad(
            doodad_catalog,
            world,
            definition_id,
            position,
            DoodadSource::Dev,
            DoodadPlacementOverrides::default(),
            None,
        )
        .is_ok(),
        DefinitionId::Building(definition_id) => {
            let occupancy = OccupancyCatalogs {
                doodad: doodad_catalog,
                building: building_catalog,
                footprint: footprint_catalog,
            };
            let ownership = BuildingOwnership::with_affiliation(spawn_affiliation);
            let spawned = if building_catalog
                .get(definition_id)
                .is_some_and(|def| def.inventory_profile_id.is_some())
            {
                create_dev_complete_building_with_inventory(
                    building_catalog,
                    world,
                    definition_id,
                    position,
                    Quat::IDENTITY,
                    ownership,
                    Some(occupancy),
                    inventory_ctx,
                )
            } else {
                create_dev_complete_building(
                    building_catalog,
                    world,
                    definition_id,
                    position,
                    Quat::IDENTITY,
                    ownership,
                    Some(occupancy),
                )
            };
            match spawned {
                Ok(record) => {
                    let _ = try_activate_interior_if_complete(
                        world,
                        building_catalog,
                        interior_catalog,
                        doodad_catalog,
                        occupancy,
                        record.id,
                    );
                    true
                }
                Err(_) => false,
            }
        }
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
        };
        let mut scratch = BatchSpawnScratch::default();
        let footprint_catalog = FootprintCatalog::default();
        let (categories, items, profiles) = item_catalogs();
        let interior_catalog = InteriorProfileCatalog::default();
        let ctx = InventoryCatalogCtx::new(&items, &categories, &profiles);
        let report = execute_batch_spawn(
            &request,
            "wolf",
            &mut world,
            &unit_catalog,
            &doodad_catalog,
            &building_catalog,
            &footprint_catalog,
            &interior_catalog,
            &ctx,
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
}
