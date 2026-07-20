//! Building container inventory attachment, access, and destruction spill (ADR-091 I5).

use super::catalog::BuildingCatalog;
use super::container_access::{
    ContainerAccessPolicy, InventoryAccessDenialReason, InventoryAccessResult,
};
use super::id::BuildingId;
use super::interaction_profile::BuildingInteractionProfileCatalog;
use super::inventory_binding::{
    BuildingInventoryBinding, BuildingInventoryBindingId, BuildingInventoryBindingSet,
    effective_inventory_binding_definitions, primary_building_inventory_id,
};
use super::inventory_error::BuildingInventoryError;
use super::record::BuildingRecord;
use crate::world::inventory::{
    InventoryCatalogCtx, InventoryError, InventoryId, InventoryOwnerRef, RemovedInventoryContents,
    create_inventory, remove_owned_inventory,
};
use crate::world::item_pile::{
    ItemPileSettings, PileOwnership, SpillReport, spill_inventory_to_world_piles,
};
use crate::world::unit::UnitId;
use crate::world::{
    BuildingDefinition, SpaceId, WorldData, WorldPosition, is_building_operational,
};

/// How building removal handles container contents (ADR-091 I5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingInventoryRemovalPolicy {
    /// Spill surviving contents to world piles (destruction default).
    SpillToWorld,
    /// Delete inventory contents without world spill (dev delete).
    DeleteContents,
    /// Remove building without spill side effects (scene teardown).
    TeardownWithoutSpill,
}

/// Optional context for inventory-aware building destruction/removal.
pub struct BuildingInventoryContext<'a> {
    pub ctx: &'a InventoryCatalogCtx<'a>,
    pub pile_settings: &'a ItemPileSettings,
    pub interaction_catalog: &'a BuildingInteractionProfileCatalog,
    pub tick: u64,
}

/// Bundled cleanup inputs for building inventory spill/delete (ADR-091 I5).
pub type BuildingInventoryCleanup<'a> = BuildingInventoryContext<'a>;

pub fn create_building_inventory(
    inventory_store: &mut crate::world::InventoryStore,
    ctx: &InventoryCatalogCtx<'_>,
    profile_id: crate::world::InventoryProfileId,
    building_id: BuildingId,
) -> Result<InventoryId, InventoryError> {
    create_inventory(
        inventory_store,
        ctx,
        profile_id,
        InventoryOwnerRef::Building(building_id),
    )
}

pub fn attach_inventory_on_building_create(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    building: &mut BuildingRecord,
    definition: &BuildingDefinition,
) -> Result<(), BuildingInventoryError> {
    let binding_definitions = effective_inventory_binding_definitions(definition);
    if binding_definitions.is_empty() {
        building.inventory_id = None;
        world.building_inventory_binding_store_mut().remove(building.id);
        return Ok(());
    }

    let mut runtime_bindings = Vec::with_capacity(binding_definitions.len());
    for binding_definition in binding_definitions {
        ctx.require_profile(&binding_definition.profile_id).map_err(|_| {
            BuildingInventoryError::InventoryProfileMissing {
                building_id: building.id,
                profile_id: binding_definition.profile_id.clone(),
            }
        })?;
        let inventory_id = create_building_inventory(
            world.inventory_store_mut(),
            ctx,
            binding_definition.profile_id.clone(),
            building.id,
        )?;
        runtime_bindings.push(
            BuildingInventoryBinding::new(
                binding_definition.binding_id.clone(),
                binding_definition.role,
                inventory_id,
            )
            .with_label(binding_definition.label.clone().unwrap_or_default())
            .with_default(binding_definition.is_default),
        );
    }

    building.inventory_id = resolve_legacy_inventory_id(definition, &runtime_bindings);
    world
        .building_inventory_binding_store_mut()
        .set(
            building.id,
            BuildingInventoryBindingSet::from_bindings(runtime_bindings),
        );
    crate::world::register_building_logistics_endpoints(world, definition, building.id);
    Ok(())
}

