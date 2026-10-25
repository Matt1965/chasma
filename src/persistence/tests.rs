//! Persistence transaction integration tests (Chunk 1B).

use bevy::prelude::{Quat, Vec3};

use crate::dev::scenes::{
    SceneApplyError, SceneCaptureContext, SceneDefinition, SceneQuat, SceneUnitRecord,
    SceneUnitSource, SceneUnitState, SceneWorldPosition, apply_scene, capture_scene, test_support,
};
use crate::persistence::transactional_session_load;
use crate::world::{
    Affiliation, AppearanceProfileCatalog, BuildingCatalog, BuildingId, BuildingSource, ChunkCoord,
    ChunkData, ChunkId, ChunkLayout, CombatState, DoodadCatalog, FactionId, FootprintCatalog,
    InteriorProfileCatalog, InventoryCatalogCtx, LocalPosition, RelationshipFacet, SettlementId,
    SpeciesId, TaskPriority, TaskRecord, TaskState, TaskTarget, TaskType, UnitCatalog,
    UnitDefinitionId, UnitId, UnitOwnership, UnitSource, WorldData, WorldPosition,
    assign_building_settlement, count_stack_item, create_building_with_inventory,
    create_settlement_with_treasury, create_unit_with_inventory, physical_gold_item_id,
    place_stack_first_fit, starter_building_catalog_with_smelter, starter_inventory_profile_definitions,
    starter_item_category_definitions, starter_item_definitions, starter_unit_definitions,
};

/// Gameplay invariants for [`rich_session`] — used instead of full `WorldData` equality.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RichSessionInvariants {
    tick: u64,
    unit_id: UnitId,
    hp: u32,
    faction: FactionId,
    species: SpeciesId,
    gold: u32,
    settlement_id: SettlementId,
    core_building: BuildingId,
    assigned_task: bool,
    standing_faction_species: i32,
}

fn invariants_from(world: &WorldData, tick: u64) -> RichSessionInvariants {
    let unit_id = world.sorted_unit_ids()[0];
    let unit = world.get_unit(unit_id).unwrap();
    let personal = unit.inventory_id.expect("inventory");
    let inv = world.inventory_store().get(personal).expect("personal inventory");
    let gold = count_stack_item(inv, &physical_gold_item_id());
    let settlement_id = unit.settlement_id.expect("settlement");
    let core = world
        .sorted_building_ids()
        .into_iter()
        .find(|id| world.get_building(*id).unwrap().definition_id.as_str() == "settlement_core")
        .expect("core");
    let assigned_task = world.task_store().unit_task_id(unit_id).is_some();
    let faction = unit.faction_id.clone();
    let species = unit.species_id.clone();
    let standing_faction_species = world.relationship_standing_store().get(
        &RelationshipFacet::Faction(faction.clone()),
        &RelationshipFacet::Species(species.clone()),
    );
    RichSessionInvariants {
        tick,
        unit_id,
        hp: unit.vitals.current_hp,
        faction,
        species,
        gold,
        settlement_id,
        core_building: core,
        assigned_task,
        standing_faction_species,
    }
}

fn assert_rich_session(world: &WorldData, tick: u64, expected: &RichSessionInvariants) {
    assert_eq!(&invariants_from(world, tick), expected);
}

fn flat_world() -> WorldData {
    let layout = ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    };
    let mut world = WorldData::new(layout);
    let heightfield =
        crate::world::Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
    world.insert(
        ChunkId::new(ChunkCoord::new(0, 0)),
        ChunkData::new(heightfield, Vec::new()),
    );
    world
}

fn inventory_ctx() -> &'static InventoryCatalogCtx<'static> {
    static CTX: std::sync::OnceLock<InventoryCatalogCtx<'static>> = std::sync::OnceLock::new();
    CTX.get_or_init(|| {
        let categories = crate::world::ItemCategoryCatalog::from_definitions(
            starter_item_category_definitions(),
        )
        .unwrap();
        let items =
            crate::world::ItemCatalog::from_definitions(starter_item_definitions(), &categories)
                .unwrap();
        let profiles = crate::world::InventoryProfileCatalog::from_definitions(
            starter_inventory_profile_definitions(),
        )
        .unwrap();
        let items = Box::leak(Box::new(items));
        let categories = Box::leak(Box::new(categories));
        let profiles = Box::leak(Box::new(profiles));
        InventoryCatalogCtx::new(items, categories, profiles)
    })
}

