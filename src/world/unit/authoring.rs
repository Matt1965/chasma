//! Authoritative unit placement API (ADR-027 U2, ADR-051 O1).
//!
//! Operates on [`crate::world::WorldData`] and [`super::catalog::UnitCatalog`].
//! No ECS entities, rendering, movement validation, or save/load.

use bevy::prelude::*;

use super::appearance::{
    definition_has_appearance_support, resolve_canonical_default_appearance,
    validate_unit_appearance, AppearanceError,
};
use super::catalog::UnitCatalog;
use super::id::UnitId;
use super::placement::UnitPlacement;
use super::record::UnitRecord;
use super::source::UnitSource;
use std::sync::OnceLock;

use crate::world::equipment::{
    attach_equipment_on_unit_create, cleanup_unit_equipment_on_delete,
    equipment_slot_profile_definitions, minimal_catalog_ctx,
};
use crate::world::ownership::{UnitOwnership, default_ownership_for_source};
use crate::world::{
    InventoryProfileCatalog, UnitDefinitionId, UnitInsertError, WorldData, WorldPosition,
};

fn default_equipment_profiles() -> &'static InventoryProfileCatalog {
    static PROFILES: OnceLock<InventoryProfileCatalog> = OnceLock::new();
    PROFILES.get_or_init(|| {
        InventoryProfileCatalog::from_definitions(equipment_slot_profile_definitions()).unwrap()
    })
}

/// Why an authoring operation failed (ADR-027 U2).
#[derive(Debug, Clone, PartialEq)]
pub enum UnitAuthoringError {
    DefinitionNotFound(UnitDefinitionId),
    DefinitionDisabled(UnitDefinitionId),
    UnitNotFound(UnitId),
    ChunkPlacementMismatch,
    InventoryAllocationFailed(UnitId),
    AppearanceResolutionFailed {
        definition_id: UnitDefinitionId,
        reason: String,
    },
}

/// Create a unit with explicit runtime ownership.
pub fn create_unit_with_ownership(
    catalog: &UnitCatalog,
    appearance_profiles: &crate::world::AppearanceProfileCatalog,
    world: &mut WorldData,
    definition_id: &UnitDefinitionId,
    position: WorldPosition,
    source: UnitSource,
    ownership: UnitOwnership,
) -> Result<UnitRecord, UnitAuthoringError> {
    create_unit_with_ownership_impl(
        catalog,
        appearance_profiles,
        world,
        definition_id,
        position,
        source,
        ownership,
        None,
        None,
    )
}

/// Create a unit with explicit appearance (CG8 starting squad).
pub fn create_unit_with_ownership_and_appearance(
    catalog: &UnitCatalog,
    appearance_profiles: &crate::world::AppearanceProfileCatalog,
    world: &mut WorldData,
    definition_id: &UnitDefinitionId,
    position: WorldPosition,
    source: UnitSource,
    ownership: UnitOwnership,
    appearance: crate::world::UnitAppearance,
) -> Result<UnitRecord, UnitAuthoringError> {
    create_unit_with_ownership_impl(
        catalog,
        appearance_profiles,
        world,
        definition_id,
        position,
        source,
        ownership,
        None,
        Some(appearance),
    )
}

/// Create a unit and attach an authoritative inventory from its definition profile.
pub fn create_unit_with_inventory(
    catalog: &UnitCatalog,
    appearance_profiles: &crate::world::AppearanceProfileCatalog,
    world: &mut WorldData,
    definition_id: &UnitDefinitionId,
    position: WorldPosition,
    source: UnitSource,
    ownership: UnitOwnership,
    inventory_ctx: &crate::world::InventoryCatalogCtx<'_>,
) -> Result<UnitRecord, UnitAuthoringError> {
    create_unit_with_ownership_impl(
        catalog,
        appearance_profiles,
        world,
        definition_id,
        position,
        source,
        ownership,
        Some(inventory_ctx),
        None,
    )
}

