//! Autonomous worker assignment step (SA7).

use crate::world::combat::AttackTargetingPolicy;
use crate::world::inventory::InventoryCatalogCtx;
use crate::world::logistics::{
    assign_hauling_task_with_priority, HaulingRequestPriority, HaulingRequestStatus,
};
use crate::world::task::assignment::{
    cancel_unit_task, claim_building_task, release_unit_task_to_marketplace,
};
use crate::world::task::{
    unit_can_perform_task, unit_may_work_on_building, TaskCancelReason, TaskId, TaskPriority,
    TaskState, TaskType,
};
use crate::world::{
    BuildingCatalog, BuildingInteractionProfileCatalog, DoodadCatalog, NavigationConfig,
    UnitCatalog, UnitId, UnitOrder, UnitState, WeaponCatalog, WorldData, issue_unit_order,
    interaction_point_world_position,
};

use super::candidates::{
    MarketplaceCandidate, MarketplaceListing, MarketplaceListingKind,
};
use super::report::{AssignmentDecision, WorkerAssignmentReport, WorkerEvaluation};
use super::score::{may_preempt_with_override, score_marketplace_listing, PreemptPolicyOverride};
use super::sync::sync_operate_workstation_tasks;

pub const WORKER_ASSIGNMENT_CADENCE_TICKS: u64 = 5;

pub struct WorkerAssignmentContext<'a> {
    pub world: &'a mut WorldData,
    pub unit_catalog: &'a UnitCatalog,
    pub weapon_catalog: &'a WeaponCatalog,
    pub doodad_catalog: &'a DoodadCatalog,
    pub building_catalog: &'a BuildingCatalog,
    pub interaction_catalog: &'a BuildingInteractionProfileCatalog,
    pub nav_config: &'a NavigationConfig,
    pub inventory_ctx: &'a InventoryCatalogCtx<'a>,
    pub simulation_tick: u64,
}

