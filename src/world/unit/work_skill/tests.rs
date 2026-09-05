//! Work skill authority tests.

use bevy::prelude::{Quat, Vec3};

use crate::world::building::operation::is_prispod_farm_definition;
use crate::world::task::{TaskType, step_worker_assignment};
use crate::world::{
    Affiliation, BuildingCatalog, BuildingCategoryCatalog, BuildingDefinitionId,
    BuildingInteractionProfileCatalog, BuildingLifecycleState, BuildingOwnership, BuildingRecord,
    BuildingSource, ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadCatalog,
    InventoryCatalogCtx, InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog, LocalPosition,
    NavigationConfig, OperationCatalog, SettlementKind, SettlementOwnership, UnitCatalog,
    UnitDefinitionId, UnitOwnership, UnitSource, UnitWorkCapabilities, WeaponCatalog,
    WorkPermissionDomain, WorkSkillCatalog, WorkSkillDefinition, WorkSkillId, WorldData,
    WorldPosition, create_settlement, create_unit_with_ownership, set_unit_work_permission,
    set_work_skill_value, starter_building_definitions, starter_inventory_profile_definitions,
    starter_item_category_definitions, starter_item_definitions, starter_unit_definitions,
    starter_work_skill_definitions, unit_work_allowed, work_permission_domain_for_task,
    work_skill_for_task, work_skill_value,
};

fn layout() -> ChunkLayout {
    ChunkLayout {
        chunk_size_meters: 256.0,
        units_per_meter: 1.0,
    }
}

fn flat_world() -> WorldData {
    let mut world = WorldData::new(layout());
    let heightfield = crate::world::Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
    world.insert(
        ChunkId::new(ChunkCoord::new(0, 0)),
        ChunkData::new(heightfield, Vec::new()),
    );
    world
}

fn pos(x: f32, z: f32) -> WorldPosition {
    WorldPosition::new(
        ChunkCoord::new(0, 0),
        LocalPosition::new(Vec3::new(x, 0.0, z)),
    )
}

fn unit_catalog() -> UnitCatalog {
    UnitCatalog::from_definitions(starter_unit_definitions()).unwrap()
}

fn work_skill_catalog() -> WorkSkillCatalog {
    WorkSkillCatalog::from_definitions(starter_work_skill_definitions()).unwrap()
}

fn inventory_ctx() -> InventoryCatalogCtx<'static> {
    let categories = Box::leak(Box::new(
        ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap(),
    ));
    let items = Box::leak(Box::new(
        ItemCatalog::from_definitions(starter_item_definitions(), categories).unwrap(),
    ));
    let profiles = Box::leak(Box::new(
        InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions()).unwrap(),
    ));
    InventoryCatalogCtx::new(items, categories, profiles)
}

fn building_catalog() -> BuildingCatalog {
    BuildingCatalog::from_definitions(
        starter_building_definitions(),
        &BuildingCategoryCatalog::default(),
    )
    .unwrap()
}

fn spawn_bandit(
    world: &mut WorldData,
    catalog: &UnitCatalog,
    x: f32,
    z: f32,
) -> crate::world::UnitId {
    create_unit_with_ownership(
        catalog,
        world,
        &UnitDefinitionId::new("bandit"),
        pos(x, z),
        UnitSource::Authored,
        UnitOwnership::with_affiliation(Affiliation::Player),
    )
    .unwrap()
    .id
}

#[test]
fn starter_catalog_contains_six_initial_skills_with_display_names() {
    let catalog = work_skill_catalog();
    assert_eq!(catalog.len(), 6);
    let ordered = catalog.enabled_definitions_ordered();
    assert_eq!(
        ordered
            .iter()
            .map(|definition| definition.display_name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "Farming",
            "General Labor",
            "Construction",
            "Cooking",
            "Science",
            "Smithing",
        ]
    );
}

#[test]
fn work_skills_are_catalog_driven_not_bespoke_unit_fields() {
    let mut definitions = starter_work_skill_definitions();
    definitions.push(WorkSkillDefinition::new("prospecting", "Prospecting", 70));
    let catalog = WorkSkillCatalog::from_definitions(definitions).unwrap();
    assert!(catalog.get_str("prospecting").is_some());
}

