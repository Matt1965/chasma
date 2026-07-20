//! Need evaluation — read-only, independent per need, no actions (SA2).

use crate::world::building::catalog::BuildingCatalog;
use crate::world::inventory::InventoryCatalogCtx;
use crate::world::item::{ItemCatalog, ItemCategoryId};
use crate::world::settlement::emergency::EmergencyCatalog;
use crate::world::settlement::state::{NeedCategory, SettlementState};
use crate::world::settlement::SettlementId;
use crate::world::{BuildingLifecycleState, WorldData};

use super::catalog::NeedCatalog;
use super::definition::{NeedDefinition, NeedEvaluationMethod, NeedTargetSource};
use super::pressure::{apply_pressure_modifiers, normalize_pressure};
use super::snapshot::{NeedSnapshot, SettlementNeedEvaluation};

/// Read-only evaluation context. Evaluators must not mutate world state.
pub struct NeedEvalContext<'a> {
    pub world: &'a WorldData,
    pub building_catalog: &'a BuildingCatalog,
    pub item_catalog: &'a ItemCatalog,
    pub inventory_ctx: &'a InventoryCatalogCtx<'a>,
    pub settlement_id: SettlementId,
    pub state: &'a SettlementState,
    pub emergency_catalog: &'a EmergencyCatalog,
    pub simulation_tick: u64,
}

/// Evaluate all enabled needs for one settlement. Pure read of world + SettlementState.
pub fn evaluate_settlement_needs(
    ctx: &NeedEvalContext<'_>,
    need_catalog: &NeedCatalog,
) -> SettlementNeedEvaluation {
    let mut evaluation = SettlementNeedEvaluation {
        settlement_id: ctx.settlement_id,
        evaluated_tick: ctx.simulation_tick,
        snapshots: Vec::new(),
        diagnostics: Vec::new(),
    };

    for definition in need_catalog.enabled_definitions() {
        let snapshot = evaluate_one_need(ctx, definition);
        if let Some(reason) = &snapshot.blocking_reason {
            evaluation.diagnostics.push(format!(
                "{}: {}",
                snapshot.need_id.as_str(),
                reason.label()
            ));
        }
        evaluation.snapshots.push(snapshot);
    }

    evaluation
}

fn evaluate_one_need(ctx: &NeedEvalContext<'_>, definition: &NeedDefinition) -> NeedSnapshot {
    let desired = resolve_desired(ctx.state, definition);
    let (current, source) = match definition.evaluation_method {
        NeedEvaluationMethod::FoodStock => measure_food_stock(ctx),
        NeedEvaluationMethod::ConstructionSites => measure_construction_sites(ctx, desired),
        NeedEvaluationMethod::HousingCapacity => measure_housing_capacity(ctx),
        NeedEvaluationMethod::DefensePosture => measure_defense_posture(ctx),
        NeedEvaluationMethod::ResearchStub => (0.0, "research_stub current=0".into()),
        NeedEvaluationMethod::ExpansionGrowth => measure_expansion_growth(ctx),
        NeedEvaluationMethod::LuxuryStock => measure_luxury_stock(ctx),
    };

    // Construction uses a satisfaction current derived from backlog; desired for display stays authored.
    let pressure_desired = if definition.evaluation_method == NeedEvaluationMethod::ConstructionSites
    {
        // Ensure backlog can produce pressure even when authored target is 0.
        desired.max(1.0)
    } else {
        desired
    };

    let base = normalize_pressure(current, pressure_desired);
    let mut pressure = apply_pressure_modifiers(
        base,
        definition.id.as_str(),
        &ctx.state.modifiers,
        ctx.simulation_tick,
    );
    // SA8: authored emergency pressure modifiers (severity-scaled). Single layer — not reapplied at SA4.
    let emergency_delta = crate::world::settlement::emergency::emergency_need_pressure_delta(
        ctx.state,
        ctx.emergency_catalog,
        definition.id.as_str(),
    );
    pressure = (f32::from(pressure) + emergency_delta)
        .clamp(0.0, 100.0)
        .round() as u8;

    NeedSnapshot::with_values(
        definition.id.clone(),
        current,
        desired,
        pressure,
        ctx.simulation_tick,
        source,
    )
}

pub fn resolve_desired(state: &SettlementState, definition: &NeedDefinition) -> f32 {
    match definition.target_source {
        NeedTargetSource::SettlementNeedTarget => state
            .need_targets
            .iter()
            .find(|t| t.category == definition.target_category)
            .map(|t| t.target_value as f32)
            .unwrap_or(definition.default_desired as f32),
        NeedTargetSource::DefinitionDefault => definition.default_desired as f32,
    }
}

fn measure_food_stock(ctx: &NeedEvalContext<'_>) -> (f32, String) {
    let stock = crate::world::settlement::aggregate_settlement_stock(
        ctx.world,
        ctx.building_catalog,
        ctx.settlement_id,
        &[],
        ctx.inventory_ctx,
    );
    let food_category = ItemCategoryId::new("food");
    let mut total = 0u32;
    for (item_id, qty) in &stock {
        if let Some(def) = ctx.item_catalog.get(item_id) {
            if def.category_id == food_category {
                total = total.saturating_add(*qty);
            }
        }
    }
    (
        total as f32,
        format!("inventory_stock category=food total={total}"),
    )
}