fn pos(x: f32, z: f32) -> WorldPosition {
    WorldPosition::new(
        ChunkCoord::new(0, 0),
        LocalPosition::new(Vec3::new(x, 0.0, z)),
    )
}

/// Meaningful authoritative session: damaged unit, custom identities, inventory, settlement, task.
fn rich_session() -> (WorldData, u64, RichSessionInvariants) {
    let mut world = flat_world();
    let tick = 12_345u64;
    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let appearance = AppearanceProfileCatalog::empty();
    let ctx = inventory_ctx();
    let ownership = UnitOwnership::with_affiliation(Affiliation::Player);

    let unit = create_unit_with_inventory(
        &unit_catalog,
        &appearance,
        &mut world,
        &UnitDefinitionId::new("bandit"),
        pos(8.0, 8.0),
        UnitSource::Authored,
        ownership,
        &ctx,
    )
    .unwrap();

    let faction = FactionId::new("custom_faction");
    let species = SpeciesId::new("custom_species");
    world.mutate_unit(unit.id, |record| {
        record.vitals.current_hp = 3;
        record.vitals.max_hp = 50;
        record.combat_state = CombatState::Peaceful;
        record.faction_id = faction.clone();
        record.species_id = species.clone();
    });

    world.relationship_standing_store_mut().apply_delta(
        RelationshipFacet::Faction(faction.clone()),
        RelationshipFacet::Species(species.clone()),
        42,
    );

    let personal = unit.inventory_id.expect("unit inventory");
    let (inventory_store, instance_store) = world.inventory_runtime_mut();
    place_stack_first_fit(
        inventory_store,
        instance_store,
        &ctx,
        personal,
        physical_gold_item_id(),
        17,
    )
    .unwrap();

    let building_catalog = starter_building_catalog_with_smelter();
    let interaction_catalog = crate::world::BuildingInteractionProfileCatalog::default();
    let building_ownership = crate::world::BuildingOwnership::with_affiliation(Affiliation::Player);
    let core = create_building_with_inventory(
        &building_catalog,
        &mut world,
        &crate::world::BuildingDefinitionId::new("settlement_core"),
        pos(40.0, 40.0),
        bevy::prelude::Quat::IDENTITY,
        BuildingSource::Authored,
        building_ownership,
        None,
        &ctx,
    )
    .unwrap()
    .id;
    world.mutate_building(core, |record| {
        record.lifecycle_state = crate::world::BuildingLifecycleState::Complete;
    });

    let settlement = create_settlement_with_treasury(
        &mut world,
        &building_catalog,
        &interaction_catalog,
        core,
        "Persistence Test",
        crate::world::SettlementOwnership::player_default(),
        pos(40.0, 40.0),
        tick,
    )
    .unwrap();
    assign_building_settlement(&mut world, core, Some(settlement.settlement_id)).unwrap();
    world.mutate_unit(unit.id, |record| {
        record.settlement_id = Some(settlement.settlement_id);
    });

    let mut task = TaskRecord::new(
        crate::world::TaskId::new(1),
        TaskType::Haul,
        TaskTarget::Building(core),
        TaskPriority::Normal,
        tick,
    );
    task.state = TaskState::Assigned;
    task.assigned_unit_id = Some(unit.id);
    world.task_store_mut().insert_task(task).unwrap();

    let invariants = invariants_from(&world, tick);
    (world, tick, invariants)
}

fn empty_scene_from(world: &WorldData) -> SceneDefinition {
    let ctx = SceneCaptureContext {
        name: "empty".into(),
        description: "test".into(),
        tags: Vec::new(),
        created_at: 1,
        world_seed: 1,
        camera_state: None,
        debug_flags: None,
    };
    capture_scene(world, &ctx)
}

