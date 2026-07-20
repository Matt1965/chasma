//! Construction planning evaluation — intent → ConstructionPlan (SA9).

use bevy::prelude::Quat;

use crate::world::building::catalog::BuildingCatalog;
use crate::world::{
    place_player_building_with_inventory, remove_building, rotation_from_quadrants,
    validate_building_placement, BuildingLifecycleState, BuildingOwnership,
    BuildingPlacementContext,
};
use crate::world::inventory::InventoryCatalogCtx;
use crate::world::settlement::arbiter::{SettlementIntent, SettlementIntentPlan};
use crate::world::settlement::response::ResponseType;
use crate::world::settlement::SettlementId;
use crate::world::{
    DoodadCatalog, FootprintCatalog, OccupancyCatalogs, UnitCatalog, WorldData, WorldPosition,
};

use super::capacity::{estimate_capacity_gap, fulfillment_key};
use super::catalog::{
    BuildingConstructionCostCatalog, ConstructionResponseCatalog, ConstructionResponseMapping,
};
use super::placement::{search_placement_candidates, PlacementSearchBudget};
use super::plan::{
    ConstructionMaterialRequirement, ConstructionPlan, ConstructionPlanSource,
    ConstructionPlanStatus,
};
use super::report::ConstructionPlanningReport;
use super::select::{best_building_candidate, select_building_candidates};

pub struct ConstructionPlanningContext<'a> {
    pub world: &'a mut WorldData,
    pub response_catalog: &'a ConstructionResponseCatalog,
    pub cost_catalog: &'a BuildingConstructionCostCatalog,
    pub building_catalog: &'a BuildingCatalog,
    pub footprint_catalog: &'a FootprintCatalog,
    pub doodad_catalog: &'a DoodadCatalog,
    pub unit_catalog: &'a UnitCatalog,
    pub inventory_ctx: &'a InventoryCatalogCtx<'a>,
    pub simulation_tick: u64,
}

