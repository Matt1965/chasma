use bevy::prelude::*;

use super::attack_cycle::AttackCycle;
use super::catalog::UnitDefinitionId;
use super::combat_state::CombatState;
use super::id::UnitId;
use super::metadata::UnitMetadata;
use super::placement::UnitPlacement;
use super::self_maintenance::{UnitNutritionState, UnitSelfMaintenanceState};
use super::source::UnitSource;
use super::state::UnitState;
use super::vitals::UnitVitals;
use crate::world::ownership::{Affiliation, OwnerId, TeamId, UnitOwnership};
use crate::world::relationship::{FactionId, SpeciesId};
use crate::world::settlement::SettlementId;

/// One authoritative unit instance (ADR-027 U2, ADR-051 O1).
///
/// [`UnitDefinitionId`] is the authoritative type reference. Instance records
/// do **not** copy catalog `faction_tag` as ownership; runtime owner/team/
/// affiliation live here.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct UnitRecord {
    pub id: UnitId,
    pub definition_id: UnitDefinitionId,
    pub placement: UnitPlacement,
    pub state: UnitState,
    pub source: UnitSource,
    pub metadata: UnitMetadata,
    /// Direct controller — not derived from catalog `faction_tag`.
    pub owner_id: Option<OwnerId>,
    /// Ally/enemy grouping for future combat/diplomacy.
    pub team_id: Option<TeamId>,
    /// Broad classification for UI and controllability.
    pub affiliation: Affiliation,
    /// Runtime-authoritative faction relationship identity (ADR-132 Phase 1).
    pub faction_id: FactionId,
    /// Runtime-authoritative species relationship identity (ADR-132 Phase 1).
    pub species_id: SpeciesId,
    pub vitals: UnitVitals,
    /// Individual nutrition fullness (ADR-134). Not normalized from catalog on restore.
    pub nutrition: UnitNutritionState,
    /// Autonomous self-maintenance activity (ADR-134).
    pub self_maintenance: UnitSelfMaintenanceState,
    /// Authoritative navigable space (ADR-083 B6).
    pub current_space_id: crate::world::SpaceId,
    pub combat_state: CombatState,
    /// Weapon attack cycle timing when in-range (ADR-058 C5).
    pub attack_cycle: Option<AttackCycle>,
    /// Attacker-specific reactive self-defense authorization (ADR-062, COMBAT-VERTICAL-1D).
    pub reactive_combat_target: Option<UnitId>,
    /// Centralized inventory reference when unit definition has a profile (ADR-089 I3).
    pub inventory_id: Option<crate::world::InventoryId>,
    /// Explicit settlement membership (ADR-133 Phase 2). None = not a member.
    pub settlement_id: Option<SettlementId>,
}

impl UnitRecord {
    pub fn new(
        id: UnitId,
        definition_id: UnitDefinitionId,
        placement: UnitPlacement,
        source: UnitSource,
        ownership: UnitOwnership,
        max_hp: u32,
        faction_id: FactionId,
        species_id: SpeciesId,
    ) -> Self {
        Self {
            id,
            definition_id,
            placement,
            state: UnitState::default(),
            source,
            metadata: UnitMetadata,
            owner_id: ownership.owner_id,
            team_id: ownership.team_id,
            affiliation: ownership.affiliation,
            faction_id,
            species_id,
            vitals: UnitVitals::full(max_hp),
            nutrition: UnitNutritionState::default(),
            self_maintenance: UnitSelfMaintenanceState::default(),
            current_space_id: crate::world::SpaceId::SURFACE,
            combat_state: CombatState::default(),
            attack_cycle: None,
            reactive_combat_target: None,
            inventory_id: None,
            settlement_id: None,
        }
    }

    /// Test helper using neutral wild/wolf identity defaults.
    #[cfg(test)]
    pub fn new_test(
        id: UnitId,
        definition_id: UnitDefinitionId,
        placement: UnitPlacement,
        source: UnitSource,
        ownership: UnitOwnership,
        max_hp: u32,
    ) -> Self {
        Self::new(
            id,
            definition_id,
            placement,
            source,
            ownership,
            max_hp,
            FactionId::new("wild"),
            SpeciesId::new("wolf"),
        )
    }

    pub fn ownership(&self) -> UnitOwnership {
        UnitOwnership {
            owner_id: self.owner_id,
            team_id: self.team_id,
            affiliation: self.affiliation,
        }
    }
}
