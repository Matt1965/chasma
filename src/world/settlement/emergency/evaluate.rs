//! Emergency detection + hysteresis (SA8). Mutates SettlementEmergencyState only.

use crate::world::building::catalog::BuildingCatalog;
use crate::world::inventory::InventoryCatalogCtx;
use crate::world::item::{ItemCatalog, ItemCategoryId};
use crate::world::settlement::state::{
    ActiveEmergencyInstance, NeedCategory, SettlementState,
};
use crate::world::settlement::SettlementId;
use crate::world::WorldData;

use super::catalog::EmergencyCatalog;
use super::definition::{EmergencyDefinition, EmergencyEvaluatorKind};
use super::report::{EmergencyEvaluationReport, EmergencySignalDiagnostic};

pub struct EmergencyEvalContext<'a> {
    pub world: &'a WorldData,
    pub building_catalog: &'a BuildingCatalog,
    pub item_catalog: &'a ItemCatalog,
    pub inventory_ctx: &'a InventoryCatalogCtx<'a>,
    pub settlement_id: SettlementId,
    pub simulation_tick: u64,
}

/// Evaluate signals and update persistent emergency instances with hysteresis.
///
/// Returns a transient report. Does not command workers or mutate inventories.
pub fn evaluate_settlement_emergencies(
    ctx: &EmergencyEvalContext<'_>,
    catalog: &EmergencyCatalog,
    state: &mut SettlementState,
) -> EmergencyEvaluationReport {
    state.emergencies.migrate_legacy_flags(ctx.simulation_tick);

    let mut report = EmergencyEvaluationReport {
        settlement_id: ctx.settlement_id,
        evaluated_tick: ctx.simulation_tick,
        signals: Vec::new(),
        activated: Vec::new(),
        deactivated: Vec::new(),
        diagnostics: Vec::new(),
    };

    // Preserve manual suppress / force entries that are not in catalog as diagnostics.
    let mut next_instances: Vec<ActiveEmergencyInstance> = Vec::new();
    let existing: Vec<ActiveEmergencyInstance> = state.emergencies.instances.clone();

    let mut defs: Vec<&EmergencyDefinition> = catalog.enabled_definitions().collect();
    defs.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));

    for def in defs {
        let signal = compute_signal(ctx, state, def.evaluator);
        report.signals.push(EmergencySignalDiagnostic {
            emergency_id: def.id.as_str().to_string(),
            signal,
            activation_threshold: def.activation_threshold,
            deactivation_threshold: def.deactivation_threshold,
            evaluator: format!("{:?}", def.evaluator),
        });

        let mut prior = existing
            .iter()
            .find(|i| i.emergency_id == def.id.as_str())
            .cloned();

        // Manual suppress blocks automatic activation; clear automatic instance.
        if prior
            .as_ref()
            .is_some_and(|p| p.manual_suppress && !p.manual_force)
        {
            report.diagnostics.push(format!(
                "`{}` suppressed (manual)",
                def.id.as_str()
            ));
            continue;
        }

        let auto_allowed = state.policies.auto_emergency_response;
        if prior.as_ref().is_some_and(|p| p.manual_force) {
            let mut inst = prior.take().unwrap();
            inst.severity = signal.max(inst.severity).clamp(0.0, 1.0);
            inst.last_signal = signal;
            next_instances.push(inst);
            continue;
        }

        if !auto_allowed {
            // Keep existing auto instances for continuity until recovery rules clear them.
            if let Some(p) = prior.take() {
                if should_deactivate(ctx.simulation_tick, def, signal, &p) {
                    report.deactivated.push(def.id.as_str().to_string());
                    report.diagnostics.push(format!(
                        "`{}` recovered (auto response disabled)",
                        def.id.as_str()
                    ));
                } else {
                    let mut inst = p;
                    inst.severity = severity_from_signal(signal, def);
                    inst.last_signal = signal;
                    next_instances.push(inst);
                }
            }
            continue;
        }

        match prior {
            None => {
                if signal >= def.activation_threshold {
                    let severity = severity_from_signal(signal, def);
                    let mut inst =
                        ActiveEmergencyInstance::new(def.id.as_str(), ctx.simulation_tick, severity);
                    inst.last_signal = signal;
                    inst.source = format!("{:?}", def.evaluator);
                    next_instances.push(inst);
                    report.activated.push(def.id.as_str().to_string());
                }
            }
            Some(mut inst) => {
                if should_deactivate(ctx.simulation_tick, def, signal, &inst) {
                    report.deactivated.push(def.id.as_str().to_string());
                    report.diagnostics.push(format!(
                        "`{}` deactivated signal={signal:.2} <= {}",
                        def.id.as_str(),
                        def.deactivation_threshold
                    ));
                } else {
                    inst.severity = severity_from_signal(signal, def);
                    inst.last_signal = signal;
                    next_instances.push(inst);
                }
            }
        }
    }

    // Carry forward unknown / catalog-removed manual instances.
    for prior in existing {
        if catalog.get_str(&prior.emergency_id).is_some() {
            continue;
        }
        if prior.manual_force || prior.manual_suppress {
            report.diagnostics.push(format!(
                "retained unknown emergency `{}` (manual)",
                prior.emergency_id
            ));
            next_instances.push(prior);
        }
    }

    next_instances.sort_by(|a, b| a.emergency_id.cmp(&b.emergency_id));
    state.emergencies.instances = next_instances;
    state.emergencies.sync_legacy_flags();

    report.diagnostics.push(format!(
        "active={} activated={} deactivated={}",
        state.emergencies.instances.len(),
        report.activated.len(),
        report.deactivated.len()
    ));
    report
}