/// Plan construction for one settlement from its current intent plan.
///
/// Does not assign workers, move materials, or spawn Complete buildings.
pub fn plan_construction_for_settlement(
    ctx: &mut ConstructionPlanningContext<'_>,
    settlement_id: SettlementId,
) -> ConstructionPlanningReport {
    let mut report = ConstructionPlanningReport::new(settlement_id, ctx.simulation_tick);

    let Some(state) = ctx.world.settlement_state_store().get(settlement_id).cloned() else {
        report
            .diagnostics
            .push("no SettlementState — skipping".into());
        return report;
    };

    sync_plan_lifecycle(ctx, settlement_id, &mut report);

    if !state.policies.automation_enabled || !state.policies.auto_construction {
        report
            .diagnostics
            .push("autonomous construction disabled by policy".into());
        ctx.world
            .construction_planning_report_store_mut()
            .insert(report.clone());
        return report;
    }

    let intent_plan = ctx
        .world
        .settlement_intent_store()
        .get(settlement_id)
        .cloned();
    if let Some(ref plan) = intent_plan {
        report.source_intent_tick = Some(plan.planned_tick);
    }

    // Refresh committed plans from matching intents; never cancel on brief pressure dips.
    if let Some(ref intent_plan) = intent_plan {
        refresh_committed_from_intents(ctx, intent_plan, &mut report);
    }

    let construct_intents: Vec<SettlementIntent> = intent_plan
        .as_ref()
        .map(|p| {
            p.intents
                .iter()
                .filter(|i| i.response_type == ResponseType::ConstructBuilding)
                .cloned()
                .collect()
        })
        .unwrap_or_default();

    if construct_intents.is_empty() {
        report
            .diagnostics
            .push("no ConstructBuilding intents this cycle".into());
        ctx.world
            .construction_planning_report_store_mut()
            .insert(report.clone());
        return report;
    }

    let max_concurrent = state.policies.max_concurrent_construction_plans.max(1) as usize;
    let max_new = state.policies.max_new_construction_plans_per_cycle.max(1);
    let mut new_created = 0u32;

    for intent in construct_intents {
        if new_created >= max_new {
            report
                .diagnostics
                .push("max_new_construction_plans_per_cycle reached".into());
            break;
        }
        let active = ctx
            .world
            .construction_plan_store()
            .active_count(settlement_id);
        if active >= max_concurrent {
            report
                .diagnostics
                .push("max_concurrent_construction_plans reached".into());
            break;
        }

        let Some(mapping) = ctx.response_catalog.get(&intent.chosen_response).cloned() else {
            report.diagnostics.push(format!(
                "no construction mapping for response `{}`",
                intent.chosen_response.as_str()
            ));
            continue;
        };
        if !mapping.enabled {
            continue;
        }
        if !mapping.creates_new_capacity {
            report.diagnostics.push(format!(
                "response `{}` advances existing sites only (no new plan)",
                mapping.response_id.as_str()
            ));
            continue;
        }

        let gap = estimate_capacity_gap(
            ctx.world,
            ctx.building_catalog,
            ctx.world.construction_plan_store(),
            settlement_id,
            &mapping,
        );
        report.capacity_notes.extend(gap.notes.clone());
        if gap.additional_needed == 0 {
            report.diagnostics.push(format!(
                "capacity sufficient for `{}` — no new plan",
                mapping.capability_key
            ));
            continue;
        }

        let considered =
            select_building_candidates(ctx.building_catalog, ctx.cost_catalog, &mapping);
        report.considered_buildings = considered.clone();
        let Some(building_def_id) =
            best_building_candidate(ctx.building_catalog, ctx.cost_catalog, &mapping)
        else {
            report.diagnostics.push(format!(
                "no eligible building for capability `{}`",
                mapping.capability_key
            ));
            continue;
        };

        let key = fulfillment_key(settlement_id, &mapping, &building_def_id);
        if let Some(existing) = ctx
            .world
            .construction_plan_store()
            .active_for_fulfillment(&key)
            .cloned()
        {
            if let Some(plan) = ctx
                .world
                .construction_plan_store_mut()
                .get_mut(existing.id)
            {
                plan.refresh_priority(intent.priority, ctx.simulation_tick);
                report.refreshed_plan_ids.push(plan.id);
            }
            report.diagnostics.push(format!(
                "dedup retained plan #{} for key `{key}`",
                existing.id.raw()
            ));
            continue;
        }

        match create_plan_for_mapping(ctx, settlement_id, &intent, &mapping, &building_def_id, &state)
        {
            Ok(plan_id) => {
                report.created_plan_ids.push(plan_id);
                new_created = new_created.saturating_add(1);
            }
            Err(diag) => report.diagnostics.push(diag),
        }
    }

    // Retry blocked plans when cadence expires.
    retry_blocked_plans(ctx, settlement_id, &state, &mut report);

    report.diagnostics.push(format!(
        "created={} refreshed={} cancelled={}",
        report.created_plan_ids.len(),
        report.refreshed_plan_ids.len(),
        report.cancelled_plan_ids.len()
    ));
    ctx.world
        .construction_planning_report_store_mut()
        .insert(report.clone());
    report
}

fn refresh_committed_from_intents(
    ctx: &mut ConstructionPlanningContext<'_>,
    intent_plan: &SettlementIntentPlan,
    report: &mut ConstructionPlanningReport,
) {
    let intents_by_response: std::collections::BTreeMap<_, _> = intent_plan
        .intents
        .iter()
        .filter(|i| i.response_type == ResponseType::ConstructBuilding)
        .map(|i| (i.chosen_response.as_str().to_string(), i.priority))
        .collect();

    let ids: Vec<_> = ctx
        .world
        .construction_plan_store()
        .plans_for_settlement(intent_plan.settlement_id)
        .into_iter()
        .filter(|p| p.status.is_committed())
        .map(|p| p.id)
        .collect();

    for id in ids {
        let Some(plan) = ctx.world.construction_plan_store_mut().get_mut(id) else {
            continue;
        };
        if let Some(&priority) = intents_by_response.get(plan.source.response_id.as_str()) {
            plan.refresh_priority(priority, ctx.simulation_tick);
            report.refreshed_plan_ids.push(id);
        }
        // Missing intent: keep committed plan (anti-oscillation).
    }
}

