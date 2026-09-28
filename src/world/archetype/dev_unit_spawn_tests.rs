//! Dev catalog controller vs faction spawn resolution.

use super::*;
use crate::world::{UnitOwnership, is_player_controllable};
use crate::world::relationship::FactionId;
use crate::world::{
    Affiliation, AppearanceProfileCatalog, ChunkCoord, ChunkData, ChunkId, ChunkLayout,
    Heightfield, LocalPosition, UnitCatalog, UnitDefinitionId, UnitSource, WorldData, WorldPosition,
    create_unit_with_inventory_and_faction,
};
use bevy::prelude::Vec3;

fn flat_world() -> WorldData {
    let mut world = WorldData::new(ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    });
    let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
    world.insert(
        ChunkId::new(ChunkCoord::new(0, 0)),
        ChunkData::new(heightfield, Vec::new()),
    );
    world
}

fn pos() -> WorldPosition {
    WorldPosition::new(
        ChunkCoord::new(0, 0),
        LocalPosition::new(Vec3::new(1.0, 0.0, 1.0)),
    )
}

fn spawn_from_spec(
    world: &mut WorldData,
    catalog: &UnitCatalog,
    spec: &ResolvedUnitSpawnSpec,
    ctx: &crate::world::InventoryCatalogCtx<'_>,
) -> crate::world::UnitRecord {
    create_unit_with_inventory_and_faction(
        catalog,
        &AppearanceProfileCatalog::empty(),
        world,
        &spec.definition_id,
        pos(),
        UnitSource::Dev,
        spec.ownership,
        spec.faction_id.clone(),
        ctx,
    )
    .unwrap()
}

#[test]
fn player_controller_with_explicit_faction_is_commandable() {
    let catalog = UnitCatalog::default();
    let archetypes = UnitArchetypeCatalog::default();
    let bandit = UnitDefinitionId::new("bandit");
    let faction_a = FactionId::new("player");
    let spec = resolve_unit_spawn_spec(
        &bandit,
        None,
        UnitOwnership::player_default(),
        Some(&faction_a),
        &catalog,
        &archetypes,
    )
    .unwrap();
    assert_eq!(spec.faction_id, faction_a);
    let mut world = flat_world();
    let categories = crate::world::ItemCategoryCatalog::from_definitions(
        crate::world::starter_item_category_definitions(),
    )
    .unwrap();
    let mut items = crate::world::test_equipment_fixture_definitions();
    items.extend(crate::world::starter_item_definitions());
    let item_catalog =
        crate::world::ItemCatalog::from_definitions(items, &categories).unwrap();
    let profiles = crate::world::InventoryProfileCatalog::from_definitions(
        crate::world::equipment::equipment_slot_profile_definitions(),
    )
    .unwrap();
    let ctx = crate::world::InventoryCatalogCtx::new(&item_catalog, &categories, &profiles);
    let record = spawn_from_spec(&mut world, &catalog, &spec, &ctx);
    assert_eq!(record.faction_id, faction_a);
    assert!(is_player_controllable(&record));
}

#[test]
fn ai_controller_with_same_faction_is_not_commandable() {
    let catalog = UnitCatalog::default();
    let archetypes = UnitArchetypeCatalog::default();
    let bandit = UnitDefinitionId::new("bandit");
    let faction_a = FactionId::new("player");
    let spec = resolve_unit_spawn_spec(
        &bandit,
        None,
        UnitOwnership::wildlife(),
        Some(&faction_a),
        &catalog,
        &archetypes,
    )
    .unwrap();
    assert_eq!(spec.faction_id, faction_a);
    assert_eq!(spec.ownership.affiliation, Affiliation::Wildlife);
}

#[test]
fn ai_controller_can_use_different_faction() {
    let catalog = UnitCatalog::default();
    let archetypes = UnitArchetypeCatalog::default();
    let bandit = UnitDefinitionId::new("bandit");
    let faction_b = FactionId::new("wild");
    let spec = resolve_unit_spawn_spec(
        &bandit,
        None,
        UnitOwnership::wildlife(),
        Some(&faction_b),
        &catalog,
        &archetypes,
    )
    .unwrap();
    assert_eq!(spec.faction_id, faction_b);
}