#[test]
fn created_unit_resolves_every_authored_skill() {
    let catalog = work_skill_catalog();
    let mut world = flat_world();
    let unit_id = spawn_bandit(&mut world, &unit_catalog(), 1.0, 1.0);
    for definition in catalog.enabled_definitions_ordered() {
        assert_eq!(
            work_skill_value(&world, &catalog, unit_id, &definition.id).unwrap(),
            0
        );
    }
}

#[test]
fn two_units_can_diverge_on_same_skill() {
    let catalog = work_skill_catalog();
    let mut world = flat_world();
    let unit_catalog = unit_catalog();
    let jim = spawn_bandit(&mut world, &unit_catalog, 1.0, 1.0);
    let larry = spawn_bandit(&mut world, &unit_catalog, 2.0, 2.0);
    set_work_skill_value(&mut world, &catalog, jim, &WorkSkillId::new("farming"), 12).unwrap();
    set_work_skill_value(
        &mut world,
        &catalog,
        larry,
        &WorkSkillId::new("farming"),
        66,
    )
    .unwrap();
    assert_eq!(
        work_skill_value(&world, &catalog, jim, &WorkSkillId::new("farming")).unwrap(),
        12
    );
    assert_eq!(
        work_skill_value(&world, &catalog, larry, &WorkSkillId::new("farming")).unwrap(),
        66
    );
}

#[test]
fn setting_farming_does_not_mutate_general_labor() {
    let catalog = work_skill_catalog();
    let mut world = flat_world();
    let unit_id = spawn_bandit(&mut world, &unit_catalog(), 1.0, 1.0);
    set_work_skill_value(
        &mut world,
        &catalog,
        unit_id,
        &WorkSkillId::new("farming"),
        42,
    )
    .unwrap();
    assert_eq!(
        work_skill_value(
            &world,
            &catalog,
            unit_id,
            &WorkSkillId::new("general_labor")
        )
        .unwrap(),
        0
    );
}

#[test]
fn no_implicit_zero_to_hundred_clamp() {
    let catalog = work_skill_catalog();
    let mut world = flat_world();
    let unit_id = spawn_bandit(&mut world, &unit_catalog(), 1.0, 1.0);
    set_work_skill_value(
        &mut world,
        &catalog,
        unit_id,
        &WorkSkillId::new("science"),
        250,
    )
    .unwrap();
    assert_eq!(
        work_skill_value(&world, &catalog, unit_id, &WorkSkillId::new("science")).unwrap(),
        250
    );
}

#[test]
fn work_capabilities_remain_separate_from_work_skills() {
    let unit_catalog = unit_catalog();
    let bandit = unit_catalog.get(&UnitDefinitionId::new("bandit")).unwrap();
    let wolf = unit_catalog.get(&UnitDefinitionId::new("wolf")).unwrap();
    assert!(bandit.work_capabilities.can_haul);
    assert!(!wolf.work_capabilities.can_haul);
    let catalog = work_skill_catalog();
    let mut world = flat_world();
    let unit_id = spawn_bandit(&mut world, &unit_catalog, 1.0, 1.0);
    set_work_skill_value(
        &mut world,
        &catalog,
        unit_id,
        &WorkSkillId::new("general_labor"),
        80,
    )
    .unwrap();
    assert!(bandit.work_capabilities.can_haul);
    assert!(!wolf.work_capabilities.can_haul);
    assert_eq!(
        work_skill_value(
            &world,
            &catalog,
            unit_id,
            &WorkSkillId::new("general_labor"),
        )
        .unwrap(),
        80
    );
}