fn create_plan_for_mapping(
    ctx: &mut ConstructionPlanningContext<'_>,
    settlement_id: SettlementId,
    intent: &SettlementIntent,
    mapping: &ConstructionResponseMapping,
    building_def_id: &crate::world::building::catalog::BuildingDefinitionId,
    state: &crate::world::settlement::SettlementState,
) -> Result<super::plan::ConstructionPlanId, String> {
    let settlement = ctx
        .world
        .settlement_store()
        .get_settlement(settlement_id)
        .cloned()
        .ok_or_else(|| format!("settlement {} missing", settlement_id.raw()))?;
    let anchor = ctx
        .world
        .get_building(settlement.anchor_building_id)
        .map(|b| b.placement.position)
        .unwrap_or(settlement.interaction_position);

    let materials: Vec<ConstructionMaterialRequirement> = ctx
        .cost_catalog
        .materials_for(building_def_id)
        .iter()
        .map(|(item, qty)| ConstructionMaterialRequirement::new(item.clone(), *qty))
        .collect();

    let plan_id = ctx.world.construction_plan_store_mut().allocate_id();
    let key = fulfillment_key(settlement_id, mapping, building_def_id);
    let mut plan = ConstructionPlan {
        id: plan_id,
        settlement_id,
        source: ConstructionPlanSource::from_intent(
            mapping.response_id.clone(),
            &intent.intent_id,
            intent.source_need.as_str(),
        ),
        building_definition_id: building_def_id.clone(),
        required_capability: mapping.capability_key.clone(),
        fulfillment_key: key,
        placement: None,
        reserved_building_id: None,
        priority: intent.priority,
        required_materials: materials,
        status: ConstructionPlanStatus::SiteSearch,
        blocking_reason: None,
        created_tick: ctx.simulation_tick,
        updated_tick: ctx.simulation_tick,
        next_retry_tick: None,
        retry_count: 0,
        player_approved: !state.policies.require_construction_approval,
        diagnostics: Vec::new(),
    };

    if state.policies.require_construction_approval && state.policies.player_controlled {
        plan.status = ConstructionPlanStatus::AwaitingApproval;
        plan.diagnostics.push("awaiting player plan approval".into());
        ctx.world.construction_plan_store_mut().insert(plan);
        return Ok(plan_id);
    }

    let budget = PlacementSearchBudget {
        search_radius_meters: state.policies.construction_search_radius_meters,
        step_meters: 8.0,
        max_candidates: state.policies.max_placement_candidates_per_pass,
    };
    let ownership = BuildingOwnership {
        owner_id: settlement.ownership.owner_id,
        team_id: settlement.ownership.team_id,
        affiliation: settlement.ownership.affiliation,
    };

    let search = search_placement_candidates(
        ctx.world,
        ctx.building_catalog,
        ctx.footprint_catalog,
        ctx.doodad_catalog,
        ctx.unit_catalog,
        building_def_id,
        ownership,
        anchor,
        budget,
    );
    plan.diagnostics.extend(search.diagnostics);

    let Some(site) = search.selected else {
        plan.status = ConstructionPlanStatus::Blocked;
        plan.blocking_reason = Some("no valid site inside search budget".into());
        plan.next_retry_tick = Some(
            ctx.simulation_tick
                .saturating_add(state.policies.blocked_plan_retry_ticks.max(1)),
        );
        // Store rejected sites on the transient report via caller — attach summary here.
        plan.diagnostics.push(format!(
            "site_search rejected={}",
            search.rejected.len()
        ));
        ctx.world.construction_plan_store_mut().insert(plan);
        return Ok(plan_id);
    };

    if state.policies.require_construction_placement_approval && state.policies.player_controlled {
        plan.placement = Some(site);
        plan.status = ConstructionPlanStatus::AwaitingApproval;
        plan.diagnostics
            .push("site selected — awaiting placement approval".into());
        ctx.world.construction_plan_store_mut().insert(plan);
        return Ok(plan_id);
    }

    commit_site(ctx, &mut plan, site, ownership)?;
    ctx.world.construction_plan_store_mut().insert(plan);
    Ok(plan_id)
}

fn commit_site(
    ctx: &mut ConstructionPlanningContext<'_>,
    plan: &mut ConstructionPlan,
    site: super::plan::ConstructionPlacementCandidate,
    ownership: BuildingOwnership,
) -> Result<(), String> {
    let occupancy = OccupancyCatalogs {
        doodad: ctx.doodad_catalog,
        building: ctx.building_catalog,
        footprint: ctx.footprint_catalog,
    };
    let rotation = rotation_from_quadrants(site.yaw_quadrants);
    let record = place_player_building_with_inventory(
        ctx.building_catalog,
        ctx.world,
        &plan.building_definition_id,
        site.position(),
        rotation,
        ownership,
        occupancy,
        ctx.inventory_ctx,
    )
    .map_err(|e| format!("place Planned building failed: {e:?}"))?;

    let _ = ctx
        .world
        .settlement_store_mut()
        .link_building_to_settlement(plan.settlement_id, record.id);

    plan.placement = Some(site);
    plan.reserved_building_id = Some(record.id);
    plan.blocking_reason = None;
    plan.updated_tick = ctx.simulation_tick;

    if plan.materials_satisfied() {
        plan.status = ConstructionPlanStatus::Ready;
        plan.diagnostics
            .push("site reserved — Ready (materials satisfied or none required)".into());
    } else {
        plan.status = ConstructionPlanStatus::AwaitingMaterials;
        plan.diagnostics
            .push("site reserved — AwaitingMaterials".into());
    }
    Ok(())
}

