//! Settlement and treasury storage on WorldData (ADR-093 I7).

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;

use super::id::{SettlementId, TreasuryId};
use super::record::{SettlementRecord, SettlementTreasuryRecord, TreasuryTransactionRecord};
use crate::world::BuildingId;

#[derive(Debug, Clone, Default, PartialEq, Reflect)]
pub struct SettlementStore {
    next_settlement_id: u64,
    next_treasury_id: u64,
    settlements: BTreeMap<SettlementId, SettlementRecord>,
    treasuries: BTreeMap<TreasuryId, SettlementTreasuryRecord>,
    settlement_by_building: BTreeMap<BuildingId, SettlementId>,
    settlement_buildings: BTreeMap<SettlementId, BTreeSet<BuildingId>>,
    treasury_by_settlement: BTreeMap<SettlementId, TreasuryId>,
    transaction_log: Vec<TreasuryTransactionRecord>,
}

impl SettlementStore {
    pub fn allocate_settlement_id(&mut self) -> SettlementId {
        let id = SettlementId::new(self.next_settlement_id);
        self.next_settlement_id = self.next_settlement_id.saturating_add(1);
        id
    }

    pub fn allocate_treasury_id(&mut self) -> TreasuryId {
        let id = TreasuryId::new(self.next_treasury_id);
        self.next_treasury_id = self.next_treasury_id.saturating_add(1);
        id
    }

    pub fn next_settlement_id(&self) -> u64 {
        self.next_settlement_id
    }

    pub fn next_treasury_id(&self) -> u64 {
        self.next_treasury_id
    }

    pub fn restore_next_ids(&mut self, next_settlement: u64, next_treasury: u64) {
        self.next_settlement_id = self.next_settlement_id.max(next_settlement);
        self.next_treasury_id = self.next_treasury_id.max(next_treasury);
    }

    pub fn get_settlement(&self, id: SettlementId) -> Option<&SettlementRecord> {
        self.settlements.get(&id)
    }

    pub fn get_settlement_mut(&mut self, id: SettlementId) -> Option<&mut SettlementRecord> {
        self.settlements.get_mut(&id)
    }

    pub fn get_treasury(&self, id: TreasuryId) -> Option<&SettlementTreasuryRecord> {
        self.treasuries.get(&id)
    }

    pub fn get_treasury_mut(&mut self, id: TreasuryId) -> Option<&mut SettlementTreasuryRecord> {
        self.treasuries.get_mut(&id)
    }

    pub fn settlement_for_building(&self, building_id: BuildingId) -> Option<SettlementId> {
        self.settlement_by_building.get(&building_id).copied()
    }

    pub fn buildings_for_settlement(&self, settlement_id: SettlementId) -> Vec<BuildingId> {
        self.settlement_buildings
            .get(&settlement_id)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default()
    }

    pub fn link_building_to_settlement(
        &mut self,
        settlement_id: SettlementId,
        building_id: BuildingId,
    ) -> Result<(), super::error::TreasuryError> {
        if !self.settlements.contains_key(&settlement_id) {
            return Err(super::error::TreasuryError::SettlementNotFound(settlement_id));
        }
        if let Some(existing) = self.settlement_by_building.get(&building_id) {
            if *existing != settlement_id {
                return Err(super::error::TreasuryError::BuildingAlreadyLinked(
                    building_id,
                ));
            }
            return Ok(());
        }
        self.settlement_by_building
            .insert(building_id, settlement_id);
        self.settlement_buildings
            .entry(settlement_id)
            .or_default()
            .insert(building_id);
        Ok(())
    }

    pub fn unlink_building(&mut self, building_id: BuildingId) {
        if let Some(settlement_id) = self.settlement_by_building.remove(&building_id) {
            if let Some(set) = self.settlement_buildings.get_mut(&settlement_id) {
                set.remove(&building_id);
            }
        }
    }

    pub fn treasury_for_settlement(&self, settlement_id: SettlementId) -> Option<TreasuryId> {
        self.treasury_by_settlement.get(&settlement_id).copied()
    }

    pub fn sorted_settlement_ids(&self) -> Vec<SettlementId> {
        self.settlements.keys().copied().collect()
    }

    pub fn sorted_treasury_ids(&self) -> Vec<TreasuryId> {
        self.treasuries.keys().copied().collect()
    }

    pub fn transaction_log(&self) -> &[TreasuryTransactionRecord] {
        &self.transaction_log
    }

    pub fn push_transaction(&mut self, record: TreasuryTransactionRecord) {
        self.transaction_log.push(record);
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Replace all settlement/treasury state (scene restore).
    pub fn restore_snapshot(
        &mut self,
        settlements: Vec<SettlementRecord>,
        treasuries: Vec<SettlementTreasuryRecord>,
        next_settlement: u64,
        next_treasury: u64,
    ) -> Result<(), super::error::TreasuryError> {
        self.clear();
        self.restore_next_ids(next_settlement, next_treasury);
        let mut treasury_by_settlement: std::collections::BTreeMap<
            SettlementId,
            SettlementTreasuryRecord,
        > = treasuries
            .into_iter()
            .map(|treasury| (treasury.settlement_id, treasury))
            .collect();
        for settlement in settlements {
            let Some(treasury) = treasury_by_settlement.remove(&settlement.id) else {
                return Err(super::error::TreasuryError::SettlementNotFound(
                    settlement.id,
                ));
            };
            self.insert_settlement(settlement, treasury)?;
        }
        Ok(())
    }

    pub fn insert_settlement(
        &mut self,
        settlement: SettlementRecord,
        treasury: SettlementTreasuryRecord,
    ) -> Result<(), super::error::TreasuryError> {
        if self.settlements.contains_key(&settlement.id) {
            return Err(super::error::TreasuryError::DuplicateSettlementId(
                settlement.id,
            ));
        }
        if self.treasuries.contains_key(&treasury.id) {
            return Err(super::error::TreasuryError::DuplicateTreasuryId(
                treasury.id,
            ));
        }
        if self
            .settlement_by_building
            .contains_key(&settlement.anchor_building_id)
        {
            return Err(super::error::TreasuryError::SettlementAlreadyExists(
                settlement.anchor_building_id,
            ));
        }
        if treasury.settlement_id != settlement.id {
            return Err(super::error::TreasuryError::SettlementNotFound(
                treasury.settlement_id,
            ));
        }
        self.settlement_by_building
            .insert(settlement.anchor_building_id, settlement.id);
        self.settlement_buildings
            .entry(settlement.id)
            .or_default()
            .insert(settlement.anchor_building_id);
        self.treasury_by_settlement
            .insert(settlement.id, treasury.id);
        self.treasuries.insert(treasury.id, treasury);
        self.settlements.insert(settlement.id, settlement);
        Ok(())
    }

    pub fn remove_settlement(&mut self, id: SettlementId) -> Option<SettlementRecord> {
        let settlement = self.settlements.remove(&id)?;
        let treasury_id = self.treasury_by_settlement.remove(&id);
        if let Some(members) = self.settlement_buildings.remove(&id) {
            for building_id in members {
                self.settlement_by_building.remove(&building_id);
            }
        }
        if let Some(treasury_id) = treasury_id {
            self.treasuries.remove(&treasury_id);
        }
        Some(settlement)
    }
}