fn create_unit_with_ownership_impl(
    catalog: &UnitCatalog,
    appearance_profiles: &crate::world::AppearanceProfileCatalog,
    world: &mut WorldData,
    definition_id: &UnitDefinitionId,
    position: WorldPosition,
    source: UnitSource,
    ownership: UnitOwnership,
    inventory_ctx: Option<&crate::world::InventoryCatalogCtx<'_>>,
    appearance_override: Option<crate::world::UnitAppearance>,
) -> Result<UnitRecord, UnitAuthoringError> {
    let definition = catalog
        .get(definition_id)
        .ok_or_else(|| UnitAuthoringError::DefinitionNotFound(definition_id.clone()))?;

    if !definition.enabled {
        return Err(UnitAuthoringError::DefinitionDisabled(
            definition_id.clone(),
        ));
    }

    let id = world.allocate_unit_id();
    let mut record = UnitRecord::new(
        id,
        definition.id.clone(),
        UnitPlacement::new(position, Quat::IDENTITY),
        source,
        ownership,
        definition.max_hp,
        definition.faction_id.clone(),
        definition.species_id.clone(),
    );

    let default_profiles = default_equipment_profiles();
    let equipment_profiles = inventory_ctx
        .map(|ctx| ctx.profiles)
        .unwrap_or(default_profiles);
    let equipment =
        attach_equipment_on_unit_create(world.inventory_store_mut(), equipment_profiles, id)
            .map_err(|_| UnitAuthoringError::InventoryAllocationFailed(id))?;
    record.equipment = Some(equipment);

    if let Some(ctx) = inventory_ctx {
        super::inventory::attach_inventory_on_unit_create(world, ctx, &mut record, definition)
            .map_err(|_| UnitAuthoringError::InventoryAllocationFailed(id))?;
    }

    super::self_maintenance::initialize_unit_nutrition(&mut record.nutrition, definition);
    super::work_skill::initialize_unit_work_skills(&mut record.work_skills);
    attach_appearance_on_unit_create(
        definition,
        appearance_profiles,
        &mut record,
        appearance_override,
    )?;

    let chunk = crate::world::ChunkId::new(position.chunk);
    if let Err(error) = world.insert_unit(chunk, record.clone()) {
        if let Some(ctx) = inventory_ctx {
            let _ = super::inventory::cleanup_unit_inventory_on_delete(world, ctx, &record);
            let _ = cleanup_unit_equipment_on_delete(world, ctx, &record);
        } else if record.equipment.is_some() {
            let profiles = default_equipment_profiles();
            let ctx = minimal_catalog_ctx(profiles);
            let _ = cleanup_unit_equipment_on_delete(world, &ctx, &record);
        }
        return Err(match error {
            UnitInsertError::ChunkPlacementMismatch => UnitAuthoringError::ChunkPlacementMismatch,
            UnitInsertError::UnitNotFound => UnitAuthoringError::UnitNotFound(id),
        });
    }

    super::navigation_membership::initialize_unit_navigation_membership(world, id);
    crate::world::settlement::seed_unit_settlement_at_creation(world, id, position);

    Ok(record)
}

fn attach_appearance_on_unit_create(
    definition: &crate::world::UnitDefinition,
    appearance_profiles: &crate::world::AppearanceProfileCatalog,
    record: &mut UnitRecord,
    appearance_override: Option<crate::world::UnitAppearance>,
) -> Result<(), UnitAuthoringError> {
    if !definition_has_appearance_support(definition) {
        record.appearance = None;
        return Ok(());
    }
    let appearance = if let Some(appearance) = appearance_override {
        appearance
    } else {
        resolve_canonical_default_appearance(definition, appearance_profiles)
            .map_err(|error| appearance_authoring_error(definition, error))?
    };
    validate_unit_appearance(&appearance, definition, appearance_profiles)
        .map_err(|error| appearance_authoring_error(definition, error))?;
    record.appearance = Some(appearance);
    Ok(())
}

fn appearance_authoring_error(
    definition: &crate::world::UnitDefinition,
    error: AppearanceError,
) -> UnitAuthoringError {
    UnitAuthoringError::AppearanceResolutionFailed {
        definition_id: definition.id.clone(),
        reason: error.to_string(),
    }
}

/// Create a unit instance using safe default ownership for [`UnitSource`].
///
/// Does **not** derive ownership from catalog `faction_tag`.
pub fn create_unit(
    catalog: &UnitCatalog,
    appearance_profiles: &crate::world::AppearanceProfileCatalog,
    world: &mut WorldData,
    definition_id: &UnitDefinitionId,
    position: WorldPosition,
    source: UnitSource,
) -> Result<UnitRecord, UnitAuthoringError> {
    create_unit_with_ownership(
        catalog,
        appearance_profiles,
        world,
        definition_id,
        position,
        source,
        default_ownership_for_source(source),
    )
}