fn resolve_legacy_inventory_id(
    definition: &BuildingDefinition,
    bindings: &[BuildingInventoryBinding],
) -> Option<InventoryId> {
    if let Some(default_id) = &definition.default_inventory_binding_id {
        if let Some(binding) = bindings.iter().find(|binding| &binding.binding_id == default_id) {
            return Some(binding.inventory_id);
        }
        return None;
    }
    if let Some(binding) = bindings.iter().find(|binding| binding.is_default) {
        return Some(binding.inventory_id);
    }
    if bindings.len() == 1 {
        return Some(bindings[0].inventory_id);
    }
    None
}

pub fn cleanup_building_inventory_on_delete(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    building: &BuildingRecord,
) -> Result<RemovedInventoryContents, BuildingInventoryError> {
    let mut destroyed_instances = Vec::new();
    let mut last_inventory_id = None;

    let binding_ids: Vec<InventoryId> = world
        .building_inventory_binding_store()
        .get(building.id)
        .map(|set| {
            set.bindings()
                .iter()
                .map(|binding| binding.inventory_id)
                .collect()
        })
        .unwrap_or_default();

    if !binding_ids.is_empty() {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        for inventory_id in binding_ids {
            let removed = remove_owned_inventory(
                inventory_store,
                instance_store,
                ctx,
                inventory_id,
                InventoryOwnerRef::Building(building.id),
            )
            .map_err(BuildingInventoryError::from)?;
            destroyed_instances.extend(removed.destroyed_instance_ids);
            last_inventory_id = Some(inventory_id);
        }
        world
            .building_inventory_binding_store_mut()
            .remove(building.id);
    } else if let Some(inventory_id) = building.inventory_id {
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        let removed = remove_owned_inventory(
            inventory_store,
            instance_store,
            ctx,
            inventory_id,
            InventoryOwnerRef::Building(building.id),
        )
        .map_err(BuildingInventoryError::from)?;
        destroyed_instances = removed.destroyed_instance_ids;
        last_inventory_id = Some(inventory_id);
    }

    Ok(RemovedInventoryContents {
        inventory_id: last_inventory_id,
        destroyed_instance_ids: destroyed_instances,
    })
}

pub fn validate_building_inventory_owner(
    world: &WorldData,
    building_id: BuildingId,
) -> Result<(), InventoryError> {
    let Some(record) = world.get_building(building_id) else {
        return Ok(());
    };
    let Some(inventory_id) = record.inventory_id else {
        return Ok(());
    };
    let inventory = world
        .inventory_store()
        .get(inventory_id)
        .ok_or(InventoryError::InventoryNotFound(inventory_id))?;
    match inventory.owner() {
        InventoryOwnerRef::Building(owner) if owner == &building_id => Ok(()),
        _ => Err(InventoryError::OwnerMismatch {
            inventory_id,
            expected: InventoryOwnerRef::Building(building_id),
        }),
    }
}

pub fn building_inventory_operational(record: &BuildingRecord) -> bool {
    record.inventory_id.is_some() && is_building_operational(record)
}

fn building_inventory_ids(world: &WorldData, building: &BuildingRecord) -> Vec<InventoryId> {
    world
        .building_inventory_binding_store()
        .get(building.id)
        .map(|set| {
            set.bindings()
                .iter()
                .map(|binding| binding.inventory_id)
                .collect()
        })
        .filter(|ids: &Vec<InventoryId>| !ids.is_empty())
        .unwrap_or_else(|| building.inventory_id.into_iter().collect())
}