fn should_deactivate(
    tick: u64,
    def: &EmergencyDefinition,
    signal: f32,
    inst: &ActiveEmergencyInstance,
) -> bool {
    if inst.manual_force {
        return false;
    }
    let active_for = tick.saturating_sub(inst.activated_tick);
    if active_for < def.min_active_duration_ticks {
        return false;
    }
    if signal > def.deactivation_threshold {
        return false;
    }
    // recovery_delay: must stay below deactivation for recovery_delay ticks.
    // Approximated: if last_signal was also <= deactivation and active_for covers delay.
    if def.recovery_delay_ticks > 0 {
        let recovered_for = if inst.last_signal <= def.deactivation_threshold {
            active_for
        } else {
            0
        };
        if recovered_for < def.min_active_duration_ticks.saturating_add(def.recovery_delay_ticks) {
            // Require min_active + recovery_delay wall time while signal is recovering.
            if active_for < def.min_active_duration_ticks.saturating_add(def.recovery_delay_ticks) {
                return false;
            }
        }
    }
    true
}

fn severity_from_signal(signal: f32, _def: &EmergencyDefinition) -> f32 {
    // Continuous severity tracks the detection signal once active.
    signal.clamp(0.0, 1.0)
}

fn compute_signal(
    ctx: &EmergencyEvalContext<'_>,
    state: &SettlementState,
    kind: EmergencyEvaluatorKind,
) -> f32 {
    match kind {
        EmergencyEvaluatorKind::FoodReserveRatio => food_reserve_signal(ctx, state),
        EmergencyEvaluatorKind::HostilePresenceSignal => seam_signal(state, "hostile_threat"),
        EmergencyEvaluatorKind::FireSignal => seam_signal(state, "fire_severity"),
        EmergencyEvaluatorKind::EvacuationSignal => {
            let evacuate = seam_signal(state, "evacuate_signal");
            let hostile = seam_signal(state, "hostile_threat");
            let fire = seam_signal(state, "fire_severity");
            evacuate.max(hostile.max(fire) * 0.85)
        }
    }
}

fn seam_signal(state: &SettlementState, key: &str) -> f32 {
    state
        .extension_seams
        .get(key)
        .and_then(|v| v.parse::<f32>().ok())
        .map(|v| v.clamp(0.0, 1.0))
        .unwrap_or(0.0)
}

fn food_reserve_signal(ctx: &EmergencyEvalContext<'_>, state: &SettlementState) -> f32 {
    let desired = state
        .need_targets
        .iter()
        .find(|t| t.category == NeedCategory::Food)
        .map(|t| t.target_value as f32)
        .unwrap_or(100.0)
        .max(1.0);
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
    let ratio = (total as f32) / desired;
    (1.0 - ratio).clamp(0.0, 1.0)
}
