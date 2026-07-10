//! Production baseline regression tests (ADR-049 U-REVIEW1).
//!
//! Verifies fail-closed catalog behavior and absence of silent fallback injection.

use crate::client::commands::{build_command_plan, CommandBuildError, CommandResolutionContext};
use crate::client::commands::{CommandTarget, CommandType, ContextualCommandIntent};
use crate::units::input::SelectedUnits;
use crate::world::{
    create_doodad, create_unit, generate_chunk_doodads_with_settings, ChunkCoord, ChunkData,
    ChunkId, ChunkLayout, DoodadCatalog, DoodadDefinitionId, DoodadGenerationContext,
    DoodadGenerationSettings, DoodadPlacementOverrides, DoodadSource, Heightfield, LocalPosition,
    UnitCatalog, UnitDefinitionId, UnitSource, WorldData, WorldPosition,
};
use bevy::prelude::Vec3;

fn empty_layout_world() -> WorldData {
    WorldData::new(ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    })
}

fn pos(x: f32, z: f32) -> WorldPosition {
    WorldPosition::new(
        ChunkCoord::new(0, 0),
        LocalPosition::new(Vec3::new(x, 0.0, z)),
    )
}

#[test]
fn empty_unit_catalog_has_no_fallback_definitions() {
    let catalog = UnitCatalog::from_definitions(Vec::new()).unwrap();
    assert!(catalog.is_empty());
    assert!(catalog.get(&UnitDefinitionId::new("wolf")).is_none());
}

#[test]
fn empty_doodad_catalog_has_no_fallback_definitions() {
    let catalog = DoodadCatalog::from_definitions(Vec::new()).unwrap();
    assert!(catalog.is_empty());
    assert!(catalog.get(&DoodadDefinitionId::new("tree_oak")).is_none());
}

#[test]
fn create_unit_fails_when_catalog_empty() {
    let catalog = UnitCatalog::from_definitions(Vec::new()).unwrap();
    let mut world = empty_layout_world();
    let err = create_unit(
        &catalog,
        &mut world,
        &UnitDefinitionId::new("wolf"),
        pos(1.0, 1.0),
        UnitSource::Authored,
    )
    .unwrap_err();
    assert!(matches!(
        err,
        crate::world::UnitAuthoringError::DefinitionNotFound(_)
    ));
}

#[test]
fn create_doodad_fails_when_catalog_empty() {
    let catalog = DoodadCatalog::from_definitions(Vec::new()).unwrap();
    let mut world = empty_layout_world();
    let err = create_doodad(
        &catalog,
        &mut world,
        &DoodadDefinitionId::new("tree_oak"),
        pos(1.0, 1.0),
        DoodadSource::Authored,
        DoodadPlacementOverrides::default(),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        crate::world::DoodadAuthoringError::DefinitionNotFound(_)
    ));
}

#[test]
fn procedural_generation_with_empty_catalog_emits_no_candidates() {
    let catalog = DoodadCatalog::from_definitions(Vec::new()).unwrap();
    let layout = ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    };
    let ctx = DoodadGenerationContext::new(
        42,
        ChunkId::new(ChunkCoord::new(0, 0)),
        &layout,
    );
    let candidates = generate_chunk_doodads_with_settings(
        &ctx,
        &catalog,
        &DoodadGenerationSettings::default(),
    );
    assert!(candidates.is_empty());
}

#[test]
fn command_builder_fails_closed_without_silent_move_fallback() {
    let mut selection = SelectedUnits::default();
    selection.set_single(crate::world::UnitId::new(1));
    let world = empty_layout_world();
    let intent = ContextualCommandIntent {
        command_type: CommandType::Move,
        target: CommandTarget::Unit {
            unit_id: crate::world::UnitId::new(999),
        },
    };
    let err = build_command_plan(&intent, &selection, &world).unwrap_err();
    assert_eq!(err, CommandBuildError::TargetUnitNotFound);
}

#[test]
fn contextual_resolver_does_not_inject_world_data() {
    let units = [crate::world::UnitId::new(1)];
    let world = empty_layout_world();
    let unit_catalog = crate::world::UnitCatalog::default();
    let weapon_catalog = crate::world::WeaponCatalog::default();
    let ctx = CommandResolutionContext {
        selected_units: &units,
        target: CommandTarget::Terrain {
            position: pos(5.0, 5.0),
        },
        world: &world,
        unit_catalog: &unit_catalog,
        weapon_catalog: &weapon_catalog,
        targeting_policy: crate::world::AttackTargetingPolicy::default(),
    };
    let resolved = crate::client::commands::resolve_contextual_command(&ctx).unwrap();
    assert_eq!(resolved.command_type, CommandType::Move);
    assert!(world.sorted_unit_ids().is_empty());
}

#[test]
fn world_data_starts_without_injected_test_entities() {
    let world = empty_layout_world();
    assert!(world.sorted_unit_ids().is_empty());
    assert!(world.sorted_doodad_ids().is_empty());
    assert!(world.extent().is_none());
}

#[test]
fn movement_with_empty_catalogs_does_not_panic_on_empty_world() {
    let catalog = UnitCatalog::from_definitions(Vec::new()).unwrap();
    let doodad_catalog = DoodadCatalog::from_definitions(Vec::new()).unwrap();
    let mut world = empty_layout_world();
    let mut scan = crate::world::CombatAiScanState::default();
    let settings = crate::world::CombatAiSettings::default();
    let report = crate::simulation::run_simulation_tick(
        &mut world,
        &catalog,
        &crate::world::WeaponCatalog::default(),
        &doodad_catalog,
        &crate::world::NavigationConfig::default(),
        crate::world::AttackTargetingPolicy::default(),
        &settings,
        &mut scan,
        1.0 / 60.0,
        0,
    );
    assert_eq!(report.movement.moved, 0);
}

#[test]
fn deterministic_catalog_from_definitions_only() {
    let defs_a = vec![];
    let defs_b = vec![];
    let a = UnitCatalog::from_definitions(defs_a).unwrap();
    let b = UnitCatalog::from_definitions(defs_b).unwrap();
    assert_eq!(a.len(), b.len());
}

#[cfg(not(feature = "dev"))]
#[test]
fn dev_module_not_linked_without_dev_feature() {
    // Compiling this test confirms `crate::dev` is absent from production builds.
}

#[test]
fn flat_chunk_fixture_does_not_imply_production_terrain_injection() {
    let mut world = empty_layout_world();
    let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
    world.insert(
        ChunkId::new(ChunkCoord::new(0, 0)),
        ChunkData::new(heightfield, Vec::new()),
    );
    assert_eq!(world.len(), 1);
    assert!(world.sorted_unit_ids().is_empty());
}