pub fn can_unit_access_building_inventory(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    unit_id: UnitId,
    building_id: BuildingId,
) -> InventoryAccessResult {
    let Some(unit) = world.get_unit(unit_id) else {
        return InventoryAccessResult::Denied(InventoryAccessDenialReason::RequesterMissing(
            unit_id,
        ));
    };
    let Some(building) = world.get_building(building_id) else {
        return InventoryAccessResult::Denied(InventoryAccessDenialReason::BuildingNotFound(
            building_id,
        ));
    };
    let _inventory_id = primary_building_inventory_id(world, building_id) else {
        return InventoryAccessResult::Denied(InventoryAccessDenialReason::BuildingHasNoInventory);
    };
    if !building_inventory_operational(building) {
        return InventoryAccessResult::Denied(InventoryAccessDenialReason::BuildingNotOperational);
    }
    if building.container_locked {
        return InventoryAccessResult::Denied(InventoryAccessDenialReason::ContainerLocked);
    }
    let definition = match building_catalog.get(&building.definition_id) {
        Some(def) => def,
        None => {
            return InventoryAccessResult::Denied(InventoryAccessDenialReason::BuildingNotFound(
                building_id,
            ));
        }
    };
    let policy = definition.inventory_access_policy;
    if policy.allows(building.ownership, unit, false) {
        InventoryAccessResult::Allowed
    } else {
        InventoryAccessResult::Denied(InventoryAccessDenialReason::PolicyDenied)
    }
}

pub fn can_unit_access_inventory(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    unit_id: UnitId,
    inventory_id: InventoryId,
) -> InventoryAccessResult {
    let inventory = match world.inventory_store().get(inventory_id) {
        Some(record) => record,
        None => {
            return InventoryAccessResult::Denied(InventoryAccessDenialReason::InventoryMissing);
        }
    };
    match inventory.owner() {
        InventoryOwnerRef::Building(building_id) => {
            can_unit_access_building_inventory(world, building_catalog, unit_id, *building_id)
        }
        InventoryOwnerRef::Unit(unit) if unit == &unit_id => InventoryAccessResult::Allowed,
        InventoryOwnerRef::Corpse(_) => {
            InventoryAccessResult::Denied(InventoryAccessDenialReason::PolicyDenied)
        }
        _ => InventoryAccessResult::Denied(InventoryAccessDenialReason::PolicyDenied),
    }
}

pub fn spill_position_for_building(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    building: &BuildingRecord,
) -> (WorldPosition, SpaceId) {
    let layout = world.layout();
    let space_id = building
        .interior
        .interior_space_id
        .unwrap_or(SpaceId::SURFACE);
    if let Some(definition) = building_catalog.get(&building.definition_id) {
        if let Some(profile) = interaction_catalog.profile_for_definition(definition) {
            if let Some(point_key) = definition.inventory_interaction_point_key.as_deref() {
                if let Some(point) = profile.points.iter().find(|p| p.key == point_key) {
                    return (
                        super::interaction_profile::interaction_point_world_position(
                            building, layout, point,
                        ),
                        space_id,
                    );
                }
            }
            if let Some(point) = profile.points.first() {
                return (
                    super::interaction_profile::interaction_point_world_position(
                        building, layout, point,
                    ),
                    space_id,
                );
            }
        }
    }
    (building.placement.position, space_id)
}

pub fn spill_building_inventory(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    inventory_ctx: &BuildingInventoryContext<'_>,
    building: &BuildingRecord,
) -> Result<Option<SpillReport>, BuildingInventoryError> {
    let inventory_ids = building_inventory_ids(world, building);
    if inventory_ids.is_empty() {
        return Ok(None);
    }

    let (position, space_id) =
        spill_position_for_building(world, building_catalog, interaction_catalog, building);
    let ownership = PileOwnership {
        owner_id: building.ownership.owner_id,
        team_id: building.ownership.team_id,
        affiliation: building.ownership.affiliation,
    };

    let mut combined: Option<SpillReport> = None;
    for inventory_id in inventory_ids {
        if world
            .inventory_store()
            .get(inventory_id)
            .is_none_or(|record| record.placed_entries().is_empty())
        {
            continue;
        }
        let report = spill_inventory_to_world_piles(
            world,
            inventory_ctx.ctx,
            inventory_ctx.pile_settings,
            inventory_id,
            position,
            space_id,
            ownership.clone(),
            inventory_ctx.tick,
        )?;
        combined = Some(match combined {
            Some(mut existing) => {
                existing.spilled_entries += report.spilled_entries;
                existing
            }
            None => report,
        });
    }

    cleanup_building_inventory_on_delete(world, inventory_ctx.ctx, building)?;
    clear_building_inventory_link(world, building.id);
    Ok(combined)
}