fn retry_blocked_plans(
    ctx: &mut ConstructionPlanningContext<'_>,
    settlement_id: SettlementId,
    state: &crate::world::settlement::SettlementState,
    report: &mut ConstructionPlanningReport,
) {
    let blocked: Vec<_> = ctx
        .world
        .construction_plan_store()
        .plans_for_settlement(settlement_id)
        .into_iter()
        .filter(|p| p.status == ConstructionPlanStatus::Blocked)
        .cloned()
        .collect();

    for mut plan in blocked {
        let due = plan
            .next_retry_tick
            .is_none_or(|t| ctx.simulation_tick >= t);
        if !due {
            continue;
        }
        let Some(settlement) = ctx
            .world
            .settlement_store()
            .get_settlement(settlement_id)
            .cloned()
        else {
            continue;
        };
        let anchor = ctx
            .world
            .get_building(settlement.anchor_building_id)
            .map(|b| b.placement.position)
            .unwrap_or(settlement.interaction_position);
        let ownership = BuildingOwnership {
            owner_id: settlement.ownership.owner_id,
            team_id: settlement.ownership.team_id,
            affiliation: settlement.ownership.affiliation,
        };
        let budget = PlacementSearchBudget {
            search_radius_meters: state.policies.construction_search_radius_meters,
            step_meters: 8.0,
            max_candidates: state.policies.max_placement_candidates_per_pass,
        };
        let search = search_placement_candidates(
            ctx.world,
            ctx.building_catalog,
            ctx.footprint_catalog,
            ctx.doodad_catalog,
            ctx.unit_catalog,
            &plan.building_definition_id,
            ownership,
            anchor,
            budget,
        );
        report.rejected_sites.extend(search.rejected);
        if let Some(site) = search.selected {
            if let Err(err) = commit_site(ctx, &mut plan, site, ownership) {
                plan.blocking_reason = Some(err);
                plan.retry_count = plan.retry_count.saturating_add(1);
                plan.next_retry_tick = Some(
                    ctx.simulation_tick
                        .saturating_add(state.policies.blocked_plan_retry_ticks.max(1)),
                );
            }
        } else {
            plan.retry_count = plan.retry_count.saturating_add(1);
            plan.next_retry_tick = Some(
                ctx.simulation_tick
                    .saturating_add(state.policies.blocked_plan_retry_ticks.max(1)),
            );
            plan.blocking_reason = Some("no valid site on retry".into());
        }
        plan.updated_tick = ctx.simulation_tick;
        ctx.world.construction_plan_store_mut().insert(plan);
    }
}

fn sync_plan_lifecycle(
    ctx: &mut ConstructionPlanningContext<'_>,
    settlement_id: SettlementId,
    report: &mut ConstructionPlanningReport,
) {
    let plans: Vec<_> = ctx
        .world
        .construction_plan_store()
        .plans_for_settlement(settlement_id)
        .into_iter()
        .cloned()
        .collect();

    for mut plan in plans {
        if plan.status.is_terminal() {
            continue;
        }
        // Materials seam: planning does not move items; when delivered catches up, advance.
        if plan.status == ConstructionPlanStatus::AwaitingMaterials && plan.materials_satisfied() {
            plan.status = ConstructionPlanStatus::Ready;
            plan.diagnostics.push("materials satisfied → Ready".into());
            plan.updated_tick = ctx.simulation_tick;
        }

        if let Some(building_id) = plan.reserved_building_id {
            match ctx.world.get_building(building_id).map(|b| b.lifecycle_state) {
                None => {
                    plan.status = ConstructionPlanStatus::Blocked;
                    plan.blocking_reason = Some("reserved building missing".into());
                    plan.reserved_building_id = None;
                    plan.updated_tick = ctx.simulation_tick;
                }
                Some(BuildingLifecycleState::Complete) => {
                    plan.status = ConstructionPlanStatus::Completed;
                    plan.updated_tick = ctx.simulation_tick;
                    plan.diagnostics
                        .push("building complete → plan Completed".into());
                }
                Some(BuildingLifecycleState::InProgress)
                | Some(BuildingLifecycleState::Foundation) => {
                    if matches!(
                        plan.status,
                        ConstructionPlanStatus::Ready | ConstructionPlanStatus::AwaitingMaterials
                    ) {
                        plan.status = ConstructionPlanStatus::InProgress;
                        plan.updated_tick = ctx.simulation_tick;
                    }
                }
                Some(BuildingLifecycleState::Destroyed) | Some(BuildingLifecycleState::Ruins) => {
                    plan.status = ConstructionPlanStatus::Cancelled;
                    plan.blocking_reason = Some("building destroyed".into());
                    plan.reserved_building_id = None;
                    plan.updated_tick = ctx.simulation_tick;
                    report.cancelled_plan_ids.push(plan.id);
                }
                Some(BuildingLifecycleState::Planned) => {}
            }
        }

        ctx.world.construction_plan_store_mut().insert(plan);
    }
}

