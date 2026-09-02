//! Settlement membership authority (ADR-133 Phase 2).
//!
//! `UnitRecord.settlement_id` and `BuildingRecord.settlement_id` are the sole
//! persistent membership authorities. [`SettlementStore`] unit/building indexes
//! are derived caches only.

use std::fmt;

use crate::world::{BuildingId, UnitId, WorldData, WorldPosition, xz_distance};

use super::id::SettlementId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettlementMembershipError {
    SettlementNotFound(SettlementId),
    UnitNotFound(UnitId),
    BuildingNotFound(BuildingId),
    UnitAlreadyAssigned {
        unit_id: UnitId,
        settlement_id: SettlementId,
    },
    BuildingAlreadyAssigned {
        building_id: BuildingId,
        settlement_id: SettlementId,
    },
    NoSettlementAtPosition,
}

impl fmt::Display for SettlementMembershipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SettlementNotFound(id) => write!(f, "settlement {id:?} not found"),
            Self::UnitNotFound(id) => write!(f, "unit {id:?} not found"),
            Self::BuildingNotFound(id) => write!(f, "building {id:?} not found"),
            Self::UnitAlreadyAssigned {
                unit_id,
                settlement_id,
            } => write!(
                f,
                "unit {unit_id:?} already assigned to settlement {settlement_id:?}"
            ),
            Self::BuildingAlreadyAssigned {
                building_id,
                settlement_id,
            } => write!(
                f,
                "building {building_id:?} already assigned to settlement {settlement_id:?}"
            ),
            Self::NoSettlementAtPosition => write!(f, "no settlement at position"),
        }
    }
}

impl std::error::Error for SettlementMembershipError {}

/// Settlement whose boundary contains `position`, if any. Settlements do not overlap.
pub fn settlement_containing_position(
    world: &WorldData,
    position: WorldPosition,
) -> Option<SettlementId> {
    let layout = world.layout();
    for settlement_id in world.settlement_store().sorted_settlement_ids() {
        let Some(settlement) = world.settlement_store().get_settlement(settlement_id) else {
            continue;
        };
        let distance = xz_distance(position, settlement.center, layout);
        if distance <= settlement.boundary_radius_meters {
            return Some(settlement_id);
        }
    }
    None
}

/// Rebuild all derived membership indexes from authoritative record fields.
pub fn rebuild_settlement_membership_indexes(world: &mut WorldData) {
    world.settlement_store_mut().clear_membership_indexes();
    for unit_id in world.sorted_unit_ids() {
        let Some(record) = world.get_unit(unit_id) else {
            continue;
        };
        if let Some(settlement_id) = record.settlement_id {
            world
                .settlement_store_mut()
                .reindex_unit_membership(unit_id, Some(settlement_id));
        }
    }
    for building_id in world.sorted_building_ids() {
        let Some(record) = world.get_building(building_id) else {
            continue;
        };
        if let Some(settlement_id) = record.settlement_id {
            world
                .settlement_store_mut()
                .reindex_building_membership(building_id, Some(settlement_id));
        }
    }
}

/// Assign or clear explicit unit membership.
pub fn assign_unit_settlement(
    world: &mut WorldData,
    unit_id: UnitId,
    settlement_id: Option<SettlementId>,
) -> Result<(), SettlementMembershipError> {
    if let Some(settlement_id) = settlement_id {
        if world
            .settlement_store()
            .get_settlement(settlement_id)
            .is_none()
        {
            return Err(SettlementMembershipError::SettlementNotFound(settlement_id));
        }
    }
    if world.get_unit(unit_id).is_none() {
        return Err(SettlementMembershipError::UnitNotFound(unit_id));
    }
    world
        .mutate_unit(unit_id, |record| record.settlement_id = settlement_id)
        .ok_or(SettlementMembershipError::UnitNotFound(unit_id))?;
    world
        .settlement_store_mut()
        .reindex_unit_membership(unit_id, settlement_id);
    Ok(())
}

/// Assign or clear explicit building membership.
pub fn assign_building_settlement(
    world: &mut WorldData,
    building_id: BuildingId,
    settlement_id: Option<SettlementId>,
) -> Result<(), SettlementMembershipError> {
    if let Some(settlement_id) = settlement_id {
        if world
            .settlement_store()
            .get_settlement(settlement_id)
            .is_none()
        {
            return Err(SettlementMembershipError::SettlementNotFound(settlement_id));
        }
    }
    if world.get_building(building_id).is_none() {
        return Err(SettlementMembershipError::BuildingNotFound(building_id));
    }
    world
        .mutate_building(building_id, |record| record.settlement_id = settlement_id)
        .ok_or(SettlementMembershipError::BuildingNotFound(building_id))?;
    world
        .settlement_store_mut()
        .reindex_building_membership(building_id, settlement_id);
    Ok(())
}

/// Seed building membership once at creation from placement position.
pub fn seed_building_settlement_at_creation(
    world: &mut WorldData,
    building_id: BuildingId,
    position: WorldPosition,
) {
    let settlement_id = settlement_containing_position(world, position);
    if let Some(record) = world.mutate_building(building_id, |record| {
        record.settlement_id = settlement_id;
    }) {
        world
            .settlement_store_mut()
            .reindex_building_membership(building_id, record.settlement_id);
    }
}

/// Assign all `unit_ids` to the settlement whose boundary contains `click_position`.
pub fn assign_selected_units_at_position(
    world: &mut WorldData,
    unit_ids: &[UnitId],
    click_position: WorldPosition,
) -> Result<(SettlementId, usize), SettlementMembershipError> {
    let settlement_id = settlement_containing_position(world, click_position)
        .ok_or(SettlementMembershipError::NoSettlementAtPosition)?;
    let mut assigned = 0usize;
    for &unit_id in unit_ids {
        assign_unit_settlement(world, unit_id, Some(settlement_id))?;
        assigned += 1;
    }
    Ok((settlement_id, assigned))
}

/// Clear unit membership before authoritative removal.
pub fn clear_unit_settlement_on_removal(world: &mut WorldData, unit_id: UnitId) {
    if world
        .get_unit(unit_id)
        .is_some_and(|record| record.settlement_id.is_some())
    {
        let _ = world.mutate_unit(unit_id, |record| record.settlement_id = None);
    }
    world.settlement_store_mut().unlink_unit(unit_id);
}

#[cfg(test)]
mod tests;