fn clear_building_inventory_link(world: &mut WorldData, building_id: super::id::BuildingId) {
    world.building_inventory_binding_store_mut().remove(building_id);
    world.mutate_building(building_id, |record| record.inventory_id = None);
}

pub fn finalize_building_inventory_removal(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    inventory_ctx: Option<&BuildingInventoryCleanup<'_>>,
    building: &BuildingRecord,
    policy: BuildingInventoryRemovalPolicy,
) -> Result<Option<SpillReport>, BuildingInventoryError> {
    let definition = building_catalog
        .get(&building.definition_id)
        .ok_or(BuildingInventoryError::BuildingNotFound(building.id))?;
    match policy {
        BuildingInventoryRemovalPolicy::SpillToWorld => {
            let Some(ctx) = inventory_ctx else {
                return Err(BuildingInventoryError::RemovalPolicyMissing);
            };
            if definition.spill_on_destroy {
                spill_building_inventory(
                    world,
                    building_catalog,
                    interaction_catalog,
                    ctx,
                    building,
                )
            } else {
                cleanup_building_inventory_on_delete(world, ctx.ctx, building)?;
                clear_building_inventory_link(world, building.id);
                Ok(None)
            }
        }
        BuildingInventoryRemovalPolicy::DeleteContents => {
            let ctx = inventory_ctx.ok_or(BuildingInventoryError::RemovalPolicyMissing)?;
            cleanup_building_inventory_on_delete(world, ctx.ctx, building)?;
            clear_building_inventory_link(world, building.id);
            Ok(None)
        }
        BuildingInventoryRemovalPolicy::TeardownWithoutSpill => {
            if let Some(ctx) = inventory_ctx {
                cleanup_building_inventory_on_delete(world, ctx.ctx, building)?;
                clear_building_inventory_link(world, building.id);
            }
            Ok(None)
        }
    }
}

pub fn set_building_container_locked(
    world: &mut WorldData,
    building_id: BuildingId,
    locked: bool,
) -> Result<(), BuildingInventoryError> {
    world
        .mutate_building(building_id, |record| record.container_locked = locked)
        .ok_or(BuildingInventoryError::BuildingNotFound(building_id))?;
    Ok(())
}

pub fn building_container_access_policy(definition: &BuildingDefinition) -> ContainerAccessPolicy {
    definition.inventory_access_policy
}

/// Validate building↔inventory owner links across the world (ADR-091 I5).
pub fn validate_building_inventory_links(world: &WorldData) -> Vec<BuildingInventoryError> {
    use std::collections::HashSet;

    let mut errors = Vec::new();
    let mut inventory_to_building: HashSet<InventoryId> = HashSet::new();

    for building_id in world.sorted_building_ids() {
        let Some(record) = world.get_building(building_id) else {
            continue;
        };

        if let Some(binding_set) = world.building_inventory_binding_store().get(building_id) {
            let mut claimed = HashSet::new();
            for binding in binding_set.bindings() {
                if !claimed.insert(binding.inventory_id) {
                    errors.push(BuildingInventoryError::OrphanedBuildingInventory {
                        building_id,
                        inventory_id: binding.inventory_id,
                    });
                }
                if let Err(error) = validate_binding_inventory_owner(
                    world,
                    building_id,
                    binding.inventory_id,
                ) {
                    errors.push(error);
                }
            }
            continue;
        }

        let Some(inventory_id) = record.inventory_id else {
            continue;
        };
        if !inventory_to_building.insert(inventory_id) {
            errors.push(BuildingInventoryError::OrphanedBuildingInventory {
                building_id,
                inventory_id,
            });
        }
        if let Err(error) = validate_building_inventory_owner(world, building_id) {
            errors.push(match error {
                InventoryError::OwnerMismatch {
                    inventory_id,
                    expected: InventoryOwnerRef::Building(owner),
                } => BuildingInventoryError::BuildingInventoryOwnerMismatch {
                    building_id: owner,
                    inventory_id,
                },
                _ => BuildingInventoryError::Inventory(error),
            });
        }
    }

    for inventory_id in world.inventory_store().sorted_inventory_ids() {
        let Some(record) = world.inventory_store().get(inventory_id) else {
            continue;
        };
        if let InventoryOwnerRef::Building(building_id) = record.owner() {
            let building = world.get_building(*building_id);
            let linked = building.is_some_and(|b| {
                if b.inventory_id == Some(inventory_id) {
                    return true;
                }
                world
                    .building_inventory_binding_store()
                    .get(*building_id)
                    .is_some_and(|set| {
                        set.bindings()
                            .iter()
                            .any(|binding| binding.inventory_id == inventory_id)
                    })
            });
            if !linked {
                errors.push(BuildingInventoryError::OrphanedBuildingInventory {
                    building_id: *building_id,
                    inventory_id,
                });
            }
        }
    }

    errors
}