/// Explicit cancellation — releases spatial reservation (Planned building).
pub fn cancel_construction_plan(
    world: &mut WorldData,
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    doodad_catalog: &DoodadCatalog,
    plan_id: super::plan::ConstructionPlanId,
    reason: impl Into<String>,
    simulation_tick: u64,
) -> Result<(), String> {
    let mut plan = world
        .construction_plan_store()
        .get(plan_id)
        .cloned()
        .ok_or_else(|| format!("plan {} not found", plan_id.raw()))?;
    if plan.status.is_terminal() {
        return Ok(());
    }
    if let Some(building_id) = plan.reserved_building_id.take() {
        let occupancy = OccupancyCatalogs {
            doodad: doodad_catalog,
            building: building_catalog,
            footprint: footprint_catalog,
        };
        // Only remove incomplete reservation buildings.
        if world
            .get_building(building_id)
            .is_some_and(|b| {
                matches!(
                    b.lifecycle_state,
                    BuildingLifecycleState::Planned
                        | BuildingLifecycleState::Foundation
                        | BuildingLifecycleState::InProgress
                )
            })
        {
            let _ = remove_building(
                world,
                building_id,
                Some(occupancy),
                Some(building_catalog),
                Some(doodad_catalog),
                None,
                None,
            );
            world.settlement_store_mut().unlink_building(building_id);
        }
    }
    plan.status = ConstructionPlanStatus::Cancelled;
    plan.blocking_reason = Some(reason.into());
    plan.updated_tick = simulation_tick;
    world.construction_plan_store_mut().insert(plan);
    Ok(())
}

/// Approve an awaiting plan and commit its site when needed.
pub fn approve_construction_plan(
    ctx: &mut ConstructionPlanningContext<'_>,
    plan_id: super::plan::ConstructionPlanId,
) -> Result<(), String> {
    let mut plan = ctx
        .world
        .construction_plan_store()
        .get(plan_id)
        .cloned()
        .ok_or_else(|| format!("plan {} not found", plan_id.raw()))?;
    if plan.status != ConstructionPlanStatus::AwaitingApproval {
        return Err(format!(
            "plan {} is not awaiting approval ({})",
            plan_id.raw(),
            plan.status.as_str()
        ));
    }
    plan.player_approved = true;
    let settlement = ctx
        .world
        .settlement_store()
        .get_settlement(plan.settlement_id)
        .cloned()
        .ok_or_else(|| "settlement missing".to_string())?;
    let ownership = BuildingOwnership {
        owner_id: settlement.ownership.owner_id,
        team_id: settlement.ownership.team_id,
        affiliation: settlement.ownership.affiliation,
    };

    if plan.reserved_building_id.is_some() {
        plan.status = if plan.materials_satisfied() {
            ConstructionPlanStatus::Ready
        } else {
            ConstructionPlanStatus::AwaitingMaterials
        };
    } else if let Some(site) = plan.placement.clone() {
        commit_site(ctx, &mut plan, site, ownership)?;
    } else {
        // Need a site search after plan-level approval.
        let state = ctx
            .world
            .settlement_state_store()
            .get(plan.settlement_id)
            .cloned()
            .ok_or_else(|| "SettlementState missing".to_string())?;
        let anchor = ctx
            .world
            .get_building(settlement.anchor_building_id)
            .map(|b| b.placement.position)
            .unwrap_or(settlement.interaction_position);
        let budget = PlacementSearchBudget {
            search_radius_meters: state.policies.construction_search_radius_meters,
            step_meters: 8.0,
            max_candidates: state.policies.max_placement_candidates_per_pass,
        };
        let search = search_placement_candidates(
            ctx.world,
            ctx.building_catalog,
            ctx.footprint_catalog,
            ctx.doodad_catalog,
            ctx.unit_catalog,
            &plan.building_definition_id,
            ownership,
            anchor,
            budget,
        );
        let Some(site) = search.selected else {
            plan.status = ConstructionPlanStatus::Blocked;
            plan.blocking_reason = Some("no valid site after approval".into());
            plan.next_retry_tick = Some(
                ctx.simulation_tick
                    .saturating_add(state.policies.blocked_plan_retry_ticks.max(1)),
            );
            ctx.world.construction_plan_store_mut().insert(plan);
            return Ok(());
        };
        commit_site(ctx, &mut plan, site, ownership)?;
    }
    plan.updated_tick = ctx.simulation_tick;
    ctx.world.construction_plan_store_mut().insert(plan);
    Ok(())
}

