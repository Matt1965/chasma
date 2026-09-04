//! Settlement-scoped workforce permission storage (opt-out deny list).

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;

use super::domain::WorkPermissionDomain;
use crate::world::{SettlementId, UnitId};

#[derive(Debug, Clone, Default, PartialEq, Eq, Reflect)]
pub struct WorkforcePermissionStore {
    /// Domains explicitly disallowed for `(settlement, unit)`. Absent key = all allowed.
    denied: BTreeMap<(SettlementId, UnitId), BTreeSet<WorkPermissionDomain>>,
}

impl WorkforcePermissionStore {
    pub fn is_allowed(
        &self,
        settlement_id: SettlementId,
        unit_id: UnitId,
        domain: WorkPermissionDomain,
    ) -> bool {
        self.denied
            .get(&(settlement_id, unit_id))
            .is_none_or(|set| !set.contains(&domain))
    }

    pub fn set_allowed(
        &mut self,
        settlement_id: SettlementId,
        unit_id: UnitId,
        domain: WorkPermissionDomain,
        allowed: bool,
    ) {
        let key = (settlement_id, unit_id);
        if allowed {
            if let Some(set) = self.denied.get_mut(&key) {
                set.remove(&domain);
                if set.is_empty() {
                    self.denied.remove(&key);
                }
            }
            return;
        }
        self.denied.entry(key).or_default().insert(domain);
    }

    pub fn clear_unit(&mut self, unit_id: UnitId) {
        self.denied
            .retain(|(_, stored_unit), _| *stored_unit != unit_id);
    }

    pub fn clear_settlement(&mut self, settlement_id: SettlementId) {
        self.denied
            .retain(|(stored_settlement, _), _| *stored_settlement != settlement_id);
    }
}
