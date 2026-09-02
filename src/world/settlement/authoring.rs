//! Settlement treasury creation (ADR-093 I7, ADR-133).

use super::access::building_supports_settlement_treasury;
use super::anchor::{
    SettlementAnchorRecord, SettlementCreationError, initial_boundary_radius_meters,
    settlement_overlaps_existing,
};
use super::error::TreasuryError;
use super::id::{SettlementId, TreasuryId};
use super::record::{SettlementOwnership, SettlementRecord, SettlementTreasuryRecord};
use super::state::SettlementKind;
use crate::world::building::{BuildingCatalog, BuildingInteractionProfileCatalog};
use crate::world::{BuildingId, WorldData, WorldPosition, xz_distance};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateSettlementReport {
    pub settlement_id: SettlementId,
    pub treasury_id: TreasuryId,
    pub anchor_id: super::anchor::SettlementAnchorId,
}

/// Canonical settlement creation seam (ADR-133). Every surface calls this.
pub fn create_settlement(
    world: &mut WorldData,
    center: WorldPosition,
    display_name: impl Into<String>,
    ownership: SettlementOwnership,
    kind: SettlementKind,
    boundary_radius_meters: Option<f32>,
    interaction_position: Option<WorldPosition>,
    created_tick: u64,
) -> Result<CreateSettlementReport, SettlementCreationError> {
    let boundary_radius_meters =
        boundary_radius_meters.unwrap_or_else(|| initial_boundary_radius_meters(kind));
    if !boundary_radius_meters.is_finite() || boundary_radius_meters <= 0.0 {
        return Err(SettlementCreationError::Treasury(
            TreasuryError::InvalidQuantity { requested: 0 },
        ));
    }

    let layout = world.layout();
    for existing_id in world.settlement_store().sorted_settlement_ids() {
        let Some(existing) = world.settlement_store().get_settlement(existing_id) else {
            continue;
        };
        if settlement_overlaps_existing(center, boundary_radius_meters, existing, layout) {
            let distance = xz_distance(center, existing.center, layout);
            let required = super::anchor::required_center_separation_meters(
                boundary_radius_meters,
                existing.boundary_radius_meters,
            );
            return Err(SettlementCreationError::OverlapsExisting {
                existing_settlement_id: existing.id,
                distance_meters: distance,
                required_separation_meters: required,
            });
        }
    }

    let settlement_id = world.settlement_store_mut().allocate_settlement_id();
    let treasury_id = world.settlement_store_mut().allocate_treasury_id();
    let anchor_id = world.settlement_anchor_store_mut().allocate_anchor_id();
    let player_controlled = ownership.affiliation == crate::world::Affiliation::Player;
    let interaction_position = interaction_position.unwrap_or(center);

    let anchor = SettlementAnchorRecord {
        id: anchor_id,
        settlement_id,
        position: center,
        created_tick,
    };
    world.settlement_anchor_store_mut().insert(anchor)?;

    let settlement = SettlementRecord {
        id: settlement_id,
        display_name: display_name.into(),
        treasury_id,
        anchor_id,
        center,
        boundary_radius_meters,
        ownership,
        interaction_position,
        created_tick,
    };
    let treasury = SettlementTreasuryRecord {
        id: treasury_id,
        settlement_id,
        ownership,
        balance_gold: 0,
        created_tick,
        metadata: String::new(),
    };
    world
        .settlement_store_mut()
        .insert_settlement(settlement, treasury)?;

    world
        .settlement_state_store_mut()
        .ensure(settlement_id, kind, player_controlled);
    world.production_planner_store_mut().ensure(settlement_id);

    Ok(CreateSettlementReport {
        settlement_id,
        treasury_id,
        anchor_id,
    })
}

/// Legacy treasury-building helper — creates settlement at building position and links building.
pub fn create_settlement_with_treasury(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &BuildingInteractionProfileCatalog,
    building_id: BuildingId,
    display_name: impl Into<String>,
    ownership: SettlementOwnership,
    interaction_position: WorldPosition,
    created_tick: u64,
) -> Result<CreateSettlementReport, TreasuryError> {
    let Some(building) = world.get_building(building_id) else {
        return Err(TreasuryError::BuildingNotFound(building_id));
    };
    if !building_supports_settlement_treasury(
        building_catalog,
        interaction_catalog,
        building_id,
        world,
    ) {
        return Err(TreasuryError::BuildingNotSettlementCapable(building_id));
    }
    if world
        .settlement_store()
        .settlement_for_building(building_id)
        .is_some()
    {
        return Err(TreasuryError::SettlementAlreadyExists(building_id));
    }

    let center = building.placement.position;
    let report = create_settlement(
        world,
        center,
        display_name,
        ownership,
        SettlementKind::Town,
        None,
        Some(interaction_position),
        created_tick,
    )
    .map_err(|error| match error {
        SettlementCreationError::Treasury(err) => err,
        SettlementCreationError::OverlapsExisting { .. } => TreasuryError::OverlappingPlacement,
        SettlementCreationError::DuplicateAnchorId(_)
        | SettlementCreationError::DuplicateSettlementId(_) => {
            TreasuryError::DuplicateSettlementId(SettlementId::new(0))
        }
        SettlementCreationError::BuildingNotFound(id) => TreasuryError::BuildingNotFound(id),
        SettlementCreationError::BuildingNotSettlementCapable(id) => {
            TreasuryError::BuildingNotSettlementCapable(id)
        }
        SettlementCreationError::SettlementAlreadyLinked(id) => {
            TreasuryError::BuildingAlreadyLinked(id)
        }
    })?;

    super::membership::assign_building_settlement(world, building_id, Some(report.settlement_id))
        .map_err(|error| match error {
        super::membership::SettlementMembershipError::SettlementNotFound(id) => {
            TreasuryError::SettlementNotFound(id)
        }
        super::membership::SettlementMembershipError::BuildingNotFound(id) => {
            TreasuryError::BuildingNotFound(id)
        }
        super::membership::SettlementMembershipError::BuildingAlreadyAssigned { .. } => {
            TreasuryError::BuildingAlreadyLinked(building_id)
        }
        super::membership::SettlementMembershipError::UnitNotFound(_)
        | super::membership::SettlementMembershipError::UnitAlreadyAssigned { .. }
        | super::membership::SettlementMembershipError::NoSettlementAtPosition => {
            TreasuryError::BuildingAlreadyLinked(building_id)
        }
    })?;

    Ok(report)
}
