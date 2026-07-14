//! Treasury deposit access checks (ADR-093 I7).

use bevy::prelude::*;

use super::error::TreasuryError;
use super::id::TreasuryId;
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
    let Some(building) = world.get_building(settlement.anchor_building_id) else {
        return TreasuryAccessResult::Denied(TreasuryError::BuildingNotFound(
            settlement.anchor_building_id,
        ));
    };
    if !building_supports_settlement_treasury(
        building_catalog,
        interaction_catalog,
        settlement.anchor_building_id,
        world,
    ) {
        return TreasuryAccessResult::Denied(TreasuryError::BuildingNotSettlementCapable(
            settlement.anchor_building_id,
        ));
    }
    if !policy.allows(settlement.ownership, unit) {
        return TreasuryAccessResult::Denied(TreasuryError::AccessDenied);
    }
    let building_space = settlement_interaction_space(building);
    if unit.current_space_id != building_space {
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