/// Match idle (and preemptible) workers to marketplace listings.
///
/// Settlement AI is never consulted for worker identity — only TaskStore + haul requests.
pub fn step_worker_assignment(ctx: &mut WorkerAssignmentContext<'_>) -> WorkerAssignmentReport {
    let tick = ctx.simulation_tick;
    sync_operate_workstation_tasks(ctx.world, ctx.building_catalog, tick);
    release_dead_worker_tasks(ctx.world);

    let mut report = WorkerAssignmentReport {
        generated_tick: tick,
        ..Default::default()
    };

    resume_stalled_assignments(ctx, &mut report);

    let listings = collect_open_listings(ctx);
    report.open_listings = listings.len() as u32;

    let worker_ids = eligible_worker_ids(ctx.world);
    let idle_ids: Vec<UnitId> = worker_ids
        .iter()
        .copied()
        .filter(|&id| ctx.world.task_store().unit_task_id(id).is_none())
        .collect();
    report.idle_workers = idle_ids.len() as u32;

    // Track which haul requests / saturate construct slots claimed this tick.
    let mut claimed_hauls: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
    let mut claimed_task_slots: std::collections::HashMap<u32, u32> =
        std::collections::HashMap::new();

    // Pass 1: idle workers claim best Available work (deterministic unit-id order).
    for unit_id in &idle_ids {
        let (decision, evaluation) = try_assign_worker(
            ctx,
            *unit_id,
            &listings,
            &mut claimed_hauls,
            &mut claimed_task_slots,
            false,
        );
        if let Some(decision) = decision {
            report.assignments.push(decision);
        }
        report.evaluations.push(evaluation);
    }

    // Pass 2: preemption — higher-priority listings may steal busy workers.
    let busy_ids: Vec<UnitId> = worker_ids
        .iter()
        .copied()
        .filter(|&id| ctx.world.task_store().unit_task_id(id).is_some())
        .collect();
    for unit_id in busy_ids {
        let Some(current_task_id) = ctx.world.task_store().unit_task_id(unit_id) else {
            continue;
        };
        let Some(current) = ctx.world.task_store().get(current_task_id).cloned() else {
            continue;
        };
        if current.priority == TaskPriority::PlayerAssigned {
            continue;
        }
        let stick = ctx.world.worker_assignment_store().stick(unit_id).cloned();
        let ticks_on = stick
            .as_ref()
            .map(|s| tick.saturating_sub(s.task_assigned_tick))
            .unwrap_or(MIN_STICK_FALLBACK);
        let since_preempt = stick.and_then(|s| s.last_preempt_tick.map(|t| tick.saturating_sub(t)));

        let candidates = evaluate_listings_for_worker(ctx, unit_id, &listings);
        let preempt_policy = preempt_policy_for_task(ctx.world, current.target_building_id());
        let best = candidates
            .iter()
            .filter(|c| c.eligible)
            .filter(|c| {
                // Don't preempt onto same task.
                c.listing.task_id != Some(current_task_id)
            })
            .filter(|c| {
                may_preempt_with_override(
                    c.listing.priority,
                    current.priority,
                    ticks_on,
                    since_preempt,
                    preempt_policy,
                )
            })
            .max_by(|a, b| {
                a.score
                    .partial_cmp(&b.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| listing_tiebreak(&a.listing, &b.listing))
            });
        let Some(best) = best.cloned() else {
            continue;
        };
        if !listing_still_open(ctx.world, &best.listing, &claimed_hauls, &claimed_task_slots) {
            continue;
        }

        let mut events = Vec::new();
        release_unit_task_to_marketplace(ctx.world, unit_id, &mut events);
        ctx.world.worker_assignment_store_mut().clear_stick(unit_id);

        let (decision, evaluation) = try_assign_worker(
            ctx,
            unit_id,
            &listings,
            &mut claimed_hauls,
            &mut claimed_task_slots,
            true,
        );
        if let Some(mut decision) = decision {
            decision.preempted = true;
            decision.reason = format!(
                "preempted {:?} → {:?} (score {:.1})",
                current.task_type, best.listing.task_type, best.score
            );
            report.assignments.push(decision);
        } else {
            report.diagnostics.push(format!(
                "preempt cancel for unit #{} but reassignment failed",
                unit_id.raw()
            ));
        }
        report.evaluations.push(evaluation);
    }

    report.diagnostics.push(format!(
        "idle={} listings={} assigned={}",
        report.idle_workers,
        report.open_listings,
        report.assignments.len()
    ));
    ctx.world
        .worker_assignment_store_mut()
        .set_report(report.clone());
    report
}

const MIN_STICK_FALLBACK: u64 = 0;

fn try_assign_worker(
    ctx: &mut WorkerAssignmentContext<'_>,
    unit_id: UnitId,
    listings: &[MarketplaceListing],
    claimed_hauls: &mut std::collections::BTreeSet<u32>,
    claimed_task_slots: &mut std::collections::HashMap<u32, u32>,
    preempted: bool,
) -> (Option<AssignmentDecision>, WorkerEvaluation) {
    let candidates = evaluate_listings_for_worker(ctx, unit_id, listings);
    let best = candidates
        .iter()
        .filter(|c| c.eligible)
        .filter(|c| listing_still_open(ctx.world, &c.listing, claimed_hauls, claimed_task_slots))
        .max_by(|a, b| {
            a.score
                .partial_cmp(&b.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| listing_tiebreak(&a.listing, &b.listing))
        })
        .cloned();

    let Some(best) = best else {
        return (
            None,
            WorkerEvaluation::from_candidates(
                unit_id,
                true,
                &candidates,
                None,
                0.0,
                None,
                "no eligible listing",
            ),
        );
    };

    match claim_listing(ctx, unit_id, &best.listing) {
        Ok((task_id, reservation_point)) => {
            mark_claimed(&best.listing, claimed_hauls, claimed_task_slots);
            ctx.world.worker_assignment_store_mut().note_assignment(
                unit_id,
                ctx.simulation_tick,
                preempted,
            );
            let decision = AssignmentDecision {
                unit_id,
                task_id: Some(task_id),
                score: best.score,
                priority: best.listing.priority,
                preempted,
                reason: format!(
                    "claimed {:?} on building #{}",
                    best.listing.task_type,
                    best.listing.building_id.raw()
                ),
            };
            let evaluation = WorkerEvaluation::from_candidates(
                unit_id,
                true,
                &candidates,
                Some(task_id),
                best.score,
                reservation_point,
                "assigned",
            );
            (Some(decision), evaluation)
        }
        Err(err) => (
            None,
            WorkerEvaluation::from_candidates(
                unit_id,
                true,
                &candidates,
                None,
                best.score,
                None,
                format!("claim failed: {err}"),
            ),
        ),
    }
}