#[test]
fn workforce_permissions_remain_separate_from_work_skills() {
    let catalog = work_skill_catalog();
    let mut world = flat_world();
    let settlement_id = create_settlement(
        &mut world,
        pos(20.0, 20.0),
        "Town",
        SettlementOwnership::player_default(),
        SettlementKind::Town,
        None,
        None,
        0,
    )
    .unwrap()
    .settlement_id;
    let unit_id = spawn_bandit(&mut world, &unit_catalog(), 1.0, 1.0);
    crate::world::assign_unit_settlement(&mut world, unit_id, Some(settlement_id)).unwrap();
    set_work_skill_value(
        &mut world,
        &catalog,
        unit_id,
        &WorkSkillId::new("general_labor"),
        83,
    )
    .unwrap();
    set_unit_work_permission(
        &mut world,
        settlement_id,
        unit_id,
        WorkPermissionDomain::GeneralLabor,
        false,
    )
    .unwrap();
    assert!(!unit_work_allowed(
        &world,
        settlement_id,
        unit_id,
        WorkPermissionDomain::GeneralLabor
    ));
    assert_eq!(
        work_skill_value(
            &world,
            &catalog,
            unit_id,
            &WorkSkillId::new("general_labor"),
        )
        .unwrap(),
        83
    );
}

#[test]
fn task_mapping_for_current_semantics() {
    let mut world = flat_world();
    let building_catalog = building_catalog();
    let operation_catalog = OperationCatalog::default();
    let farm_def = building_catalog
        .definitions()
        .iter()
        .find(|definition| is_prispod_farm_definition(definition))
        .cloned()
        .expect("farm");
    let farm_id = world.allocate_building_id();
    let mut record = BuildingRecord::new(
        farm_id,
        farm_def.id.clone(),
        crate::world::BuildingPlacement::new(pos(10.0, 10.0), Quat::IDENTITY),
        BuildingOwnership::with_affiliation(Affiliation::Player),
        farm_def.max_hp,
        BuildingSource::Authored,
    );
    record.lifecycle_state = BuildingLifecycleState::Complete;
    world
        .insert_building(ChunkId::new(ChunkCoord::new(0, 0)), record)
        .unwrap();
    world
        .building_production_store_mut()
        .ensure_policy_for_building(farm_id, &farm_def, &operation_catalog);

    assert_eq!(
        work_skill_for_task(
            &world,
            &building_catalog,
            &operation_catalog,
            farm_id,
            TaskType::OperateWorkstation,
        ),
        Some(WorkSkillId::new("farming"))
    );
    assert_eq!(
        work_skill_for_task(
            &world,
            &building_catalog,
            &operation_catalog,
            farm_id,
            TaskType::Haul,
        ),
        Some(WorkSkillId::new("general_labor"))
    );
    assert_eq!(
        work_skill_for_task(
            &world,
            &building_catalog,
            &operation_catalog,
            farm_id,
            TaskType::ConstructBuilding,
        ),
        Some(WorkSkillId::new("construction"))
    );

    let quarry_def = building_catalog
        .get(&BuildingDefinitionId::new("stone_quarry"))
        .cloned()
        .expect("stone_quarry");
    let quarry_id = world.allocate_building_id();
    let mut quarry_record = BuildingRecord::new(
        quarry_id,
        quarry_def.id.clone(),
        crate::world::BuildingPlacement::new(pos(20.0, 20.0), Quat::IDENTITY),
        BuildingOwnership::with_affiliation(Affiliation::Player),
        quarry_def.max_hp,
        BuildingSource::Authored,
    );
    quarry_record.lifecycle_state = BuildingLifecycleState::Complete;
    world
        .insert_building(ChunkId::new(ChunkCoord::new(0, 0)), quarry_record)
        .unwrap();
    world
        .building_production_store_mut()
        .ensure_policy_for_building(quarry_id, &quarry_def, &operation_catalog);
    assert_eq!(
        work_skill_for_task(
            &world,
            &building_catalog,
            &operation_catalog,
            quarry_id,
            TaskType::OperateWorkstation,
        ),
        Some(WorkSkillId::new("general_labor"))
    );
    assert_eq!(
        work_permission_domain_for_task(
            &world,
            &building_catalog,
            &operation_catalog,
            quarry_id,
            TaskType::OperateWorkstation,
        ),
        Some(WorkPermissionDomain::GeneralLabor)
    );
}

