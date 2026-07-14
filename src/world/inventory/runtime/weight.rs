//! Soft-weight queries for authoritative inventories (ADR-088 I2).

use super::catalog_ctx::InventoryCatalogCtx;
use super::error::InventoryError;
use super::record::InventoryRecord;

/// Weight query result — overweight inventories remain valid (ADR-088 I2).
#[derive(Debug, Clone, PartialEq)]
pub struct InventoryWeightQuery {
    pub total_mass_grams: u64,
    pub reference_weight_grams: Option<u32>,
    pub comfortable_weight_grams: Option<u32>,
    /// Mass above reference when reference is set; zero otherwise.
    pub over_reference_grams: u64,
    /// `total / reference` when reference is set and non-zero.
    pub over_reference_ratio: Option<f64>,
}

pub fn query_inventory_weight(
    record: &InventoryRecord,
    ctx: &InventoryCatalogCtx<'_>,
) -> Result<InventoryWeightQuery, InventoryError> {
    let profile = ctx.require_profile(record.profile_id())?;
    let total = record.total_mass_grams();
    let reference = profile.reference_weight_grams;
    let comfortable = profile.comfortable_weight_grams;
    let over_reference_grams = reference
        .map(|reference| total.saturating_sub(u64::from(reference)))
        .unwrap_or(0);
    let over_reference_ratio = reference.and_then(|reference| {
        if reference == 0 {
            None
        } else {
            Some(total as f64 / f64::from(reference))
        }
    });
    Ok(InventoryWeightQuery {
        total_mass_grams: total,
        reference_weight_grams: reference,
        comfortable_weight_grams: comfortable,
        over_reference_grams,
        over_reference_ratio,
    })
}
