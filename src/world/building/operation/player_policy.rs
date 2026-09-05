//! Player-facing production policy mutations with internal manual override (BP3).

use crate::world::building::catalog::BuildingCatalog;
use crate::world::operation::OperationCatalog;
use crate::world::{BuildingId, WorldData};

use super::commands::{
    ProductionCommandError, require_building, set_production_selected_operation,
};
use super::lifecycle::OperationLifecycle;
use super::operation_id::OperationDefinitionId;
use super::policy::ControlSource;
use super::priority::{
    BuildingWorkPriorityLevel, building_work_priority_u8_for_level,
    step_building_work_priority_level,
};

/// Player toggles production enabled/disabled and takes manual control atomically.
pub fn apply_player_production_enabled(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    operation_catalog: &OperationCatalog,
    building_id: BuildingId,
    enabled: bool,
) -> Result<(), ProductionCommandError> {
    require_building(world, building_id)?;
    let definition = world
        .get_building(building_id)
        .and_then(|record| building_catalog.get(&record.definition_id))
        .ok_or(ProductionCommandError::BuildingNotFound(building_id))?;
    let store = world.building_production_store_mut();
    store.ensure_policy_for_building(building_id, definition, operation_catalog);
    let policy = store.get_policy_mut(building_id);
    policy.enabled = enabled;
    policy.control_source = ControlSource::PlayerControlled;
    if !enabled {
        let state = store.get_state_mut(building_id);
        state.lifecycle = OperationLifecycle::Disabled;
        state.blocked_reason = None;
        state.active_worker_count = 0;
    }
    Ok(())
}

/// Player selects an operation and takes manual control atomically.
pub fn apply_player_production_selected_operation(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    operation_catalog: &OperationCatalog,
    building_id: BuildingId,
    operation: OperationDefinitionId,
) -> Result<(), ProductionCommandError> {
    set_production_selected_operation(
        world,
        building_catalog,
        operation_catalog,
        building_id,
        Some(operation),
    )?;
    world
        .building_production_store_mut()
        .get_policy_mut(building_id)
        .control_source = ControlSource::PlayerControlled;
    Ok(())
}