fn measure_luxury_stock(ctx: &NeedEvalContext<'_>) -> (f32, String) {
    let stock = crate::world::settlement::aggregate_settlement_stock(
        ctx.world,
        ctx.building_catalog,
        ctx.settlement_id,
        &[],
        ctx.inventory_ctx,
    );
    let mut total = 0u32;
    for (item_id, qty) in &stock {
        let id = item_id.as_str();
        if id.contains("luxury") || id == "iron_bar" {
            total = total.saturating_add(*qty);
        }
    }
    (
        total as f32,
        format!("inventory_stock luxury_proxy total={total}"),
    )
}

/// Construction backlog: `current` is remaining capacity vs a pressure floor of 1.
/// Incomplete sites reduce current; cleared backlog restores current to the floor.
fn measure_construction_sites(ctx: &NeedEvalContext<'_>, authored_desired: f32) -> (f32, String) {
    let building_ids = ctx
        .world
        .settlement_store()
        .buildings_for_settlement(ctx.settlement_id);
    let mut incomplete = 0u32;
    for building_id in building_ids {
        let Some(record) = ctx.world.get_building(building_id) else {
            continue;
        };
        if matches!(
            record.lifecycle_state,
            BuildingLifecycleState::Planned
                | BuildingLifecycleState::Foundation
                | BuildingLifecycleState::InProgress
        ) {
            incomplete += 1;
        }
    }
    let floor = authored_desired.max(1.0);
    let current = (floor - incomplete as f32).max(0.0);
    (
        current,
        format!("construction_sites incomplete={incomplete}"),
    )
}

fn measure_housing_capacity(ctx: &NeedEvalContext<'_>) -> (f32, String) {
    let building_ids = ctx
        .world
        .settlement_store()
        .buildings_for_settlement(ctx.settlement_id);
    let mut housing = 0u32;
    for building_id in building_ids {
        let Some(record) = ctx.world.get_building(building_id) else {
            continue;
        };
        if record.lifecycle_state != BuildingLifecycleState::Complete {
            continue;
        }
        let Some(definition) = ctx.building_catalog.get(&record.definition_id) else {
            continue;
        };
        let id = definition.id.as_str();
        if id.contains("hut")
            || id.contains("house")
            || id.contains("dwelling")
            || id == "settlement_core"
        {
            housing += 1;
        }
    }
    (housing as f32, format!("housing_buildings complete={housing}"))
}

fn measure_defense_posture(ctx: &NeedEvalContext<'_>) -> (f32, String) {
    let aggression = ctx.state.policies.aggression as f32;
    let building_ids = ctx
        .world
        .settlement_store()
        .buildings_for_settlement(ctx.settlement_id);
    let mut defense_buildings = 0u32;
    for building_id in building_ids {
        let Some(record) = ctx.world.get_building(building_id) else {
            continue;
        };
        if record.lifecycle_state != BuildingLifecycleState::Complete {
            continue;
        }
        let Some(definition) = ctx.building_catalog.get(&record.definition_id) else {
            continue;
        };
        let id = definition.id.as_str();
        if id.contains("wall")
            || id.contains("tower")
            || id.contains("gate")
            || id.contains("barracks")
        {
            defense_buildings += 1;
        }
    }
    let current = aggression / 25.0 + defense_buildings as f32;
    (
        current,
        format!(
            "defense_posture aggression={} buildings={}",
            ctx.state.policies.aggression, defense_buildings
        ),
    )
}

fn measure_expansion_growth(ctx: &NeedEvalContext<'_>) -> (f32, String) {
    let count = ctx
        .world
        .settlement_store()
        .buildings_for_settlement(ctx.settlement_id)
        .len() as f32;
    (count, format!("expansion_growth buildings={count}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::settlement::state::{NeedTarget, SettlementKind, SettlementState};
    use crate::world::settlement::SettlementId;
    use crate::world::settlement::ProductionPriorityCategory;
    use super::super::definition::{
        NeedMeasurementType, NeedResponseCategory,
    };

    #[test]
    fn food_desired_and_pressure_deterministic() {
        let id = SettlementId::new(1);
        let mut state = SettlementState::new(id, SettlementKind::Town, false);
        state.need_targets = vec![NeedTarget::new(NeedCategory::Food, 100, 1.0)];
        let def = NeedDefinition::new(
            "food",
            "Food",
            "",
            NeedMeasurementType::InventoryStock,
            NeedTargetSource::SettlementNeedTarget,
            NeedCategory::Food,
            NeedEvaluationMethod::FoodStock,
            ProductionPriorityCategory::Food,
            NeedResponseCategory::Production,
            100,
        );
        assert_eq!(resolve_desired(&state, &def), 100.0);
        assert_eq!(normalize_pressure(0.0, 100.0), 100);
        assert_eq!(normalize_pressure(100.0, 100.0), 0);
    }
}
