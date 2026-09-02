//! Treasury deposit access checks (ADR-093 I7, ADR-133).

use bevy::prelude::*;

use super::error::TreasuryError;
use super::id::{SettlementId, TreasuryId};
use super::record::SettlementOwnership;
use super::store::SettlementStore;
use crate::world::building::{
    BuildingCatalog, BuildingInteractionProfileCatalog, INTERACTION_WORK_RANGE_METERS,
    interaction_point_world_position, is_building_operational,
};
use crate::world::unit::UnitRecord;
use crate::world::{BuildingId, SpaceId, UnitId, WorldData, xz_distance};

/// Who may deposit physical gold into a settlement treasury.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Reflect)]
pub enum TreasuryAccessPolicy {
    #[default]
    OwnerOnly,
    Team,
    Everyone,
}

impl TreasuryAccessPolicy {
    pub fn allows(self, settlement: SettlementOwnership, unit: &UnitRecord) -> bool {
        match self {
            Self::Everyone => true,
            Self::OwnerOnly => settlement
                .owner_id
                .map_or(true, |owner| unit.owner_id == Some(owner)),
            Self::Team => {
                settlement.team_id.is_some() && unit.team_id == settlement.team_id
                    || settlement
                        .owner_id
                        .map_or(false, |owner| unit.owner_id == Some(owner))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreasuryAccessResult {
    Allowed,
    Denied(TreasuryError),
}

impl TreasuryAccessResult {
    pub fn is_allowed(self) -> bool {
        matches!(self, Self::Allowed)
    }
}

pub fn building_supports_settlement_treasury(
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    building_id: BuildingId,
    world: &WorldData,
) -> bool {
    let Some(building) = world.get_building(building_id) else {
        return false;
    };
    if !is_building_operational(building) {
        return false;
    }
    let Some(definition) = building_catalog.get(&building.definition_id) else {
        return false;
    };
    interaction_catalog
        .profile_for_definition(definition)
        .is_some_and(|profile| profile.capabilities.settlement_treasury)
}

pub fn settlement_interaction_space(building: &crate::world::BuildingRecord) -> SpaceId {
    building
        .interior
        .interior_space_id
        .unwrap_or(SpaceId::SURFACE)
}

/// Whether a unit is in any space owned by the building (region-per-space aware).
pub fn unit_is_in_building_space(
    world: &WorldData,
    unit: &UnitRecord,
    building_id: BuildingId,
) -> bool {
    if unit.current_space_id.is_surface() {
        return false;
    }
    world
        .space_registry()
        .building_space_ids(building_id)
        .iter()
        .any(|space_id| *space_id == unit.current_space_id)
}

pub fn can_unit_deposit_to_treasury(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    settlement_store: &SettlementStore,
    unit_id: UnitId,
    treasury_id: TreasuryId,
    policy: TreasuryAccessPolicy,
) -> TreasuryAccessResult {
    let Some(unit) = world.get_unit(unit_id) else {
        return TreasuryAccessResult::Denied(TreasuryError::RequesterMissing(unit_id));
    };
    let Some(treasury) = settlement_store.get_treasury(treasury_id) else {
        return TreasuryAccessResult::Denied(TreasuryError::TreasuryNotFound(treasury_id));
    };
    let Some(settlement) = settlement_store.get_settlement(treasury.settlement_id) else {
        return TreasuryAccessResult::Denied(TreasuryError::SettlementNotFound(
            treasury.settlement_id,
        ));
    };
    let Some(building_id) = treasury_deposit_building_id(
        world,
        building_catalog,
        interaction_catalog,
        settlement_store,
        settlement.id,
    ) else {
        return TreasuryAccessResult::Denied(TreasuryError::BuildingNotFound(
            settlement_store
                .buildings_for_settlement(settlement.id)
                .first()
                .copied()
                .unwrap_or(BuildingId::new(0)),
        ));
    };
    let Some(building) = world.get_building(building_id) else {
        return TreasuryAccessResult::Denied(TreasuryError::BuildingNotFound(building_id));
    };
    if !building_supports_settlement_treasury(
        building_catalog,
        interaction_catalog,
        building_id,
        world,
    ) {
        return TreasuryAccessResult::Denied(TreasuryError::BuildingNotSettlementCapable(
            building_id,
        ));
    }
    if !policy.allows(settlement.ownership, unit) {
        return TreasuryAccessResult::Denied(TreasuryError::AccessDenied);
    }
    if !unit_is_in_building_space(world, unit, building_id) {
        return TreasuryAccessResult::Denied(TreasuryError::WrongSpace);
    }
    let layout = world.layout();
    let interaction_position =
        settlement_interaction_position(world, building_catalog, interaction_catalog, building);
    let distance = xz_distance(unit.placement.position, interaction_position, layout);
    if distance > INTERACTION_WORK_RANGE_METERS {
        return TreasuryAccessResult::Denied(TreasuryError::OutOfRange);
    }
    TreasuryAccessResult::Allowed
}

/// Treasury-capable building linked to this settlement (legacy cache seam).
///
/// Anchor-only settlements with no linked treasury building have no deposit interaction yet.
fn treasury_deposit_building_id(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    settlement_store: &SettlementStore,
    settlement_id: SettlementId,
) -> Option<BuildingId> {
    settlement_store
        .buildings_for_settlement(settlement_id)
        .into_iter()
        .find(|building_id| {
            building_supports_settlement_treasury(
                building_catalog,
                interaction_catalog,
                *building_id,
                world,
            )
        })
}

pub fn settlement_interaction_position(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    building: &crate::world::BuildingRecord,
) -> crate::world::WorldPosition {
    let layout = world.layout();
    if let Some(definition) = building_catalog.get(&building.definition_id) {
        if let Some(profile) = interaction_catalog.profile_for_definition(definition) {
            if let Some(point) = profile
                .points
                .iter()
                .find(|p| p.key == "treasury")
                .or_else(|| profile.points.first())
            {
                return interaction_point_world_position(building, layout, point);
            }
        }
    }
    building.placement.position
}