fn claim_listing(
    ctx: &mut WorkerAssignmentContext<'_>,
    unit_id: UnitId,
    listing: &MarketplaceListing,
) -> Result<(TaskId, Option<String>), String> {
    match listing.kind {
        MarketplaceListingKind::HaulRequest => {
            let request_id = listing
                .haul_request_id
                .ok_or_else(|| "haul listing missing id".to_string())?;
            let (task_id, _) = assign_hauling_task_with_priority(
                ctx.world,
                ctx.unit_catalog,
                ctx.weapon_catalog,
                ctx.doodad_catalog,
                ctx.nav_config,
                ctx.inventory_ctx,
                unit_id,
                request_id,
                listing.priority,
                ctx.simulation_tick,
            )
            .map_err(|e| format!("{e:?}"))?;
            Ok((task_id, None))
        }
        MarketplaceListingKind::Task => {
            let (task_id, _) = claim_building_task(
                ctx.world,
                ctx.unit_catalog,
                ctx.weapon_catalog,
                ctx.doodad_catalog,
                ctx.building_catalog,
                ctx.interaction_catalog,
                ctx.nav_config,
                unit_id,
                listing.building_id,
                listing.task_type,
                listing.priority,
                ctx.simulation_tick,
            )
            .map_err(|e| format!("{e:?}"))?;
            let point = ctx
                .world
                .task_store()
                .get(task_id)
                .and_then(|t| t.reserved_point_key.clone());
            Ok((task_id, point))
        }
    }
}

fn mark_claimed(
    listing: &MarketplaceListing,
    claimed_hauls: &mut std::collections::BTreeSet<u32>,
    claimed_task_slots: &mut std::collections::HashMap<u32, u32>,
) {
    match listing.kind {
        MarketplaceListingKind::HaulRequest => {
            if let Some(id) = listing.haul_request_id {
                claimed_hauls.insert(id.raw());
            }
        }
        MarketplaceListingKind::Task => {
            if let Some(id) = listing.task_id {
                *claimed_task_slots.entry(id.raw()).or_insert(0) += 1;
            }
        }
    }
}

fn listing_still_open(
    world: &WorldData,
    listing: &MarketplaceListing,
    claimed_hauls: &std::collections::BTreeSet<u32>,
    claimed_task_slots: &std::collections::HashMap<u32, u32>,
) -> bool {
    match listing.kind {
        MarketplaceListingKind::HaulRequest => {
            let Some(id) = listing.haul_request_id else {
                return false;
            };
            if claimed_hauls.contains(&id.raw()) {
                return false;
            }
            world
                .hauling_request_store()
                .get(id)
                .is_some_and(|r| {
                    r.assigned_unit_id.is_none()
                        && matches!(
                            r.status,
                            HaulingRequestStatus::Pending
                                | HaulingRequestStatus::PartiallyFulfilled
                        )
                })
        }
        MarketplaceListingKind::Task => {
            let Some(task_id) = listing.task_id else {
                return false;
            };
            let Some(task) = world.task_store().get(task_id) else {
                return false;
            };
            if matches!(task.state, TaskState::Completed | TaskState::Canceled) {
                return false;
            }
            // Operate: single worker.
            if listing.task_type == TaskType::OperateWorkstation {
                return task.state == TaskState::Available
                    && claimed_task_slots.get(&task_id.raw()).copied().unwrap_or(0) == 0;
            }
            // Construct: open while free interaction points remain (approximate via claimed slots).
            true
        }
    }
}

