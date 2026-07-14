//! Explicit unit removal inventory policies (ADR-089 I3).

use super::death::RemovalReason;
use super::inventory::cleanup_unit_inventory_on_delete;
use super::record::UnitRecord;
use crate::world::corpse::{CorpseSettings, create_corpse_from_unit, transfer_inventory_to_corpse};
use crate::world::inventory::InventoryCatalogCtx;
use crate::world::unit::UnitCatalog;
use crate::world::{UnitId, WorldData};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitRemovalOutcome {
    pub unit_id: UnitId,
    pub reason: RemovalReason,
    pub corpse_id: Option<crate::world::CorpseId>,
}

/// Finalize authoritative unit removal with inventory/corpse policy.
pub fn finalize_unit_removal(
    world: &mut WorldData,
    unit_id: UnitId,
    reason: RemovalReason,
    catalog: &UnitCatalog,
    ctx: &InventoryCatalogCtx<'_>,
    corpse_settings: &CorpseSettings,
    tick: u64,
) -> Result<UnitRemovalOutcome, super::death::UnitRemovalError> {
    let Some(unit) = world.get_unit(unit_id).cloned() else {
        return Ok(UnitRemovalOutcome {
            unit_id,
            reason,
            corpse_id: None,
        });
    };

    let outcome = match reason {
        RemovalReason::Killed => {
            let definition = catalog.get(&unit.definition_id).ok_or(
                super::death::UnitRemovalError::MissingDefinition {
                    unit_id,
                    definition_id: unit.definition_id.clone(),
                },
            )?;
            let corpse = create_corpse_from_unit(world, &unit, definition, corpse_settings, tick)
                .map_err(
                |error| super::death::UnitRemovalError::CorpseCreationFailed { unit_id, error },
            )?;
            if let Some(inventory_id) = unit.inventory_id {
                let (inventory_store, instance_store) = world.inventory_runtime_mut();
                transfer_inventory_to_corpse(
                    inventory_store,
                    instance_store,
                    inventory_id,
                    unit_id,
                    corpse.id,
                )
                .map_err(|error| {
                    super::death::UnitRemovalError::InventoryTransferFailed {
                        unit_id,
                        inventory_id,
                        error,
                    }
                })?;
            }
            UnitRemovalOutcome {
                unit_id,
                reason,
                corpse_id: Some(corpse.id),
            }
        }
        RemovalReason::DevDeleted | RemovalReason::Cleanup | RemovalReason::Unknown => {
            let _ = cleanup_unit_inventory_on_delete(world, ctx, &unit);
            UnitRemovalOutcome {
                unit_id,
                reason,
                corpse_id: None,
            }
        }
    };

    let _ = world.remove_unit_by_id(unit_id);
    Ok(outcome)
}