#[test]
fn invalid_scene_plan_leaves_session_unchanged() {
    let (mut world, mut tick, expected) = rich_session();

    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let mut scene = empty_scene_from(&world);
    scene.unit_records.push(SceneUnitRecord {
        id: 99,
        definition_id: "missing_definition".into(),
        position: SceneWorldPosition {
            chunk_x: 0,
            chunk_z: 0,
            local_x: 1.0,
            local_y: 0.0,
            local_z: 1.0,
        },
        rotation: SceneQuat::from_quat(Quat::IDENTITY),
        state: SceneUnitState::Idle,
        source: SceneUnitSource::Authored,
        owner_id: None,
        team_id: None,
        affiliation: None,
        current_space_id: 0,
        inventory_id: None,
        equipment: None,
        faction_id: None,
        species_id: None,
        settlement_id: None,
        current_nutrition: None,
        work_skill_overrides: Vec::new(),
        appearance: None,
        dialogue: None,
    });

    let err = apply_scene(
        &mut world,
        &unit_catalog,
        &DoodadCatalog::default(),
        &BuildingCatalog::default(),
        &FootprintCatalog::default(),
        &InteriorProfileCatalog::default(),
        None,
        &AppearanceProfileCatalog::empty(),
        &scene,
        Some(&mut tick),
    )
    .unwrap_err();
    assert!(matches!(err, SceneApplyError::MissingUnitDefinition { .. }));
    assert_rich_session(&world, tick, &expected);
}

#[test]
fn late_apply_failure_restores_full_session() {
    let (mut world, mut tick, expected) = rich_session();

    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let empty = flat_world();
    let scene = empty_scene_from(&empty);

    test_support::set_inject_late_apply_failure(true);
    let err = apply_scene(
        &mut world,
        &unit_catalog,
        &DoodadCatalog::default(),
        &BuildingCatalog::default(),
        &FootprintCatalog::default(),
        &InteriorProfileCatalog::default(),
        None,
        &AppearanceProfileCatalog::empty(),
        &scene,
        Some(&mut tick),
    )
    .unwrap_err();
    test_support::set_inject_late_apply_failure(false);

    assert!(matches!(err, SceneApplyError::InventoryRestore { .. }));
    assert_rich_session(&world, tick, &expected);
}

#[test]
fn successful_scene_apply_mutates_session() {
    let (mut world, mut tick, expected) = rich_session();

    let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
    let mut donor = flat_world();
    let _ = create_unit_with_inventory(
        &unit_catalog,
        &AppearanceProfileCatalog::empty(),
        &mut donor,
        &UnitDefinitionId::new("deer"),
        pos(3.0, 3.0),
        UnitSource::Authored,
        UnitOwnership::with_affiliation(Affiliation::Player),
        &inventory_ctx(),
    )
    .unwrap();
    let scene = empty_scene_from(&donor);

    test_support::set_inject_late_apply_failure(false);
    apply_scene(
        &mut world,
        &unit_catalog,
        &DoodadCatalog::default(),
        &BuildingCatalog::default(),
        &FootprintCatalog::default(),
        &InteriorProfileCatalog::default(),
        None,
        &AppearanceProfileCatalog::empty(),
        &scene,
        Some(&mut tick),
    )
    .unwrap();

    assert_eq!(world.sorted_unit_ids().len(), 1);
    let loaded = world.get_unit(world.sorted_unit_ids()[0]).unwrap();
    assert_eq!(loaded.definition_id.as_str(), "deer");
    assert_ne!(loaded.definition_id.as_str(), "bandit");
    assert!(loaded.settlement_id.is_none());
    assert!(world.task_store().records().next().is_none());
    assert_eq!(tick, expected.tick);
}

#[test]
fn transactional_hook_rolls_back_without_scene_format() {
    let (mut world, mut tick, expected) = rich_session();

    let err = transactional_session_load(&mut world, &mut tick, |world| {
        world.dev_clear_units_and_doodads();
        Err::<(), &str>("simulated failure")
    })
    .unwrap_err();
    assert_eq!(err.error, "simulated failure");
    assert_rich_session(&world, tick, &expected);
}