fn validate_binding_inventory_owner(
    world: &WorldData,
    building_id: BuildingId,
    inventory_id: InventoryId,
) -> Result<(), BuildingInventoryError> {
    let inventory = world
        .inventory_store()
        .get(inventory_id)
        .ok_or(InventoryError::InventoryNotFound(inventory_id))?;
    match inventory.owner() {
        InventoryOwnerRef::Building(owner) if owner == &building_id => Ok(()),
        _ => Err(BuildingInventoryError::BuildingInventoryOwnerMismatch {
            building_id,
            inventory_id,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::building::create_building_with_inventory;
    use crate::world::inventory::{InventoryOwnerRef, place_stack_first_fit};
    use crate::world::{
        Affiliation, BuildingCatalog, BuildingDefinitionId, BuildingInteractionProfileCatalog,
        BuildingLifecycleState, BuildingOwnership, BuildingSource, ChunkCoord, ChunkData, ChunkId,
        ChunkLayout, Heightfield, InventoryProfileCatalog, ItemCatalog, ItemCategoryCatalog,
        ItemDefinitionId, ItemPileSettings, LocalPosition, UnitCatalog, UnitDefinitionId,
        UnitOwnership, UnitSource, create_unit_with_inventory, starter_building_definitions,
        starter_inventory_profile_definitions, starter_item_category_definitions,
        starter_item_definitions, starter_unit_definitions,
    };
    use bevy::prelude::{Quat, Vec3};

    fn flat_world() -> WorldData {
        let mut world = WorldData::new(ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        let heightfield = Heightfield::from_samples(65, 4.0, vec![0.0; 65 * 65]).unwrap();
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

    fn test_ctx() -> &'static InventoryCatalogCtx<'static> {
        static CTX: std::sync::OnceLock<InventoryCatalogCtx<'static>> = std::sync::OnceLock::new();
        CTX.get_or_init(|| {
            let categories =
                ItemCategoryCatalog::from_definitions(starter_item_category_definitions()).unwrap();
            let items =
                ItemCatalog::from_definitions(starter_item_definitions(), &categories).unwrap();
            let profiles =
                InventoryProfileCatalog::from_definitions(starter_inventory_profile_definitions())
                    .unwrap();
            let items = Box::leak(Box::new(items));
            let categories = Box::leak(Box::new(categories));
            let profiles = Box::leak(Box::new(profiles));
            InventoryCatalogCtx::new(items, categories, profiles)
        })
    }

    fn chest_catalog() -> BuildingCatalog {
        let categories = crate::world::BuildingCategoryCatalog::default();
        BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap()
    }

    #[test]
    fn definition_without_profile_has_no_inventory() {
        let catalog = BuildingCatalog::default();
        let mut world = flat_world();
        let record = create_building_with_inventory(
            &catalog,
            &mut world,
            &BuildingDefinitionId::new("hut"),
            pos(1.0, 1.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::neutral(),
            None,
            test_ctx(),
        )
        .unwrap();
        assert!(record.inventory_id.is_none());
    }

    #[test]
    fn chest_gets_inventory_on_create() {
        let catalog = chest_catalog();
        let mut world = flat_world();
        let ctx = test_ctx();
        let record = create_building_with_inventory(
            &catalog,
            &mut world,
            &BuildingDefinitionId::new("storage_chest"),
            pos(2.0, 2.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::neutral(),
            None,
            ctx,
        )
        .unwrap();
        let inventory_id = record.inventory_id.expect("inventory");
        let inventory = world.inventory_store().get(inventory_id).unwrap();
        assert_eq!(*inventory.owner(), InventoryOwnerRef::Building(record.id));
        validate_building_inventory_owner(&world, record.id).unwrap();
    }

    #[test]
    fn incomplete_building_inventory_access_blocked() {
        let catalog = chest_catalog();
        let mut world = flat_world();
        let ctx = test_ctx();
        let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let unit = create_unit_with_inventory(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(3.0, 3.0),
            UnitSource::Authored,
            UnitOwnership::with_affiliation(Affiliation::Player),
            ctx,
        )
        .unwrap();
        let mut record = create_building_with_inventory(
            &catalog,
            &mut world,
            &BuildingDefinitionId::new("storage_chest"),
            pos(4.0, 4.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
            ctx,
        )
        .unwrap();
        world.mutate_building(record.id, |r| {
            r.lifecycle_state = BuildingLifecycleState::Planned;
        });
        let access = can_unit_access_building_inventory(&world, &catalog, unit.id, record.id);
        assert!(matches!(
            access,
            InventoryAccessResult::Denied(InventoryAccessDenialReason::BuildingNotOperational)
        ));
        world.mutate_building(record.id, |r| {
            r.lifecycle_state = BuildingLifecycleState::Complete;
        });
        assert!(
            can_unit_access_building_inventory(&world, &catalog, unit.id, record.id).is_allowed()
        );
    }

    #[test]
    fn destruction_spills_contents() {
        let catalog = chest_catalog();
        let interaction = BuildingInteractionProfileCatalog::default();
        let mut world = flat_world();
        let ctx = test_ctx();
        let settings = ItemPileSettings::default();
        let record = create_building_with_inventory(
            &catalog,
            &mut world,
            &BuildingDefinitionId::new("storage_chest"),
            pos(5.0, 5.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
            ctx,
        )
        .unwrap();
        let inventory_id = record.inventory_id.unwrap();
        let (inventory_store, instance_store) = world.inventory_runtime_mut();
        place_stack_first_fit(
            inventory_store,
            instance_store,
            ctx,
            inventory_id,
            ItemDefinitionId::new("gold"),
            7,
        )
        .unwrap();
        let spill = spill_building_inventory(
            &mut world,
            &catalog,
            &interaction,
            &BuildingInventoryContext {
                ctx,
                pile_settings: &settings,
                interaction_catalog: &interaction,
                tick: 1,
            },
            &record,
        )
        .unwrap()
        .expect("spill");
        assert_eq!(spill.spilled_entries, 1);
        assert!(world.inventory_store().get(inventory_id).is_none());
        assert!(!world.item_pile_store().sorted_item_pile_ids().is_empty());
    }

    #[test]
    fn locked_container_denies_access() {
        let catalog = chest_catalog();
        let mut world = flat_world();
        let ctx = test_ctx();
        let unit_catalog = UnitCatalog::from_definitions(starter_unit_definitions()).unwrap();
        let unit = create_unit_with_inventory(
            &unit_catalog,
            &mut world,
            &UnitDefinitionId::new("bandit"),
            pos(6.0, 6.0),
            UnitSource::Authored,
            UnitOwnership::with_affiliation(Affiliation::Player),
            ctx,
        )
        .unwrap();
        let record = create_building_with_inventory(
            &catalog,
            &mut world,
            &BuildingDefinitionId::new("storage_chest"),
            pos(7.0, 7.0),
            Quat::IDENTITY,
            BuildingSource::Authored,
            BuildingOwnership::with_affiliation(Affiliation::Player),
            None,
            ctx,
        )
        .unwrap();
        set_building_container_locked(&mut world, record.id, true).unwrap();
        let access = can_unit_access_building_inventory(&world, &catalog, unit.id, record.id);
        assert!(matches!(
            access,
            InventoryAccessResult::Denied(InventoryAccessDenialReason::ContainerLocked)
        ));
    }
}
