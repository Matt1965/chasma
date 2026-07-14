//! Corpse fixed-tick lifecycle (ADR-089 I3).

use super::authoring::remove_corpse_with_inventory;
use super::id::CorpseId;
use super::record::CorpseState;
use crate::world::WorldData;
use crate::world::inventory::InventoryCatalogCtx;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CorpseLifecycleReport {
    pub expired_corpse_ids: Vec<CorpseId>,
}

/// Advance authoritative corpse lifetimes by one simulation tick.
pub fn step_corpse_lifecycle(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
) -> CorpseLifecycleReport {
    let mut report = CorpseLifecycleReport::default();
    let corpse_ids = world.corpse_store().sorted_corpse_ids();
    for corpse_id in corpse_ids {
        let should_expire = {
            let Some(record) = world.corpse_store_mut().get_mut(corpse_id) else {
                continue;
            };
            if record.state != CorpseState::Present {
                continue;
            }
            if record.remaining_lifetime_ticks == 0 {
                true
            } else {
                record.remaining_lifetime_ticks -= 1;
                record.remaining_lifetime_ticks == 0
            }
        };
        if should_expire {
            if let Some(record) = world.corpse_store_mut().get_mut(corpse_id) {
                record.state = CorpseState::Expired;
            }
            if remove_corpse_with_inventory(world, ctx, corpse_id).is_ok() {
                report.expired_corpse_ids.push(corpse_id);
            }
        }
    }
    report
}

#[cfg(feature = "dev")]
pub fn dev_expire_corpse(
    world: &mut WorldData,
    ctx: &InventoryCatalogCtx<'_>,
    corpse_id: CorpseId,
) -> Result<(), super::error::CorpseError> {
    if world.corpse_store().get(corpse_id).is_none() {
        return Err(super::error::CorpseError::CorpseNotFound(corpse_id));
    }
    if let Some(record) = world.corpse_store_mut().get_mut(corpse_id) {
        record.remaining_lifetime_ticks = 0;
        record.state = CorpseState::Present;
    }
    remove_corpse_with_inventory(world, ctx, corpse_id)?;
    Ok(())
}