#[test]
fn controller_and_faction_are_independent_in_state() {
    use crate::dev::dev_mode::{DevModeState, DevSpawnController};

    let mut state = DevModeState::default();
    state.spawn_faction_id = FactionId::new("wild");
    state.cycle_spawn_controller();
    assert_eq!(state.spawn_controller, DevSpawnController::Ai);
    assert_eq!(state.spawn_faction_id.as_str(), "wild");
    state.cycle_spawn_faction(&crate::world::FactionCatalog::default());
    assert_eq!(state.spawn_controller, DevSpawnController::Ai);
}

#[test]
fn archetype_default_faction_used_when_catalog_not_overriding() {
    let catalog = UnitCatalog::default();
    let archetypes = UnitArchetypeCatalog::from_definitions(vec![UnitArchetypeDefinition {
        id: UnitArchetypeId::new("bandit_preset"),
        display_name: "Bandit".to_string(),
        applicable_species: vec![crate::world::relationship::SpeciesId::new("human")],
        gold_min: 0,
        gold_max: 0,
        default_faction_id: Some(FactionId::new("bandits")),
        equipment: Vec::new(),
        inventory_stacks: Vec::new(),
        dialogue: None,
        enabled: true,
    }])
    .unwrap();
    let bandit = UnitDefinitionId::new("bandit");
    let spec = resolve_unit_spawn_spec(
        &bandit,
        Some(&UnitArchetypeId::new("bandit_preset")),
        UnitOwnership::wildlife(),
        None,
        &catalog,
        &archetypes,
    )
    .unwrap();
    assert_eq!(spec.faction_id, FactionId::new("bandits"));
}

#[test]
fn definition_faction_fallback_without_archetype_or_catalog() {
    let catalog = UnitCatalog::default();
    let archetypes = UnitArchetypeCatalog::default();
    let bandit = UnitDefinitionId::new("bandit");
    let definition_faction = catalog.get(&bandit).unwrap().faction_id.clone();
    let spec = resolve_unit_spawn_spec(
        &bandit,
        None,
        UnitOwnership::neutral(),
        None,
        &catalog,
        &archetypes,
    )
    .unwrap();
    assert_eq!(spec.faction_id, definition_faction);
}

#[test]
fn explicit_catalog_faction_overrides_archetype_default() {
    let catalog = UnitCatalog::default();
    let archetypes = UnitArchetypeCatalog::from_definitions(vec![UnitArchetypeDefinition {
        id: UnitArchetypeId::new("bandit_preset"),
        display_name: "Bandit".to_string(),
        applicable_species: vec![crate::world::relationship::SpeciesId::new("human")],
        gold_min: 0,
        gold_max: 0,
        default_faction_id: Some(FactionId::new("bandits")),
        equipment: Vec::new(),
        inventory_stacks: Vec::new(),
        dialogue: None,
        enabled: true,
    }])
    .unwrap();
    let bandit = UnitDefinitionId::new("bandit");
    let explicit = FactionId::new("wild");
    let spec = resolve_unit_spawn_spec(
        &bandit,
        Some(&UnitArchetypeId::new("bandit_preset")),
        UnitOwnership::player_default(),
        Some(&explicit),
        &catalog,
        &archetypes,
    )
    .unwrap();
    assert_eq!(spec.faction_id, explicit);
}

#[test]
fn archetype_capture_preserves_faction_not_controller() {
    use super::capture::{CapturedUnitArchetypeTemplate, build_unit_archetype_definition};

    let template = CapturedUnitArchetypeTemplate {
        species_id: crate::world::relationship::SpeciesId::new("human"),
        default_faction_id: Some(FactionId::new("bandits")),
        equipment: Vec::new(),
        inventory_stacks: Vec::new(),
        gold_on_unit: 0,
    };
    let def = build_unit_archetype_definition(
        UnitArchetypeId::new("captured"),
        "Captured".to_string(),
        vec![crate::world::relationship::SpeciesId::new("human")],
        0,
        0,
        &template,
        None,
    );
    assert_eq!(def.default_faction_id, Some(FactionId::new("bandits")));
}
