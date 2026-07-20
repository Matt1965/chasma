//! Settlement treasury creation (ADR-093 I7).

use super::access::building_supports_settlement_treasury;
use super::error::TreasuryError;
use super::id::{SettlementId, TreasuryId};
use super::record::{SettlementOwnership, SettlementRecord, SettlementTreasuryRecord};
use super::store::SettlementStore;
use crate::world::building::{BuildingCatalog, BuildingInteractionProfileCatalog};
use crate::world::{BuildingId, WorldData, WorldPosition};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateSettlementReport {
    pub settlement_id: SettlementId,
    pub treasury_id: TreasuryId,
}

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
    if world.get_building(building_id).is_none() {
        return Err(TreasuryError::BuildingNotFound(building_id));
    }
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

    let settlement_id = world.settlement_store_mut().allocate_settlement_id();
    let treasury_id = world.settlement_store_mut().allocate_treasury_id();
    let player_controlled = ownership.affiliation == crate::world::Affiliation::Player;
    let settlement = SettlementRecord {
        id: settlement_id,
        display_name: display_name.into(),
        treasury_id,
        anchor_building_id: building_id,
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

    // SA1: every settlement owns a SettlementState (identical structure for player and AI).
    world.settlement_state_store_mut().ensure(
        settlement_id,
        super::state::SettlementKind::Town,
        player_controlled,
    );
    // EP9: ensure production planner entry so interval replan can run.
    world.production_planner_store_mut().ensure(settlement_id);

    Ok(CreateSettlementReport {
        settlement_id,
        treasury_id,
    })
}