/// Player adjusts building work priority and takes manual control atomically.
pub fn apply_player_building_work_priority(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    operation_catalog: &OperationCatalog,
    building_id: BuildingId,
    increase: bool,
) -> Result<(), ProductionCommandError> {
    require_building(world, building_id)?;
    let definition = world
        .get_building(building_id)
        .and_then(|record| building_catalog.get(&record.definition_id))
        .ok_or(ProductionCommandError::BuildingNotFound(building_id))?;
    let store = world.building_production_store_mut();
    store.ensure_policy_for_building(building_id, definition, operation_catalog);
    let current_priority = store
        .get_policy(building_id)
        .map(|policy| policy.priority)
        .unwrap_or(super::priority::DEFAULT_BUILDING_WORK_PRIORITY_U8);
    let level = step_building_work_priority_level(
        super::priority::building_work_priority_level_from_u8(current_priority),
        increase,
    );
    let policy = store.get_policy_mut(building_id);
    policy.priority = building_work_priority_u8_for_level(level);
    policy.control_source = ControlSource::PlayerControlled;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::inventory::InventoryCatalogCtx;
    use crate::world::settlement::{
        SettlementOwnership, assign_building_settlement, create_settlement_with_treasury,
    };
    use crate::world::{
        Affiliation, BuildingCategoryCatalog, BuildingDefinitionId, BuildingLifecycleState,
        BuildingOwnership, BuildingSource, ChunkCoord, ChunkExtent, LocalPosition, WorldData,
        WorldPosition, building_inventory_bindings, create_building_with_inventory,
        starter_building_definitions, starter_inventory_profile_definitions,
        starter_item_category_definitions, starter_item_definitions, starter_operation_definitions,
    };
    use bevy::prelude::{Quat, Vec3};

    fn flat_world() -> WorldData {
        let layout = crate::world::WorldConfig::default().chunk_layout();
        let mut world = WorldData::new(layout);
        world.set_authored_extent(ChunkExtent {
            min: ChunkCoord::new(0, 0),
            max: ChunkCoord::new(1, 1),
        });
        world
    }

    fn test_inventory_ctx() -> &'static InventoryCatalogCtx<'static> {
        static CTX: std::sync::OnceLock<InventoryCatalogCtx<'static>> = std::sync::OnceLock::new();
        CTX.get_or_init(|| {
            let categories = crate::world::ItemCategoryCatalog::from_definitions(
                starter_item_category_definitions(),
            )
            .unwrap();
            let items = crate::world::ItemCatalog::from_definitions(
                starter_item_definitions(),
                &categories,
            )
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

    #[test]
    fn player_enable_sets_enabled_and_player_controlled() {
        let mut world = flat_world();
        let categories = BuildingCategoryCatalog::default();
        let building_catalog =
            BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
        let operation_catalog =
            OperationCatalog::from_definitions(starter_operation_definitions()).unwrap();
        let farm = create_building_with_inventory(
            &building_catalog,
            &mut world,
            &BuildingDefinitionId::new("prispod_farm"),
            pos(10.0, 10.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
            test_inventory_ctx(),
        )
        .unwrap()
        .id;
        {
            let store = world.building_production_store_mut();
            let def = building_catalog
                .get(&BuildingDefinitionId::new("prispod_farm"))
                .unwrap();
            store.ensure_policy_for_building(farm, def, &operation_catalog);
            store.get_policy_mut(farm).enabled = false;
            store.get_policy_mut(farm).control_source = ControlSource::AIControlled;
        }

        apply_player_production_enabled(
            &mut world,
            &building_catalog,
            &operation_catalog,
            farm,
            true,
        )
        .unwrap();
        let policy = world.building_production_store().get_policy(farm).unwrap();
        assert!(policy.enabled);
        assert_eq!(policy.control_source, ControlSource::PlayerControlled);
    }

    #[test]
    fn player_disable_sets_disabled_and_player_controlled() {
        let mut world = flat_world();
        let categories = BuildingCategoryCatalog::default();
        let building_catalog =
            BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
        let operation_catalog =
            OperationCatalog::from_definitions(starter_operation_definitions()).unwrap();
        let farm = create_building_with_inventory(
            &building_catalog,
            &mut world,
            &BuildingDefinitionId::new("prispod_farm"),
            pos(10.0, 10.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
            test_inventory_ctx(),
        )
        .unwrap()
        .id;
        {
            let store = world.building_production_store_mut();
            let def = building_catalog
                .get(&BuildingDefinitionId::new("prispod_farm"))
                .unwrap();
            store.ensure_policy_for_building(farm, def, &operation_catalog);
            store.get_policy_mut(farm).enabled = true;
            store.get_policy_mut(farm).control_source = ControlSource::AIControlled;
        }

        apply_player_production_enabled(
            &mut world,
            &building_catalog,
            &operation_catalog,
            farm,
            false,
        )
        .unwrap();
        let policy = world.building_production_store().get_policy(farm).unwrap();
        assert!(!policy.enabled);
        assert_eq!(policy.control_source, ControlSource::PlayerControlled);
    }

    #[test]
    fn player_operation_selection_sets_player_controlled() {
        let mut world = flat_world();
        let categories = BuildingCategoryCatalog::default();
        let building_catalog =
            BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
        let operation_catalog =
            OperationCatalog::from_definitions(starter_operation_definitions()).unwrap();
        let workbench = create_building_with_inventory(
            &building_catalog,
            &mut world,
            &BuildingDefinitionId::new("workbench"),
            pos(20.0, 20.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
            test_inventory_ctx(),
        )
        .unwrap()
        .id;
        {
            let store = world.building_production_store_mut();
            let def = building_catalog
                .get(&BuildingDefinitionId::new("workbench"))
                .unwrap();
            store.ensure_policy_for_building(workbench, def, &operation_catalog);
            store.get_policy_mut(workbench).control_source = ControlSource::AIControlled;
        }

        apply_player_production_selected_operation(
            &mut world,
            &building_catalog,
            &operation_catalog,
            workbench,
            OperationDefinitionId::new("research"),
        )
        .unwrap();
        let policy = world
            .building_production_store()
            .get_policy(workbench)
            .unwrap();
        assert_eq!(
            policy.selected_operation.as_ref().map(|op| op.as_str()),
            Some("research")
        );
        assert_eq!(policy.control_source, ControlSource::PlayerControlled);
    }

    #[test]
    fn operation_change_does_not_alter_inventory_bindings_or_contents() {
        let mut world = flat_world();
        let categories = BuildingCategoryCatalog::default();
        let building_catalog =
            BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
        let operation_catalog =
            OperationCatalog::from_definitions(starter_operation_definitions()).unwrap();
        let workbench = create_building_with_inventory(
            &building_catalog,
            &mut world,
            &BuildingDefinitionId::new("workbench"),
            pos(30.0, 30.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
            test_inventory_ctx(),
        )
        .unwrap()
        .id;
        {
            let store = world.building_production_store_mut();
            let def = building_catalog
                .get(&BuildingDefinitionId::new("workbench"))
                .unwrap();
            store.ensure_policy_for_building(workbench, def, &operation_catalog);
        }
        let bindings_before: Vec<_> =
            building_inventory_bindings(world.building_inventory_binding_store(), workbench)
                .into_iter()
                .cloned()
                .collect();
        let inventory_ids_before: Vec<_> = bindings_before
            .iter()
            .map(|binding| binding.inventory_id)
            .collect();

        apply_player_production_selected_operation(
            &mut world,
            &building_catalog,
            &operation_catalog,
            workbench,
            OperationDefinitionId::new("research"),
        )
        .unwrap();

        let bindings_after: Vec<_> =
            building_inventory_bindings(world.building_inventory_binding_store(), workbench)
                .into_iter()
                .cloned()
                .collect();
        assert_eq!(bindings_before, bindings_after);
        let contents_before: Vec<_> = inventory_ids_before
            .iter()
            .map(|inventory_id| {
                world
                    .inventory_store()
                    .get(*inventory_id)
                    .unwrap()
                    .placed_entries()
                    .len()
            })
            .collect();
        let contents_after: Vec<_> = inventory_ids_before
            .iter()
            .map(|inventory_id| {
                world
                    .inventory_store()
                    .get(*inventory_id)
                    .unwrap()
                    .placed_entries()
                    .len()
            })
            .collect();
        assert_eq!(contents_before, contents_after);
    }
}
