//! Settlement building membership helpers (EP9).

use crate::world::{BuildingId, WorldData};

use super::id::SettlementId;
use super::store::SettlementStore;

/// Link all buildings sharing the settlement's affiliation to that settlement (EP9 dev/scene seam).
pub fn reconcile_settlement_building_membership(world: &mut WorldData) {
    let settlement_ids = world.settlement_store().sorted_settlement_ids();
    for settlement_id in settlement_ids {
        let Some(settlement) = world.settlement_store().get_settlement(settlement_id).cloned()
        else {
            continue;
        };
        for building_id in world.sorted_building_ids() {
            let Some(record) = world.get_building(building_id) else {
                continue;
            };
            if record.ownership.affiliation != settlement.ownership.affiliation {
                continue;
            }
            let _ = world
                .settlement_store_mut()
                .link_building_to_settlement(settlement_id, building_id);
        }
    }
}

impl SettlementStore {
    pub fn link_buildings(
        &mut self,
        settlement_id: SettlementId,
        building_ids: impl IntoIterator<Item = BuildingId>,
    ) -> Result<(), super::error::TreasuryError> {
        for building_id in building_ids {
            self.link_building_to_settlement(settlement_id, building_id)?;
        }
        Ok(())
    }
}