fn collect_open_listings(ctx: &WorkerAssignmentContext<'_>) -> Vec<MarketplaceListing> {
    let mut listings = Vec::new();
    for task_id in ctx.world.task_store().sorted_task_ids() {
        let Some(task) = ctx.world.task_store().get(task_id) else {
            continue;
        };
        if !marketplace_task_type(task.task_type) {
            continue;
        }
        let open = match task.task_type {
            TaskType::ConstructBuilding => matches!(
                task.state,
                TaskState::Available | TaskState::Assigned | TaskState::InProgress
            ),
            TaskType::OperateWorkstation => task.state == TaskState::Available,
            _ => false,
        };
        if !open {
            continue;
        }
        listings.push(MarketplaceListing {
            kind: MarketplaceListingKind::Task,
            task_id: Some(task_id),
            haul_request_id: None,
            building_id: task.target_building_id(),
            task_type: task.task_type,
            priority: task.priority,
            created_tick: task.created_tick,
        });
    }
    for request_id in ctx.world.hauling_request_store().sorted_request_ids() {
        let Some(request) = ctx.world.hauling_request_store().get(request_id) else {
            continue;
        };
        if request.assigned_unit_id.is_some() {
            continue;
        }
        if !matches!(
            request.status,
            HaulingRequestStatus::Pending | HaulingRequestStatus::PartiallyFulfilled
        ) {
            continue;
        }
        listings.push(MarketplaceListing {
            kind: MarketplaceListingKind::HaulRequest,
            task_id: None,
            haul_request_id: Some(request.id),
            building_id: request.owning_building_id,
            task_type: TaskType::Haul,
            priority: haul_priority_to_task(request.priority),
            created_tick: request.created_tick,
        });
    }
    listings.sort_by(|a, b| listing_tiebreak(a, b));
    listings
}

fn marketplace_task_type(task_type: TaskType) -> bool {
    matches!(
        task_type,
        TaskType::ConstructBuilding | TaskType::OperateWorkstation
    )
}

fn haul_priority_to_task(priority: HaulingRequestPriority) -> TaskPriority {
    match priority {
        HaulingRequestPriority::Critical | HaulingRequestPriority::High => TaskPriority::High,
        HaulingRequestPriority::Normal => TaskPriority::Normal,
        HaulingRequestPriority::Low => TaskPriority::Low,
    }
}

fn eligible_worker_ids(world: &WorldData) -> Vec<UnitId> {
    world
        .sorted_unit_ids()
        .into_iter()
        .filter(|&unit_id| {
            let Some(unit) = world.get_unit(unit_id) else {
                return false;
            };
            !matches!(unit.state, UnitState::Dead)
        })
        .collect()
}

fn evaluate_listings_for_worker(
    ctx: &WorkerAssignmentContext<'_>,
    unit_id: UnitId,
    listings: &[MarketplaceListing],
) -> Vec<MarketplaceCandidate> {
    let layout = ctx.world.layout();
    let Some(unit) = ctx.world.get_unit(unit_id) else {
        return Vec::new();
    };
    let unit_pos = unit.placement.position.to_global(layout);
    let ownership = unit.ownership();

    let mut out = Vec::new();
    for listing in listings {
        let mut block = None;
        let mut eligible = true;
        if !unit_can_perform_task(ctx.unit_catalog, ctx.world, unit_id, listing.task_type) {
            eligible = false;
            block = Some("capability".into());
        }
        if let Some(building) = ctx.world.get_building(listing.building_id) {
            if !unit_may_work_on_building(building, ownership) {
                eligible = false;
                block = Some("ownership".into());
            }
        } else {
            eligible = false;
            block = Some("missing building".into());
        }
        let distance = ctx
            .world
            .get_building(listing.building_id)
            .map(|b| {
                let p = b.placement.position.to_global(layout);
                let dx = p.x - unit_pos.x;
                let dz = p.z - unit_pos.z;
                (dx * dx + dz * dz).sqrt()
            })
            .unwrap_or(0.0);
        let scored = score_marketplace_listing(listing, distance);
        out.push(MarketplaceCandidate {
            listing: listing.clone(),
            distance_meters: distance,
            score: scored.total,
            eligible,
            block_reason: block,
        });
    }
    out
}