/// Manual player placement creates the same authoritative ConstructionPlan structure.
pub fn create_plan_from_manual_placement(
    ctx: &mut ConstructionPlanningContext<'_>,
    settlement_id: SettlementId,
    building_definition_id: crate::world::building::catalog::BuildingDefinitionId,
    position: WorldPosition,
    yaw_quadrants: u8,
    response_id: crate::world::settlement::response::ResponseId,
    capability_key: impl Into<String>,
    priority: f32,
) -> Result<super::plan::ConstructionPlanId, String> {
    let settlement = ctx
        .world
        .settlement_store()
        .get_settlement(settlement_id)
        .cloned()
        .ok_or_else(|| "settlement missing".to_string())?;
    let ownership = BuildingOwnership {
        owner_id: settlement.ownership.owner_id,
        team_id: settlement.ownership.team_id,
        affiliation: settlement.ownership.affiliation,
    };
    let materials: Vec<ConstructionMaterialRequirement> = ctx
        .cost_catalog
        .materials_for(&building_definition_id)
        .iter()
        .map(|(item, qty)| ConstructionMaterialRequirement::new(item.clone(), *qty))
        .collect();
    let plan_id = ctx.world.construction_plan_store_mut().allocate_id();
    let capability_key = capability_key.into();
    let mut plan = ConstructionPlan {
        id: plan_id,
        settlement_id,
        source: ConstructionPlanSource::manual(response_id.clone()),
        building_definition_id: building_definition_id.clone(),
        required_capability: capability_key.clone(),
        fulfillment_key: format!(
            "{}:{}:{}:{}",
            settlement_id.raw(),
            response_id.as_str(),
            capability_key,
            building_definition_id.as_str()
        ),
        placement: None,
        reserved_building_id: None,
        priority,
        required_materials: materials,
        status: ConstructionPlanStatus::SiteSearch,
        blocking_reason: None,
        created_tick: ctx.simulation_tick,
        updated_tick: ctx.simulation_tick,
        next_retry_tick: None,
        retry_count: 0,
        player_approved: true,
        diagnostics: vec!["manual placement".into()],
    };
    let site = super::plan::ConstructionPlacementCandidate::from_world_position(
        position,
        yaw_quadrants,
        0,
    );
    // Validate hard constraints before commit.
    let rotation = Quat::from_rotation_y(yaw_quadrants as f32 * std::f32::consts::FRAC_PI_2);
    let validation = validate_building_placement(
        &BuildingPlacementContext {
            world: ctx.world,
            building_catalog: ctx.building_catalog,
            footprint_catalog: ctx.footprint_catalog,
            doodad_catalog: ctx.doodad_catalog,
            unit_catalog: ctx.unit_catalog,
            config: Default::default(),
            player_authorized: true,
        },
        &building_definition_id,
        position,
        rotation,
        ownership,
    );
    if !validation.valid {
        return Err(validation
            .primary_reason
            .map(|r| r.label().to_string())
            .unwrap_or_else(|| "invalid placement".into()));
    }
    commit_site(ctx, &mut plan, site, ownership)?;
    ctx.world.construction_plan_store_mut().insert(plan);
    Ok(plan_id)
}