#[test]
fn permission_domains_map_one_to_one_with_work_skills() {
    use crate::world::work_skill_for_permission_domain;

    assert_eq!(
        work_skill_for_permission_domain(WorkPermissionDomain::Farming).as_str(),
        "farming"
    );
    assert_eq!(
        work_skill_for_permission_domain(WorkPermissionDomain::GeneralLabor).as_str(),
        "general_labor"
    );
    assert_eq!(
        work_skill_for_permission_domain(WorkPermissionDomain::Construction).as_str(),
        "construction"
    );
    assert_eq!(
        work_skill_for_permission_domain(WorkPermissionDomain::Cooking).as_str(),
        "cooking"
    );
    assert_eq!(
        work_skill_for_permission_domain(WorkPermissionDomain::Science).as_str(),
        "science"
    );
    assert_eq!(
        work_skill_for_permission_domain(WorkPermissionDomain::Smithing).as_str(),
        "smithing"
    );
}

#[test]
fn unimplemented_task_types_do_not_map_to_cooking_science_smithing_skills() {
    let mut world = flat_world();
    let building_catalog = building_catalog();
    let operation_catalog = OperationCatalog::default();
    let building_id = world.allocate_building_id();
    assert!(
        work_skill_for_task(
            &world,
            &building_catalog,
            &operation_catalog,
            building_id,
            TaskType::RepairBuilding,
        )
        .is_none()
    );
}

#[test]
fn skill_values_do_not_change_marketplace_assignment() {
    let catalog = work_skill_catalog();
    let mut world = flat_world();
    let building_catalog = building_catalog();
    let operation_catalog = OperationCatalog::default();
    let interaction = BuildingInteractionProfileCatalog::default();
    let unit_catalog = unit_catalog();
    let settlement_id = create_settlement(
        &mut world,
        pos(30.0, 30.0),
        "Town",
        SettlementOwnership::player_default(),
        SettlementKind::Town,
        None,
        None,
        0,
    )
    .unwrap()
    .settlement_id;
    let worker = spawn_bandit(&mut world, &unit_catalog, 30.0, 30.0);
    crate::world::assign_unit_settlement(&mut world, worker, Some(settlement_id)).unwrap();
    set_work_skill_value(
        &mut world,
        &catalog,
        worker,
        &WorkSkillId::new("construction"),
        999,
    )
    .unwrap();

    let farm_def = building_catalog
        .definitions()
        .iter()
        .find(|definition| is_prispod_farm_definition(definition))
        .cloned()
        .expect("farm");
    let farm_id = world.allocate_building_id();
    let mut record = BuildingRecord::new(
        farm_id,
        farm_def.id.clone(),
        crate::world::BuildingPlacement::new(pos(31.0, 31.0), Quat::IDENTITY),
        BuildingOwnership::with_affiliation(Affiliation::Player),
        farm_def.max_hp,
        BuildingSource::Authored,
    );
    record.lifecycle_state = BuildingLifecycleState::Complete;
    record.settlement_id = Some(settlement_id);
    world
        .insert_building(ChunkId::new(ChunkCoord::new(0, 0)), record)
        .unwrap();
    world
        .building_production_store_mut()
        .ensure_policy_for_building(farm_id, &farm_def, &operation_catalog);
    crate::world::sync_operate_workstation_tasks(&mut world, &building_catalog, 1);

    let mut assign_ctx = crate::world::WorkerAssignmentContext {
        world: &mut world,
        unit_catalog: &unit_catalog,
        weapon_catalog: &WeaponCatalog::default(),
        doodad_catalog: &DoodadCatalog::default(),
        building_catalog: &building_catalog,
        operation_catalog: &operation_catalog,
        interaction_catalog: &interaction,
        nav_config: &NavigationConfig::default(),
        inventory_ctx: &inventory_ctx(),
        simulation_tick: 2,
    };
    let report = step_worker_assignment(&mut assign_ctx);
    assert!(
        report.assignments.is_empty(),
        "high construction skill must not affect unrelated assignment in this phase"
    );
}