/// Move an existing unit to a new world position, including cross-chunk moves.
pub fn move_unit(
    world: &mut WorldData,
    id: UnitId,
    new_position: WorldPosition,
) -> Result<UnitRecord, UnitAuthoringError> {
    world
        .relocate_unit(id, new_position)
        .map_err(|error| match error {
            UnitInsertError::ChunkPlacementMismatch => UnitAuthoringError::ChunkPlacementMismatch,
            UnitInsertError::UnitNotFound => UnitAuthoringError::UnitNotFound(id),
        })
}

/// Remove a unit by id, returning the removed record.
pub fn remove_unit(world: &mut WorldData, id: UnitId) -> Result<UnitRecord, UnitAuthoringError> {
    world
        .remove_unit_by_id(id)
        .ok_or(UnitAuthoringError::UnitNotFound(id))
}

/// Borrow a unit record by id.
pub fn lookup_unit(world: &WorldData, id: UnitId) -> Option<&UnitRecord> {
    world.get_unit(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::ownership::{Affiliation, UnitOwnership};
    use crate::world::{ChunkCoord, LocalPosition, UnitCatalog};

    fn layout_world() -> WorldData {
        WorldData::new(crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        })
    }

    fn catalog() -> UnitCatalog {
        UnitCatalog::default()
    }

    fn appearance_profiles() -> crate::world::AppearanceProfileCatalog {
        crate::world::AppearanceProfileCatalog::empty()
    }

    fn position(chunk_x: i32, chunk_z: i32, local: Vec3) -> WorldPosition {
        WorldPosition::new(ChunkCoord::new(chunk_x, chunk_z), LocalPosition::new(local))
    }

    #[test]
    fn create_unit_starts_at_full_hp() {
        let cat = catalog();
        let mut world = layout_world();
        let def_id = UnitDefinitionId::new("wolf");
        let expected_max = cat.get(&def_id).unwrap().max_hp;

        let record = create_unit(
            &cat,
            &appearance_profiles(),
            &mut world,
            &def_id,
            position(0, 0, Vec3::ZERO),
            UnitSource::Authored,
        )
        .unwrap();

        assert_eq!(record.vitals.current_hp, expected_max);
        assert_eq!(record.vitals.max_hp, expected_max);
        assert_eq!(record.combat_state, crate::world::CombatState::Peaceful);
    }

    #[test]
    fn create_unit_from_definition() {
        let cat = catalog();
        let mut world = layout_world();
        let def = UnitDefinitionId::new("wolf");
        let pos = position(1, 2, Vec3::new(64.0, 0.0, 128.0));

        let record = create_unit(
            &cat,
            &appearance_profiles(),
            &mut world,
            &def,
            pos,
            UnitSource::Authored,
        )
        .unwrap();

        assert_eq!(record.definition_id, def);
        assert_eq!(record.placement.position, pos);
        assert_eq!(lookup_unit(&world, record.id).unwrap().id, record.id);
        world.assert_unit_index_consistent();
    }

    #[test]
    fn create_unit_with_ownership_stores_fields() {
        let cat = catalog();
        let mut world = layout_world();
        let ownership = UnitOwnership::player_default();
        let record = create_unit_with_ownership(
            &cat,
            &appearance_profiles(),
            &mut world,
            &UnitDefinitionId::new("wolf"),
            position(0, 0, Vec3::ZERO),
            UnitSource::Authored,
            ownership,
        )
        .unwrap();
        assert_eq!(record.owner_id, ownership.owner_id);
        assert_eq!(record.team_id, ownership.team_id);
        assert_eq!(record.affiliation, Affiliation::Player);
    }

    #[test]
    fn disabled_definition_rejected() {
        let mut cat = UnitCatalog::default();
        let mut def = cat.get(&UnitDefinitionId::new("wolf")).unwrap().clone();
        def.enabled = false;
        cat = UnitCatalog::from_definitions(vec![def]).unwrap();

        let mut world = layout_world();
        let err = create_unit(
            &cat,
            &appearance_profiles(),
            &mut world,
            &UnitDefinitionId::new("wolf"),
            position(0, 0, Vec3::ZERO),
            UnitSource::Authored,
        )
        .unwrap_err();

        assert_eq!(
            err,
            UnitAuthoringError::DefinitionDisabled(UnitDefinitionId::new("wolf"))
        );
    }
}