fn listing_tiebreak(a: &MarketplaceListing, b: &MarketplaceListing) -> std::cmp::Ordering {
    a.priority
        .rank()
        .cmp(&b.priority.rank())
        .then_with(|| a.created_tick.cmp(&b.created_tick))
        .then_with(|| {
            a.task_id
                .map(|id| id.raw())
                .cmp(&b.task_id.map(|id| id.raw()))
        })
        .then_with(|| {
            a.haul_request_id
                .map(|id| id.raw())
                .cmp(&b.haul_request_id.map(|id| id.raw()))
        })
}

fn release_dead_worker_tasks(world: &mut WorldData) {
    let unit_ids = world.sorted_unit_ids();
    for unit_id in unit_ids {
        let dead = world
            .get_unit(unit_id)
            .is_some_and(|u| matches!(u.state, UnitState::Dead));
        if !dead {
            continue;
        }
        if world.task_store().unit_task_id(unit_id).is_some() {
            let mut events = Vec::new();
            cancel_unit_task(world, unit_id, TaskCancelReason::WorkerDied, &mut events);
            world.worker_assignment_store_mut().clear_stick(unit_id);
        }
    }
}

fn preempt_policy_for_task(world: &WorldData, building_id: crate::world::BuildingId) -> PreemptPolicyOverride {
    let Some(settlement_id) = world.settlement_store().settlement_for_building(building_id) else {
        return PreemptPolicyOverride::default();
    };
    let Some(state) = world.settlement_state_store().get(settlement_id) else {
        return PreemptPolicyOverride::default();
    };
    let catalog = crate::world::settlement::emergency::EmergencyCatalog::default();
    let Some(relax) =
        crate::world::settlement::emergency::emergency_preempt_relaxation(state, &catalog)
    else {
        return PreemptPolicyOverride::default();
    };
    PreemptPolicyOverride {
        min_stick_ticks: Some(relax.min_stick_ticks),
        min_priority_rank_gap: Some(relax.min_priority_rank_gap),
        max_interruptible: Some(relax.max_interruptible),
    }
}

fn resume_stalled_assignments(
    ctx: &mut WorkerAssignmentContext<'_>,
    report: &mut WorkerAssignmentReport,
) {
    let unit_ids = ctx.world.sorted_unit_ids();
    for unit_id in unit_ids {
        let Some(task_id) = ctx.world.task_store().unit_task_id(unit_id) else {
            continue;
        };
        let Some(unit) = ctx.world.get_unit(unit_id).cloned() else {
            continue;
        };
        if !matches!(unit.state, UnitState::Idle) {
            continue;
        }
        let Some(task) = ctx.world.task_store().get(task_id).cloned() else {
            continue;
        };
        if task.task_type == TaskType::Haul {
            // Haul step owns movement phases.
            continue;
        }
        let building_id = task.target_building_id();
        let Some(building) = ctx.world.get_building(building_id).cloned() else {
            continue;
        };
        let Some(definition) = ctx.building_catalog.get(&building.definition_id) else {
            continue;
        };
        let Some(profile) = ctx.interaction_catalog.profile_for_definition(definition) else {
            continue;
        };
        let Some(point_key) = task.reserved_point_key.as_deref() else {
            continue;
        };
        let Some(point) = profile.points.iter().find(|p| p.key == point_key) else {
            continue;
        };
        let target = interaction_point_world_position(&building, ctx.world.layout(), point);
        if issue_unit_order(
            ctx.world,
            ctx.unit_catalog,
            ctx.weapon_catalog,
            ctx.doodad_catalog,
            ctx.nav_config,
            unit_id,
            UnitOrder::Work { task_id, target },
            AttackTargetingPolicy::default(),
        )
        .is_ok()
        {
            report.diagnostics.push(format!(
                "resumed Idle unit #{} on task #{}",
                unit_id.raw(),
                task_id.raw()
            ));
        }
    }
}
