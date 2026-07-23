//! Read-only capture from [`WorldData`] (ADR-048). Never mutates simulation.

use bevy::prelude::*;

use crate::debug::blocked_reason_label;

use crate::ui::gameplay::combat_display::{
    attack_cycle_summary, combat_target_id, weapon_display_for_unit,
};
use crate::world::{
    BuildingCatalog, BuildingId, ChunkCoord, DoodadCatalog, FootprintCatalog,
    InteractionQueryContext, NavigationPath, NavigationWaypoint, SlopeWalkability, SpaceId,
    SteeringContext, SteeringSettings, UnitCatalog, UnitId, UnitMovementTrace, UnitState,
    WeaponCatalog, WorldData, WorldPosition, alignment_force, blocking_doodad_at_position,
    classify_slope_walkability, cohesion_force, gather_steering_neighbors, ground_world_position,
    interaction_plan_to_unit_order, is_building_operational, query_world_interaction,
    resolve_interaction_to_order, separation_force, unit_spacing_meters,
};

use super::snapshot::{
    BuildingAssetPresentationInfo, BuildingBlueprintInspectorSnapshot, BuildingInspectorSnapshot,
    ChunkResidencySnapshot, CombatInspectorSnapshot, FormationInspectorSnapshot,
    InteractionInspectorSnapshot, PathInspectorSnapshot, ProjectileInspectorSnapshot,
    SteeringInspectorSnapshot, UnitInspectorSnapshot,
};

const STEERING_SETTINGS: SteeringSettings = SteeringSettings::DEFAULT;
const FORMATION_TARGET_EPSILON: f32 = 0.25;

/// Capture a full unit inspection snapshot. Returns `None` if the unit does not exist.
pub fn capture_unit_inspector_snapshot(
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    unit_id: UnitId,
    simulation_tick: u64,
    last_block: Option<&UnitMovementTrace>,
) -> Option<UnitInspectorSnapshot> {
    let record = world.get_unit(unit_id)?.clone();
    let definition = unit_catalog.get(&record.definition_id)?;
    let layout = world.layout();
    let position = record.placement.position;

    let path = capture_path_inspector(&record.state, position, layout);
    let formation = capture_formation_inspector(
        world,
        unit_id,
        &record.state,
        layout,
        definition.collision_radius_meters,
    );
    let steering = capture_steering_inspector(
        world,
        unit_catalog,
        unit_id,
        position,
        &record.state,
        definition.collision_radius_meters,
        layout,
    );
    let block_reason = last_block
        .map(|trace| blocked_reason_label(trace.reason).to_string())
        .or_else(|| {
            diagnose_block_reason(
                world,
                doodad_catalog,
                building_catalog,
                footprint_catalog,
                position,
                definition.collision_radius_meters,
            )
        });
    let chunk = capture_chunk_residency(world, unit_id)?;

    let inventory_summary = record.inventory_id.map(|inventory_id| {
        world
            .inventory_store()
            .get(inventory_id)
            .map(|inv| {
                format!(
                    "inventory={inventory_id:?} {} entries, {}g carried",
                    inv.placed_entries().len(),
                    inv.total_mass_grams()
                )
            })
            .unwrap_or_else(|| format!("inventory={inventory_id:?} missing"))
    });

    let (display_floor_label, current_space_id) = if record.current_space_id.is_surface() {
        ("Surface".to_string(), SpaceId::SURFACE)
    } else {
        world
            .space_registry()
            .get_space(record.current_space_id)
            .map(|space| (space.display_floor_label.clone(), space.id))
            .unwrap_or_else(|| {
                (
                    format!("Space {}", record.current_space_id.raw()),
                    record.current_space_id,
                )
            })
    };

    let combat = capture_combat_inspector(&record, unit_catalog, weapon_catalog);
    let projectiles = capture_projectiles_for_unit(world, unit_id);

    Some(UnitInspectorSnapshot {
        unit_id,
        definition_id: record.definition_id.clone(),
        state_label: state_label(&record.state),
        current_hp: record.vitals.current_hp,
        max_hp: record.vitals.max_hp,
        combat_state_label: record.combat_state.label().to_string(),
        combat,
        projectiles,
        path,
        formation,
        steering,
        block_reason,
        chunk,
        simulation_tick,
        current_space_id,
        display_floor_label,
        inventory_summary,
    })
}

/// Capture a building inspection snapshot. Returns `None` if the building does not exist.
pub fn capture_building_inspector_snapshot(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    interaction_catalog: &crate::world::BuildingInteractionProfileCatalog,
    building_id: BuildingId,
    presentation: Option<BuildingAssetPresentationInfo>,
    operation_probe: Option<BuildingOperationProbe>,
) -> Option<BuildingInspectorSnapshot> {
    let record = world.get_building(building_id)?.clone();
    let definition = building_catalog.get(&record.definition_id)?;
    let chunk = world.building_chunk(building_id)?.coord();
    let inventory_summary = record.inventory_id.map(|inventory_id| {
        let entries = world
            .inventory_store()
            .get(inventory_id)
            .map(|inv| {
                format!(
                    "{} entries, {}g",
                    inv.placed_entries().len(),
                    inv.total_mass_grams()
                )
            })
            .unwrap_or_else(|| "missing".to_string());
        format!(
            "inventory={inventory_id:?} {entries} locked={}",
            record.container_locked
        )
    });
    let inventory_bindings_summary = world
        .building_inventory_binding_store()
        .get(building_id)
        .map(|set| {
            set.bindings()
                .iter()
                .map(|binding| {
                    format!(
                        "{} [{}] inv={:?}{}",
                        binding.binding_id,
                        binding.role.label(),
                        binding.inventory_id,
                        if binding.is_default { " (default)" } else { "" }
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        });
    let interaction_point = definition
        .inventory_interaction_point_key
        .as_deref()
        .and_then(|key| {
            interaction_catalog
                .profile_for_definition(definition)
                .and_then(|profile| profile.points.iter().find(|p| p.key == key))
                .map(|point| point.key.to_string())
        });
    let presentation = presentation.unwrap_or_default();
    let probe = operation_probe.unwrap_or_default();
    Some(BuildingInspectorSnapshot {
        building_id,
        definition_id: record.definition_id.clone(),
        display_name: definition.display_name.clone(),
        current_hp: record.vitals.current_hp,
        max_hp: record.vitals.max_hp,
        lifecycle_state: record.lifecycle_state.label().to_string(),
        progress_percent: (record.construction.progress_0_1 * 100.0).clamp(0.0, 100.0),
        operational: is_building_operational(&record),
        affiliation: record.ownership.affiliation.label().to_string(),
        chunk,
        inventory_summary,
        interaction_point,
        desired_render_key: presentation.desired_render_key,
        resolved_asset_path: presentation.resolved_asset_path,
        asset_load_state: presentation.asset_load_state,
        runtime_entity: presentation.runtime_entity,
        uses_diagnostic_fallback: presentation.uses_diagnostic_fallback,
        fallback_reason: presentation.fallback_reason,
        space_tag_count: presentation.space_tag_count,
        roof_tag_count: presentation.roof_tag_count,
        terrain_output_rate: probe.terrain_output_rate,
        final_output_rate: probe.final_output_rate,
        operation_progress: probe.operation_progress,
        operation_completions: probe.operation_completions,
        operation_limiting_factor: probe.operation_limiting_factor,
        production_lifecycle: probe.lifecycle,
        selected_operation: probe.selected_operation,
        policy_enabled: probe.policy_enabled,
        policy_paused: probe.policy_paused,
        repeat_mode: probe.repeat_mode,
        control_source: probe.control_source,
        policy_priority: probe.priority,
        assigned_workers: probe.assigned_workers,
        production_blocking_reason: probe.blocking_reason,
        active_worker_count: probe.active_worker_count,
        remaining_repeat_count: probe.remaining_repeat_count,
        last_efficiency_revision: probe.last_efficiency_revision,
        supported_operations: probe.supported_operations,
        default_operation: probe.default_operation,
        operation_category: probe.operation_category,
        base_labor: probe.base_labor,
        max_workers: probe.max_workers,
        validation_state: probe.validation_state,
        execution_inputs_summary: probe.execution_inputs_summary,
        execution_outputs_summary: probe.execution_outputs_summary,
        execution_inventory_summary: probe.execution_inventory_summary,
        execution_blocking: probe.execution_blocking,
        terrain_assessment_summary: probe.terrain_assessment_summary,
        terrain_assessment_revision: probe.terrain_assessment_revision,
        terrain_assessment_stale: probe.terrain_assessment_stale,
        inventory_bindings_summary,
        hauling_requests_summary: Some(format_hauling_requests_for_building(world, building_id)),
        planner_summary: format_settlement_planner_summary(world, building_id),
    })
}

/// Capture navigation blueprint inspection data for a building (NV1.2.5).
pub fn capture_building_blueprint_inspection_snapshot(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    nav_catalog: &crate::world::BuildingNavigationBlueprintCatalog,
    building_id: BuildingId,
    selected_floor_id: Option<i32>,
) -> Option<BuildingBlueprintInspectorSnapshot> {
    use crate::world::{
        NAVIGATION_BLUEPRINT_CACHE_MANIFEST_PATH, NAVIGATION_BLUEPRINT_GENERATOR_VERSION,
        NavigationBlueprintCacheManifest, building_model_world_transform,
        resolve_building_navigation_blueprint, should_generate_navigation_blueprint,
        validate_blueprint_for_inspection,
    };
    #[cfg(feature = "data-import")]
    use crate::world::{blueprint_id_for_building, hash_asset_path};

    let record = world.get_building(building_id)?;
    let definition = building_catalog.get(&record.definition_id)?;
    let layout = world.layout();

    let resolved = resolve_building_navigation_blueprint(
        definition,
        nav_catalog,
        record.interior.navigation_blueprint_override.as_ref(),
    )
    .ok()
    .flatten();

    let blueprint_source = crate::world::classify_blueprint_authority(
        definition,
        nav_catalog,
        record.interior.navigation_blueprint_override.as_ref(),
    )
    .label()
    .to_string();

    let manifest_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(NAVIGATION_BLUEPRINT_CACHE_MANIFEST_PATH);
    let manifest = NavigationBlueprintCacheManifest::load_from_path(&manifest_path);

    #[cfg(feature = "data-import")]
    let (cache_fresh, source_fingerprint, generation_status) = {
        if !should_generate_navigation_blueprint(definition) {
            (
                false,
                None,
                if resolved.is_some() {
                    "authored (not auto-generated)".to_string()
                } else {
                    "not configured".to_string()
                },
            )
        } else {
            let blueprint_id = blueprint_id_for_building(definition);
            let collision_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("assets/buildings")
                .join(format!(
                    "{}.glb",
                    definition
                        .collision_render_key
                        .0
                        .clone()
                        .or(definition.render_key.0.clone())
                        .unwrap_or_default()
                ));
            let collision_hash = hash_asset_path(&collision_path).unwrap_or_default();
            let render_hash = definition.render_key.0.as_deref().and_then(|key| {
                hash_asset_path(
                    &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("assets/buildings")
                        .join(format!("{key}.glb")),
                )
            });
            let baseline_scale_milli = {
                let vec = definition.asset_sizing.resolved_baseline_scale().to_vec3();
                Some((vec.x * 1000.0).round() as i32)
            };
            let fresh = manifest.is_fresh(
                &blueprint_id,
                &collision_hash,
                render_hash.as_deref(),
                baseline_scale_milli,
            );
            let in_catalog = nav_catalog.get(&blueprint_id).is_some();
            let status = if resolved.is_none() {
                "missing".to_string()
            } else if !in_catalog {
                "missing from catalog".to_string()
            } else if manifest.generator_version != NAVIGATION_BLUEPRINT_GENERATOR_VERSION {
                "stale generator version".to_string()
            } else if fresh {
                "cached".to_string()
            } else {
                "stale inputs".to_string()
            };
            (fresh, Some(collision_hash), status)
        }
    };

    #[cfg(not(feature = "data-import"))]
    let (cache_fresh, source_fingerprint, generation_status) = (
        false,
        None,
        if resolved.is_some() {
            "loaded".to_string()
        } else {
            "not configured".to_string()
        },
    );

    let blueprint = resolved.as_ref().map(|r| r.blueprint().clone());
    let validation = blueprint
        .as_ref()
        .map(validate_blueprint_for_inspection)
        .unwrap_or_default();

    let floor_ids = blueprint
        .as_ref()
        .map(|bp| bp.floors.iter().map(|f| f.floor_id).collect::<Vec<_>>())
        .unwrap_or_default();
    let floor_id = selected_floor_id
        .filter(|id| floor_ids.contains(id))
        .or_else(|| floor_ids.first().copied());

    let transform = building_model_world_transform(definition, &record.placement, layout);
    let building_center = transform.translation;
    let world_bounds_radius = blueprint
        .as_ref()
        .map(|bp| {
            bp.floors
                .iter()
                .flat_map(|floor| {
                    floor.walkable_outline.vertices_xz.iter().map(|&[x, z]| {
                        transform
                            .transform_point(Vec3::new(x, floor.elevation_meters, z))
                            .xz()
                            .distance(building_center.xz())
                    })
                })
                .fold(4.0_f32, f32::max)
        })
        .unwrap_or(6.0);

    let (selected_floor_vertex_count, selected_floor_elevation, selected_floor_entrances, selected_floor_transitions) =
        if let (Some(bp), Some(fid)) = (blueprint.as_ref(), floor_id) {
            if let Some(floor) = bp.floors.iter().find(|f| f.floor_id == fid) {
                let entrances = bp
                    .entrances
                    .iter()
                    .filter(|e| e.floor_key == floor.key)
                    .map(|e| {
                        format!(
                            "{} @ [{:.1},{:.1}] r={:.1}m",
                            e.key, e.local_position_xz[0], e.local_position_xz[1], e.radius_meters
                        )
                    })
                    .collect();
                let transitions = bp
                    .vertical_transitions
                    .iter()
                    .filter(|t| {
                        bp.floors
                            .iter()
                            .find(|f| f.key == t.from_floor_key)
                            .map(|f| f.floor_id == fid)
                            .unwrap_or(false)
                    })
                    .map(|t| format!("{} {:?} → {}", t.key, t.kind, t.to_floor_key))
                    .collect();
                (
                    floor.walkable_outline.vertices_xz.len(),
                    Some(floor.elevation_meters),
                    entrances,
                    transitions,
                )
            } else {
                (0, None, Vec::new(), Vec::new())
            }
        } else {
            (0, None, Vec::new(), Vec::new())
        };

    Some(BuildingBlueprintInspectorSnapshot {
        blueprint_id: blueprint.as_ref().map(|bp| bp.id.as_str().to_string()),
        blueprint_source,
        generator_version: NAVIGATION_BLUEPRINT_GENERATOR_VERSION,
        generation_status,
        cache_fresh,
        source_fingerprint,
        floor_ids,
        selected_floor_id: floor_id,
        selected_floor_vertex_count,
        selected_floor_elevation,
        selected_floor_entrances,
        selected_floor_transitions,
        entrance_count: blueprint.as_ref().map(|bp| bp.entrances.len()).unwrap_or(0),
        transition_count: blueprint
            .as_ref()
            .map(|bp| bp.vertical_transitions.len())
            .unwrap_or(0),
        validation,
        inspection_active: false,
        edit_active: false,
        edit_dirty: false,
        selected_element: None,
        variant_draft_active: false,
        variant_draft_display_name: None,
        variant_draft_asset_id: None,
        variant_draft_description: None,
        variant_draft_active_field: None,
        building_center,
        world_bounds_radius,
        resolved_blueprint: blueprint,
    })
}

fn format_settlement_planner_summary(
    world: &WorldData,
    building_id: crate::world::BuildingId,
) -> Option<String> {
    let settlement_id = world.settlement_store().settlement_for_building(building_id)?;
    let mut lines = Vec::new();
    if let Some(state) = world.settlement_state_store().get(settlement_id) {
        lines.push(format!(
            "SettlementState #{} kind={} player_controlled={}",
            settlement_id.raw(),
            state.kind.as_str(),
            state.policies.player_controlled
        ));
        lines.push(format!(
            "  policies: aggression={} expand={} auto={} planner_enabled={}",
            state.policies.aggression,
            state.policies.expansion_enabled,
            state.policies.automation_enabled,
            state.policies.planner_enabled
        ));
        lines.push(format!(
            "  need_targets={} modifiers={} emergencies={{starvation={}, attack={}, disease={}, evacuate={}, instances={}}} auto_em={{resp={}, prod={}, irq={}}}",
            state.need_targets.len(),
            state.modifiers.len(),
            state.emergencies.starvation,
            state.emergencies.under_attack,
            state.emergencies.disease,
            state.emergencies.evacuation,
            state.emergencies.instances.len(),
            state.policies.auto_emergency_response,
            state.policies.auto_production_reprioritize,
            state.policies.auto_task_interruption
        ));
        for inst in &state.emergencies.instances {
            lines.push(format!(
                "    active `{}` sev={:.2} signal={:.2} since={} force={} suppress={} ack={}",
                inst.emergency_id,
                inst.severity,
                inst.last_signal,
                inst.activated_tick,
                inst.manual_force,
                inst.manual_suppress,
                inst.acknowledged
            ));
        }
        if let Some(report) = world.emergency_evaluation_store().get(settlement_id) {
            lines.push(format!(
                "Emergency evaluation @ tick {}:",
                report.evaluated_tick
            ));
            for sig in &report.signals {
                lines.push(format!(
                    "  signal `{}`={:.2} act>={:.2} deact<={:.2} ({})",
                    sig.emergency_id,
                    sig.signal,
                    sig.activation_threshold,
                    sig.deactivation_threshold,
                    sig.evaluator
                ));
            }
            for id in &report.activated {
                lines.push(format!("  activated: {id}"));
            }
            for id in &report.deactivated {
                lines.push(format!("  deactivated: {id}"));
            }
            for diag in &report.diagnostics {
                lines.push(format!("  diag: {diag}"));
            }
        }
        lines.push(format!(
            "  planner_lifecycle: enabled={} paused={} dirty={} last={} next={} interval={}",
            state.planner.enabled,
            state.planner.paused,
            state.planner.dirty,
            state.planner.last_evaluation_tick,
            state.planner.next_scheduled_evaluation_tick,
            state.planner.evaluation_interval_ticks
        ));
        if !state.need_targets.is_empty() {
            lines.push("  targets:".to_string());
            for target in &state.need_targets {
                lines.push(format!(
                    "    {} value={} weight={:.2}",
                    target.category.as_str(),
                    target.target_value,
                    target.weight
                ));
            }
        }
        if !state.modifiers.is_empty() {
            lines.push("  modifiers:".to_string());
            for modifier in &state.modifiers {
                lines.push(format!(
                    "    {} mag={:.2} expires={:?}",
                    modifier.key, modifier.magnitude, modifier.expires_tick
                ));
            }
        }
    } else {
        lines.push(format!(
            "SettlementState #{}: MISSING",
            settlement_id.raw()
        ));
    }

    if let Some(eval) = world.need_evaluation_store().get(settlement_id) {
        lines.push(format!(
            "Need evaluation @ tick {} ({} needs):",
            eval.evaluated_tick,
            eval.snapshots.len()
        ));
        for snap in &eval.snapshots {
            lines.push(format!(
                "  {} cur={:.1} tgt={:.1} pressure={} src={}",
                snap.need_id.as_str(),
                snap.current_value,
                snap.desired_value,
                snap.pressure,
                snap.evaluation_source
            ));
        }
        for diag in &eval.diagnostics {
            lines.push(format!("  diag: {diag}"));
        }
    } else {
        lines.push("Need evaluation: (none — awaiting SA2 step)".to_string());
    }

    if let Some(responses) = world.response_candidate_store().get(settlement_id) {
        lines.push(format!(
            "Response candidates @ tick {} (from needs @ {}, {} options):",
            responses.evaluated_tick,
            responses.source_need_tick,
            responses.candidates.len()
        ));
        for candidate in &responses.candidates {
            let blocking = candidate
                .blocking_reason
                .as_ref()
                .map(|r| r.label())
                .unwrap_or_else(|| "-".into());
            lines.push(format!(
                "  {} need={} score={:.1} {} impact={:.2} cost={:.1} block={}",
                candidate.response_id.as_str(),
                candidate.need_id.as_str(),
                candidate.priority_score,
                candidate.availability.as_str(),
                candidate.expected_impact,
                candidate.estimated_cost,
                blocking
            ));
        }
        for diag in &responses.diagnostics {
            lines.push(format!("  diag: {diag}"));
        }
    } else {
        lines.push("Response candidates: (none — awaiting SA3 step)".to_string());
    }

    if let Some(plan) = world.settlement_intent_store().get(settlement_id) {
        lines.push(format!(
            "Settlement intent @ tick {} (from responses @ {}, needs @ {}):",
            plan.planned_tick, plan.source_response_tick, plan.source_need_tick
        ));
        if plan.intents.is_empty() {
            lines.push("  chosen: (none)".to_string());
        } else {
            lines.push("  chosen (priority order):".to_string());
            for intent in &plan.intents {
                lines.push(format!(
                    "    {} need={} resp={} pri={:.1} persist={} | {}",
                    intent.intent_id.as_str(),
                    intent.source_need.as_str(),
                    intent.chosen_response.as_str(),
                    intent.priority,
                    intent.desired_persistence.as_str(),
                    intent.reasoning
                ));
            }
        }
        if !plan.rejected.is_empty() {
            lines.push(format!("  rejected ({})", plan.rejected.len()));
            for rejected in plan.rejected.iter().take(12) {
                lines.push(format!(
                    "    {} need={} score={:.1} arb={:.1} ({})",
                    rejected.response_id.as_str(),
                    rejected.need_id.as_str(),
                    rejected.candidate_score,
                    rejected.arbitration_score,
                    rejected.reason.label()
                ));
            }
            if plan.rejected.len() > 12 {
                lines.push(format!(
                    "    … {} more",
                    plan.rejected.len().saturating_sub(12)
                ));
            }
        }
        for diag in &plan.diagnostics {
            lines.push(format!("  diag: {diag}"));
        }
    } else {
        lines.push("Settlement intent: (none — awaiting SA4 step)".to_string());
    }

    if let Some(report) = world.building_intent_propagation_store().get(settlement_id) {
        lines.push(format!(
            "Building intent propagation @ tick {} (from intent @ {}):",
            report.propagated_tick, report.source_intent_tick
        ));
        if report.assignments.is_empty() {
            lines.push("  assignments: (none)".to_string());
        } else {
            lines.push("  assignments:".to_string());
            for a in &report.assignments {
                let op = a
                    .selected_operation
                    .as_ref()
                    .map(|o| o.as_str())
                    .unwrap_or("-");
                lines.push(format!(
                    "    building#{} op={} enabled={} pri={} resp={} | {}",
                    a.building_id.raw(),
                    op,
                    a.enabled,
                    a.priority,
                    a.response_id.as_str(),
                    a.reason
                ));
            }
        }
        if !report.ignored_buildings.is_empty() {
            lines.push(format!("  ignored ({})", report.ignored_buildings.len()));
            for ignored in report.ignored_buildings.iter().take(8) {
                lines.push(format!(
                    "    building#{} resp={} ({})",
                    ignored.building_id.raw(),
                    ignored.response_id.as_str(),
                    ignored.reason
                ));
            }
        }
        for deferred in &report.deferred_intents {
            lines.push(format!("  deferred: {deferred}"));
        }
        for diag in &report.diagnostics {
            lines.push(format!("  diag: {diag}"));
        }
    } else {
        lines.push("Building intent propagation: (none — awaiting SA5 step)".to_string());
    }

    let construction_plans: Vec<_> = world
        .construction_plan_store()
        .plans_for_settlement(settlement_id);
    if !construction_plans.is_empty() {
        lines.push(format!(
            "Construction plans ({}):",
            construction_plans.len()
        ));
        for plan in construction_plans {
            lines.push(format!(
                "  plan#{} status={} def={} cap={} pri={:.1} reserved={:?} block={:?} key={}",
                plan.id.raw(),
                plan.status.as_str(),
                plan.building_definition_id.as_str(),
                plan.required_capability,
                plan.priority,
                plan.reserved_building_id.map(|id| id.raw()),
                plan.blocking_reason,
                plan.fulfillment_key
            ));
            if !plan.required_materials.is_empty() {
                for mat in &plan.required_materials {
                    lines.push(format!(
                        "    material {} req={} del={} miss={}",
                        mat.item_id.as_str(),
                        mat.required,
                        mat.delivered,
                        mat.missing()
                    ));
                }
            }
            if let Some(site) = &plan.placement {
                lines.push(format!(
                    "    site soft={} yaw_q={} ({},{}) local=({:.1},{:.1},{:.1})",
                    site.soft_score,
                    site.yaw_quadrants,
                    site.chunk_x,
                    site.chunk_z,
                    site.local_x,
                    site.local_y,
                    site.local_z
                ));
            }
            for diag in plan.diagnostics.iter().take(4) {
                lines.push(format!("    plan_diag: {diag}"));
            }
        }
    }
    if let Some(report) = world.construction_planning_report_store().get(settlement_id) {
        lines.push(format!(
            "Construction planning @ tick {}:",
            report.planned_tick
        ));
        for note in &report.capacity_notes {
            lines.push(format!("  capacity: {note}"));
        }
        for cand in report.considered_buildings.iter().take(6) {
            lines.push(format!(
                "  candidate {} score={}",
                cand.building_definition_id.as_str(),
                cand.score
            ));
        }
        for rej in report.rejected_sites.iter().take(4) {
            lines.push(format!(
                "  rejected_site ({:.1},{:.1}): {}",
                rej.offset_x, rej.offset_z, rej.reason
            ));
        }
        for diag in &report.diagnostics {
            lines.push(format!("  diag: {diag}"));
        }
    }

    if let Some(report) = world.strategic_task_generation_store().get(settlement_id) {
        lines.push(format!(
            "Strategic tasks @ tick {} (from intent @ {}):",
            report.generated_tick, report.source_intent_tick
        ));
        if report.emissions.is_empty() {
            lines.push("  generated: (none)".to_string());
        } else {
            lines.push("  generated:".to_string());
            for e in &report.emissions {
                let task_state = world
                    .task_store()
                    .get(e.task_id)
                    .map(|t| format!("{:?}", t.state))
                    .unwrap_or_else(|| "missing".into());
                lines.push(format!(
                    "    task#{} {:?} pri={:?} state={} intent={} resp={} tpl={} | {}",
                    e.task_id.raw(),
                    e.task_type,
                    e.priority,
                    task_state,
                    e.intent_id,
                    e.response_id,
                    e.template_id,
                    e.reason
                ));
            }
        }
        if !report.cancelled_task_ids.is_empty() {
            lines.push(format!(
                "  cancelled: {}",
                report
                    .cancelled_task_ids
                    .iter()
                    .map(|id| format!("#{}", id.raw()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        for diag in &report.diagnostics {
            lines.push(format!("  diag: {diag}"));
        }
    } else {
        lines.push("Strategic tasks: (none — awaiting SA6 step)".to_string());
    }

    {
        let report = &world.worker_assignment_store().last_report;
        if report.generated_tick > 0 {
            lines.push(format!(
                "Worker assignment @ tick {} (idle={} listings={}):",
                report.generated_tick, report.idle_workers, report.open_listings
            ));
            for a in report.assignments.iter().take(12) {
                lines.push(format!(
                    "  unit#{} → task#{:?} score={:.1} pri={:?} preempt={} | {}",
                    a.unit_id.raw(),
                    a.task_id.map(|id| id.raw()),
                    a.score,
                    a.priority,
                    a.preempted,
                    a.reason
                ));
            }
            for e in report.evaluations.iter().take(8) {
                lines.push(format!(
                    "  eval unit#{} chosen={:?} score={:.1} cands={} idle={} | {}",
                    e.unit_id.raw(),
                    e.chosen_task_id.map(|id| id.raw()),
                    e.chosen_score,
                    e.candidate_count,
                    e.idle,
                    e.notes
                ));
                for c in e.top_candidates.iter().take(3) {
                    lines.push(format!("    cand: {c}"));
                }
                if let Some(point) = &e.reservation_point {
                    lines.push(format!("    reservation: {point}"));
                }
            }
            for diag in &report.diagnostics {
                lines.push(format!("  diag: {diag}"));
            }
        } else {
            lines.push("Worker assignment: (none — awaiting SA7 step)".to_string());
        }
    }

    let Some(planner) = world.production_planner_store().get(settlement_id) else {
        lines.push("Production planner: (none)".to_string());
        return Some(lines.join("\n"));
    };
    let diagnostics = &planner.last_diagnostics;
    lines.push(format!(
        "Production planner: enabled={}",
        planner.enabled
    ));
    lines.push(format!("Stock goals: {}", planner.stock_goals.len()));
    for entry in &diagnostics.stock_entries {
        lines.push(format!(
            "  {} current={} desired={} demand={}",
            entry.item_id.as_str(),
            entry.current_stock,
            entry.desired_stock,
            entry.demand
        ));
    }
    if !diagnostics.chosen_producers.is_empty() {
        lines.push("Chosen producers:".to_string());
        for decision in &diagnostics.chosen_producers {
            lines.push(format!(
                "  #{} op={} enabled={} ({})",
                decision.building_id.raw(),
                decision.operation_id.as_str(),
                decision.enabled,
                decision.reason
            ));
        }
    }
    if !diagnostics.shortages.is_empty() {
        lines.push("Shortages:".to_string());
        for (item, kind) in &diagnostics.shortages {
            lines.push(format!("  {} {:?}", item.as_str(), kind));
        }
    }
    if !diagnostics.blocked_chains.is_empty() {
        lines.push(format!("Blocked: {}", diagnostics.blocked_chains.join("; ")));
    }
    Some(lines.join("\n"))
}

fn format_hauling_requests_for_building(
    world: &WorldData,
    building_id: crate::world::BuildingId,
) -> String {
    let store = world.hauling_request_store();
    let reservation_store = world.inventory_reservation_store();
    let request_ids = store.requests_for_building(building_id);
    if request_ids.is_empty() {
        return "no hauling requests".to_string();
    }
    request_ids
        .iter()
        .filter_map(|request_id| store.get(*request_id))
        .map(|request| {
            let reservation = reservation_store
                .request_record(request.id)
                .map(|record| {
                    format!(
                        "src={:?} dst_cap={:?}",
                        record.source.map(|source| source.quantity),
                        record.destination.map(|dest| dest.quantity)
                    )
                })
                .unwrap_or_else(|| "none".to_string());
            format!(
                "#{} {} x{} rem={} {}→{} status={} phase={} worker={:?} block={:?} res={}",
                request.id.raw(),
                request.item_id.as_str(),
                request.quantity,
                request.remaining_quantity,
                request.source_inventory_id.raw(),
                request.destination_inventory_id.raw(),
                request.status.label(),
                request.execution_phase.label(),
                request.assigned_unit_id,
                request
                    .blocking_reason
                    .as_ref()
                    .map(|reason| reason.clone().label()),
                reservation
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Optional production probe fields for dev inspector (EP1).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BuildingOperationProbe {
    pub terrain_output_rate: Option<String>,
    pub final_output_rate: Option<String>,
    pub operation_progress: Option<String>,
    pub operation_completions: Option<u32>,
    pub operation_limiting_factor: Option<String>,
    pub lifecycle: Option<String>,
    pub selected_operation: Option<String>,
    pub policy_enabled: Option<bool>,
    pub policy_paused: Option<bool>,
    pub repeat_mode: Option<String>,
    pub control_source: Option<String>,
    pub priority: Option<u8>,
    pub assigned_workers: Option<String>,
    pub blocking_reason: Option<String>,
    pub active_worker_count: Option<u32>,
    pub remaining_repeat_count: Option<u32>,
    pub last_efficiency_revision: Option<u64>,
    pub supported_operations: Option<String>,
    pub default_operation: Option<String>,
    pub operation_category: Option<String>,
    pub base_labor: Option<u32>,
    pub max_workers: Option<u32>,
    pub validation_state: Option<String>,
    pub execution_inputs_summary: Option<String>,
    pub execution_outputs_summary: Option<String>,
    pub execution_inventory_summary: Option<String>,
    pub execution_blocking: Option<String>,
    pub terrain_assessment_summary: Option<String>,
    pub terrain_assessment_revision: Option<u64>,
    pub terrain_assessment_stale: Option<bool>,
}

/// Probe authoritative operational efficiency and progress for dev inspector (EP2).
pub fn probe_building_operation(
    world: &WorldData,
    building_catalog: &BuildingCatalog,
    operation: &mut crate::world::BuildingOperationParams<'_>,
    building_id: BuildingId,
) -> BuildingOperationProbe {
    let mut probe = BuildingOperationProbe::default();
    if !world
        .get_building(building_id)
        .is_some_and(is_building_operational)
    {
        return probe;
    }
    let selected_operation = world
        .building_production_store()
        .get_policy(building_id)
        .and_then(|policy| policy.selected_operation.as_ref())
        .and_then(|id| operation.operation_catalog.get(id));
    let mut ctx = operation.efficiency_context(world, building_catalog);
    if let Ok(report) = crate::world::building_operational_efficiency(
        &mut ctx,
        building_id,
        selected_operation,
    ) {
        probe.terrain_output_rate = Some(crate::world::format_efficiency_display(
            report.terrain_efficiency_basis_points,
        ));
        probe.final_output_rate = Some(crate::world::format_efficiency_display(
            report.final_output_efficiency_basis_points,
        ));
        if report.limiting_factor != crate::world::OperationalLimitingFactor::None {
            probe.operation_limiting_factor = Some(report.limiting_factor.label().to_string());
        }
        probe.terrain_assessment_revision = Some(report.assessment_revision);
    }
    if let Some(assessment) = operation.assessment_store.get(building_id) {
        probe.terrain_assessment_stale = Some(assessment.stale);
        probe.terrain_assessment_summary = Some(
            assessment
                .per_requirement
                .iter()
                .map(|req| {
                    format!(
                        "{} avg={} eff={} can_operate={}",
                        req.field_id,
                        crate::world::format_field_average_display(req.average_value),
                        crate::world::format_efficiency_display(
                            req.response_efficiency_basis_points,
                        ),
                        req.can_operate
                    )
                })
                .collect::<Vec<_>>()
                .join("; "),
        );
    }
    let production = world.building_production_store();
    if let Some(state) = production.get_state(building_id) {
        let pct = state.progress.value() as f32 / crate::world::PRODUCTION_PROGRESS_ONE_UNIT as f32
            * 100.0;
        probe.operation_progress = Some(format!("{pct:.1}%"));
        probe.operation_completions = Some(state.completion_count);
        probe.lifecycle = Some(state.lifecycle.label().to_string());
        probe.active_worker_count = Some(state.active_worker_count);
        probe.last_efficiency_revision = Some(state.last_efficiency_revision);
        probe.blocking_reason = state
            .blocked_reason
            .as_ref()
            .map(|reason| reason.label().to_string());
    }
    if let Some(policy) = production.get_policy(building_id) {
        probe.selected_operation = policy
            .selected_operation
            .as_ref()
            .map(|id| id.to_string());
        probe.policy_enabled = Some(policy.enabled);
        probe.policy_paused = Some(policy.paused);
        probe.repeat_mode = Some(policy.repeat_mode.display_label());
        probe.control_source = Some(policy.control_source.label().to_string());
        probe.priority = Some(policy.priority);
        probe.remaining_repeat_count =
            policy.repeat_mode.remaining_repeats(production.get_state(building_id).map(|s| s.completion_count).unwrap_or(0));
    }
    let workers = crate::world::workstation_workers_for_building(
        world, building_id,
    );
    if !workers.is_empty() {
        probe.assigned_workers = Some(
            workers
                .iter()
                .map(|id| id.raw().to_string())
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    if let Some(record) = world.get_building(building_id) {
        if let Some(definition) = building_catalog.get(&record.definition_id) {
            if !definition.supported_operations.is_empty() {
                probe.supported_operations = Some(
                    definition
                        .supported_operations
                        .iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>()
                        .join(", "),
                );
            }
            probe.default_operation = definition
                .resolved_default_operation()
                .map(|id| id.to_string());
            if let Some(selected) = production
                .get_policy(building_id)
                .and_then(|policy| policy.selected_operation.as_ref())
            {
                if let Some(op_def) = operation.operation_catalog.get(selected) {
                    probe.operation_category = Some(op_def.category.label().to_string());
                    probe.base_labor = Some(op_def.base_labor);
                    probe.max_workers = Some(op_def.max_workers);
                    probe.validation_state = Some(
                        crate::world::validate_operation_selection(
                            definition,
                            building_id,
                            operation.operation_catalog,
                            selected,
                        )
                        .map(|_| "OK".to_string())
                        .unwrap_or_else(|err| err.to_string()),
                    );
                    let assessment = crate::world::assess_production_execution(
                        world,
                        operation.inventory_ctx,
                        building_id,
                        op_def,
                        definition,
                    );
                if !assessment.inputs.is_empty() {
                    probe.execution_inputs_summary = Some(
                        assessment
                            .inputs
                            .iter()
                            .map(|input| {
                                format!(
                                    "{}: avail={}/{} phys={} res={} {}",
                                    input.binding_id,
                                    input.available,
                                    input.required,
                                    input.physical,
                                    input.reserved,
                                    input.item_id.as_str()
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("; "),
                    );
                }
                if !assessment.outputs.is_empty() {
                    probe.execution_outputs_summary = Some(
                        assessment
                            .outputs
                            .iter()
                            .map(|output| {
                                format!(
                                    "{}: {} {} ({})",
                                    output.binding_id,
                                    output.quantity,
                                    output.item_id.as_str(),
                                    if output.can_accept { "ok" } else { "full" }
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("; "),
                    );
                }
                probe.execution_inventory_summary = Some(
                    world
                        .building_inventory_binding_store()
                        .get(building_id)
                        .map(|set| {
                            set.bindings()
                                .iter()
                                .map(|binding| {
                                    let entries = world
                                        .inventory_store()
                                        .get(binding.inventory_id)
                                        .map(|record| {
                                            record
                                                .placed_entries()
                                                .iter()
                                                .filter_map(|entry| match &entry.contents {
                                                    crate::world::InventoryEntryContents::Stack {
                                                        item_definition_id,
                                                        quantity,
                                                    } => Some(format!(
                                                        "{}x{quantity}",
                                                        item_definition_id.as_str()
                                                    )),
                                                    _ => None,
                                                })
                                                .collect::<Vec<_>>()
                                                .join(",")
                                        })
                                        .unwrap_or_else(|| "missing".to_string());
                                    format!("{}={entries}", binding.binding_id)
                                })
                                .collect::<Vec<_>>()
                                .join("; ")
                        })
                        .unwrap_or_else(|| "no bindings".to_string()),
                );
                probe.execution_blocking = assessment.blocking_label().map(str::to_string);
                }
            } else {
                probe.validation_state = Some("No operation selected".into());
            }
        }
    }
    probe
}

/// Capture runtime building asset presentation for dev inspector (ADR-095 BA1).
pub fn capture_building_asset_presentation(
    building_id: BuildingId,
    world: &WorldData,
    catalog: &BuildingCatalog,
    asset_server: &AssetServer,
    scene_assets: &crate::buildings::BuildingSceneAssets,
    render_index: &crate::buildings::BuildingRenderIndex,
    render_entities: &Query<(
        Entity,
        &crate::buildings::BuildingRenderEntity,
        Option<&crate::buildings::BuildingDiagnosticFallback>,
        Option<&crate::buildings::BuildingSceneTags>,
    )>,
) -> BuildingAssetPresentationInfo {
    let Some(record) = world.get_building(building_id) else {
        return BuildingAssetPresentationInfo::default();
    };
    let desired_render_key = catalog.get(&record.definition_id).and_then(|definition| {
        crate::buildings::lifecycle_render_key(definition, record.lifecycle_state)
    });
    let resolved_asset_path = desired_render_key
        .as_ref()
        .map(|key| format!("assets/buildings/{key}.glb"));
    let asset_load_state = desired_render_key.as_ref().and_then(|key| {
        scene_assets
            .scene_for_key(key)
            .and_then(|scene| asset_server.get_load_state(scene))
            .map(|state| format!("{state:?}"))
    });
    let runtime_entity = render_index
        .0
        .get(&building_id)
        .map(|entity| entity.to_bits());
    let mut info = BuildingAssetPresentationInfo {
        desired_render_key,
        resolved_asset_path,
        asset_load_state,
        runtime_entity,
        ..Default::default()
    };
    let Some(entity) = render_index.0.get(&building_id) else {
        return info;
    };
    if let Ok((_, marker, fallback, tags)) = render_entities.get(*entity) {
        info.uses_diagnostic_fallback = marker.uses_diagnostic_fallback;
        info.fallback_reason = fallback.map(|value| value.reason.label().to_string());
        if let Some(tags) = tags {
            info.space_tag_count = Some(tags.space_node_names.len() as u32);
            info.roof_tag_count = Some(tags.roof_entities.len() as u32);
        }
    }
    info
}

/// Capture interaction classification at a world click (U6 + U-UI5).
pub fn capture_interaction_inspector_snapshot(
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    weapon_catalog: &crate::world::WeaponCatalog,
    click_position: WorldPosition,
) -> Option<InteractionInspectorSnapshot> {
    let interaction_catalog = crate::world::BuildingInteractionProfileCatalog::default();
    let ctx = InteractionQueryContext::new(
        world,
        doodad_catalog,
        building_catalog,
        footprint_catalog,
        &interaction_catalog,
        unit_catalog,
        weapon_catalog,
    );
    let terrain_hit = ground_world_position(world, click_position).is_some();
    let interaction = query_world_interaction(&ctx, click_position)?;
    let plan = resolve_interaction_to_order(&interaction);
    let order = interaction_plan_to_unit_order(plan);

    let doodad_hit = match &interaction.target {
        crate::world::InteractionTargetRef::Doodad(doodad_id) => world
            .get_doodad(*doodad_id)
            .map(|record| record.definition_id.clone()),
        _ => None,
    };

    Some(InteractionInspectorSnapshot {
        click_position,
        terrain_hit,
        doodad_hit,
        interaction_type: format!("{:?}", interaction.interaction_type),
        resolved_command: order.as_ref().map(|o| format!("{o:?}")),
        resolved_order: order,
    })
}

fn state_label(state: &UnitState) -> String {
    match state {
        UnitState::Idle => "Idle".into(),
        UnitState::Moving { .. } => "Moving".into(),
        UnitState::Working { task_id } => format!("Working({task_id:?})"),
        UnitState::Dead => "Dead".into(),
    }
}

fn capture_combat_inspector(
    record: &crate::world::UnitRecord,
    unit_catalog: &UnitCatalog,
    weapon_catalog: &WeaponCatalog,
) -> CombatInspectorSnapshot {
    CombatInspectorSnapshot {
        weapon_name: weapon_display_for_unit(record, unit_catalog, weapon_catalog).map(|w| w.name),
        target_unit_id: combat_target_id(&record.combat_state),
        attack_phase: record.attack_cycle.as_ref().map(attack_cycle_summary),
    }
}

fn capture_projectiles_for_unit(
    world: &WorldData,
    unit_id: UnitId,
) -> Vec<ProjectileInspectorSnapshot> {
    world
        .sorted_projectile_ids()
        .into_iter()
        .filter_map(|id| world.get_projectile(id))
        .filter(|record| record.source_unit_id == unit_id)
        .map(projectile_inspector_from_record)
        .collect()
}

#[cfg(test)]
fn capture_projectile_inspector_snapshot(
    world: &WorldData,
    projectile_id: crate::world::ProjectileId,
) -> Option<ProjectileInspectorSnapshot> {
    world
        .get_projectile(projectile_id)
        .map(projectile_inspector_from_record)
}

fn projectile_inspector_from_record(
    record: &crate::world::ProjectileRecord,
) -> ProjectileInspectorSnapshot {
    ProjectileInspectorSnapshot {
        projectile_id: record.id,
        source_unit_id: record.source_unit_id,
        target_unit_id: record.target_unit_id,
        weapon_id: record.weapon_id.as_str().to_string(),
        position: record.position,
        speed_mps: record.speed_mps,
        status: projectile_status_label(record.status).to_string(),
    }
}

fn projectile_status_label(status: crate::world::ProjectileStatus) -> &'static str {
    match status {
        crate::world::ProjectileStatus::InFlight => "InFlight",
        crate::world::ProjectileStatus::Hit => "Hit",
        crate::world::ProjectileStatus::Expired => "Expired",
        crate::world::ProjectileStatus::Invalidated => "Invalidated",
    }
}

fn capture_path_inspector(
    state: &UnitState,
    unit_position: WorldPosition,
    layout: crate::world::ChunkLayout,
) -> PathInspectorSnapshot {
    let UnitState::Moving {
        path,
        waypoint_index,
        ..
    } = state
    else {
        return PathInspectorSnapshot::default();
    };

    let (segment_start, segment_end) = active_segment(*waypoint_index, unit_position, path);
    PathInspectorSnapshot {
        waypoints: path
            .waypoints
            .iter()
            .map(|waypoint| waypoint.position)
            .collect(),
        waypoint_index: *waypoint_index,
        segment_start: segment_start.map(|waypoint| waypoint.position),
        segment_end: segment_end.map(|waypoint| waypoint.position),
        length_meters: path.length_meters(layout),
        chunk_transitions: chunk_transitions_along_path(path),
    }
}

fn active_segment(
    waypoint_index: usize,
    unit_position: WorldPosition,
    path: &NavigationPath,
) -> (Option<NavigationWaypoint>, Option<NavigationWaypoint>) {
    let start = if waypoint_index == 0 {
        Some(NavigationWaypoint::surface(unit_position))
    } else {
        path.waypoints
            .get(waypoint_index.saturating_sub(1))
            .copied()
    };
    let end = path.waypoints.get(waypoint_index).copied();
    (start, end)
}

fn chunk_transitions_along_path(path: &NavigationPath) -> Vec<ChunkCoord> {
    let mut chunks = Vec::new();
    let mut last: Option<ChunkCoord> = None;
    for waypoint in &path.waypoints {
        if last != Some(waypoint.position.chunk) {
            chunks.push(waypoint.position.chunk);
            last = Some(waypoint.position.chunk);
        }
    }
    chunks
}

fn capture_formation_inspector(
    world: &WorldData,
    unit_id: UnitId,
    state: &UnitState,
    layout: crate::world::ChunkLayout,
    collision_radius: f32,
) -> FormationInspectorSnapshot {
    let UnitState::Moving { target, .. } = state else {
        return FormationInspectorSnapshot {
            spacing_meters: unit_spacing_meters(collision_radius),
            ..Default::default()
        };
    };

    let unit_global = world
        .get_unit(unit_id)
        .map(|r| r.placement.position.to_global(layout))
        .unwrap_or(Vec3::ZERO);
    let target_global = target.to_global(layout);
    let offset_xz = Vec2::new(
        target_global.x - unit_global.x,
        target_global.z - unit_global.z,
    );

    let mut peers: Vec<UnitId> = world
        .sorted_unit_ids()
        .into_iter()
        .filter(|id| {
            world
                .get_unit(*id)
                .and_then(|record| match &record.state {
                    UnitState::Moving {
                        target: peer_target,
                        ..
                    } => Some(positions_close(*peer_target, *target, layout)),
                    _ => None,
                })
                .unwrap_or(false)
        })
        .collect();
    peers.sort_unstable();
    let slot_index = peers.iter().position(|id| *id == unit_id);

    FormationInspectorSnapshot {
        slot_index,
        offset_xz,
        target: Some(*target),
        spacing_meters: unit_spacing_meters(collision_radius),
        peers_sharing_target: peers.len() as u32,
    }
}

fn positions_close(a: WorldPosition, b: WorldPosition, layout: crate::world::ChunkLayout) -> bool {
    let ga = a.to_global(layout);
    let gb = b.to_global(layout);
    Vec2::new(ga.x - gb.x, ga.z - gb.z).length() <= FORMATION_TARGET_EPSILON
}

fn capture_steering_inspector(
    world: &WorldData,
    unit_catalog: &UnitCatalog,
    unit_id: UnitId,
    position: WorldPosition,
    state: &UnitState,
    collision_radius: f32,
    layout: crate::world::ChunkLayout,
) -> SteeringInspectorSnapshot {
    let UnitState::Moving {
        target,
        path,
        waypoint_index,
        ..
    } = state
    else {
        return SteeringInspectorSnapshot::default();
    };

    let path_direction = path_direction_xz(position, path, *waypoint_index, layout);
    let global = position.to_global(layout);
    let position_xz = Vec2::new(global.x, global.z);
    let target_global = target.to_global(layout);
    let formation_target_xz = Vec2::new(target_global.x, target_global.z);

    let neighbors = gather_steering_neighbors(
        world,
        unit_catalog,
        unit_id,
        position,
        STEERING_SETTINGS.neighbor_query_radius,
    );

    let separation = separation_force(
        position_xz,
        collision_radius,
        &neighbors,
        &STEERING_SETTINGS,
    );
    let cohesion = cohesion_force(
        position_xz,
        Some(formation_target_xz),
        &neighbors,
        &STEERING_SETTINGS,
    );
    let alignment = alignment_force(&neighbors, &STEERING_SETTINGS);

    let context = SteeringContext {
        unit_id,
        position_xz,
        path_direction_xz: path_direction,
        collision_radius,
        formation_target_xz: Some(formation_target_xz),
        neighbors: neighbors.clone(),
        delta_seconds: 1.0 / 60.0,
        settings: STEERING_SETTINGS,
    };
    let final_direction = context.steered_direction_xz();

    SteeringInspectorSnapshot {
        separation,
        cohesion,
        alignment,
        final_direction,
        neighbor_count: neighbors.len() as u32,
        path_direction,
    }
}

fn path_direction_xz(
    position: WorldPosition,
    path: &NavigationPath,
    waypoint_index: usize,
    layout: crate::world::ChunkLayout,
) -> Vec2 {
    let Some(waypoint) = path.waypoints.get(waypoint_index).copied() else {
        return Vec2::ZERO;
    };
    let current = position.to_global(layout);
    let waypoint_global = waypoint.position.to_global(layout);
    let delta = Vec2::new(waypoint_global.x - current.x, waypoint_global.z - current.z);
    if delta.length_squared() <= 1e-8 {
        return Vec2::ZERO;
    }
    delta.normalize()
}

fn diagnose_block_reason(
    world: &WorldData,
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
    footprint_catalog: &FootprintCatalog,
    position: WorldPosition,
    radius: f32,
) -> Option<String> {
    if ground_world_position(world, position).is_none() {
        return Some("Missing terrain / chunk not resident".into());
    }
    if let Some(doodad_id) = blocking_doodad_at_position(
        world,
        doodad_catalog,
        building_catalog,
        footprint_catalog,
        position,
        radius,
    ) {
        return Some(format!("Blocked by doodad #{}", doodad_id.raw()));
    }
    match classify_slope_walkability(world, position, 40.0) {
        SlopeWalkability::Walkable => None,
        SlopeWalkability::TooSteep => Some("Unwalkable slope".into()),
        SlopeWalkability::Unavailable => Some("Terrain slope unavailable".into()),
    }
}

fn capture_chunk_residency(world: &WorldData, unit_id: UnitId) -> Option<ChunkResidencySnapshot> {
    let chunk_id = world.unit_chunk(unit_id)?;
    let coord = chunk_id.coord();
    let terrain_loaded = world.is_chunk_loaded(chunk_id);
    let doodads_in_chunk = world
        .doodads_in_chunk(chunk_id)
        .map(|store| store.len() as u32)
        .unwrap_or(0);
    let units_in_chunk = world
        .units_in_chunk(chunk_id)
        .map(|store| store.len() as u32)
        .unwrap_or(0);
    Some(ChunkResidencySnapshot {
        unit_chunk: coord,
        terrain_loaded,
        doodads_in_chunk,
        units_in_chunk,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadCatalog, Heightfield, LocalPosition,
        NavigationPath, UnitDefinitionId, UnitOrder, UnitSource, UnitState, create_unit,
    };

    fn flat_chunk() -> ChunkData {
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        ChunkData::new(heightfield, Vec::new())
    }

    fn insert_flat(world: &mut WorldData) {
        let chunk = ChunkId::new(ChunkCoord::new(0, 0));
        world.insert(chunk, flat_chunk());
    }

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn spawn_wolf(world: &mut WorldData, catalog: &UnitCatalog, position: WorldPosition) -> UnitId {
        create_unit(
            catalog,
            world,
            &UnitDefinitionId::new("wolf"),
            position,
            UnitSource::Authored,
        )
        .unwrap()
        .id
    }

    #[test]
    fn inspector_returns_correct_unit_state_snapshot() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world);
        let unit_id = spawn_wolf(&mut world, &catalog, pos(1.0, 2.0));
        world
            .set_unit_state(
                unit_id,
                UnitState::Moving {
                    target: pos(10.0, 10.0),
                    path: NavigationPath::from_surface_positions(vec![
                        pos(5.0, 5.0),
                        pos(10.0, 10.0),
                    ]),
                    waypoint_index: 0,
                },
            )
            .unwrap();

        let snap = capture_unit_inspector_snapshot(
            &world,
            &catalog,
            &crate::world::WeaponCatalog::default(),
            &DoodadCatalog::default(),
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            unit_id,
            7,
            None,
        )
        .unwrap();
        assert_eq!(snap.unit_id, unit_id);
        assert_eq!(snap.state_label, "Moving");
        assert_eq!(snap.path.waypoints.len(), 2);
        assert_eq!(snap.simulation_tick, 7);
    }

    #[test]
    fn path_inspection_matches_world_data_path() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world);
        let unit_id = spawn_wolf(&mut world, &catalog, pos(0.0, 0.0));
        let waypoints = vec![pos(20.0, 0.0), pos(20.0, 20.0)];
        world
            .set_unit_state(
                unit_id,
                UnitState::Moving {
                    target: pos(20.0, 20.0),
                    path: NavigationPath::from_surface_positions(waypoints.clone()),
                    waypoint_index: 1,
                },
            )
            .unwrap();

        let snap = capture_unit_inspector_snapshot(
            &world,
            &catalog,
            &crate::world::WeaponCatalog::default(),
            &DoodadCatalog::default(),
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            unit_id,
            0,
            None,
        )
        .unwrap();
        assert_eq!(snap.path.waypoints, waypoints);
        assert_eq!(snap.path.waypoint_index, 1);
        assert!(snap.path.length_meters > 0.0);
    }

    #[test]
    fn steering_values_match_simulation_output() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world);
        let unit_id = spawn_wolf(&mut world, &catalog, pos(0.0, 0.0));
        world
            .set_unit_state(
                unit_id,
                UnitState::Moving {
                    target: pos(10.0, 0.0),
                    path: NavigationPath::from_surface_positions(vec![pos(10.0, 0.0)]),
                    waypoint_index: 0,
                },
            )
            .unwrap();

        let snap = capture_unit_inspector_snapshot(
            &world,
            &catalog,
            &crate::world::WeaponCatalog::default(),
            &DoodadCatalog::default(),
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            unit_id,
            0,
            None,
        )
        .unwrap();
        assert!(snap.steering.path_direction.length() > 0.0);
        assert_eq!(snap.steering.neighbor_count, 0);
    }

    #[test]
    fn formation_inspector_matches_moving_target() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world);
        let unit_id = spawn_wolf(&mut world, &catalog, pos(0.0, 0.0));
        let target = pos(8.0, 0.0);
        world
            .set_unit_state(
                unit_id,
                UnitState::Moving {
                    target,
                    path: NavigationPath::from_surface_positions(vec![target]),
                    waypoint_index: 0,
                },
            )
            .unwrap();

        let snap = capture_unit_inspector_snapshot(
            &world,
            &catalog,
            &crate::world::WeaponCatalog::default(),
            &DoodadCatalog::default(),
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            unit_id,
            0,
            None,
        )
        .unwrap();
        assert_eq!(snap.formation.target, Some(target));
        assert!((snap.formation.offset_xz.x - 8.0).abs() < 0.01);
    }

    #[test]
    fn interaction_inspector_resolves_move_target() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world);
        let click = pos(64.0, 64.0);
        let snap = capture_interaction_inspector_snapshot(
            &world,
            &catalog,
            &DoodadCatalog::default(),
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            &crate::world::WeaponCatalog::default(),
            click,
        )
        .unwrap();
        assert!(snap.terrain_hit);
        assert!(snap.interaction_type.contains("MoveTarget"));
        assert!(matches!(
            snap.resolved_order,
            Some(UnitOrder::MoveTo { .. })
        ));
    }

    #[test]
    fn inspector_does_not_mutate_simulation_state() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world);
        let unit_id = spawn_wolf(&mut world, &catalog, pos(3.0, 4.0));
        let before = world.get_unit(unit_id).unwrap().clone();
        let _ = capture_unit_inspector_snapshot(
            &world,
            &catalog,
            &crate::world::WeaponCatalog::default(),
            &DoodadCatalog::default(),
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            unit_id,
            0,
            None,
        );
        assert_eq!(world.get_unit(unit_id).unwrap(), &before);
    }

    #[test]
    fn cached_snapshot_fields_remain_consistent_on_repeat_capture() {
        let catalog = UnitCatalog::default();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world);
        let unit_id = spawn_wolf(&mut world, &catalog, pos(1.0, 1.0));
        let a = capture_unit_inspector_snapshot(
            &world,
            &catalog,
            &crate::world::WeaponCatalog::default(),
            &DoodadCatalog::default(),
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            unit_id,
            3,
            None,
        )
        .unwrap();
        let b = capture_unit_inspector_snapshot(
            &world,
            &catalog,
            &crate::world::WeaponCatalog::default(),
            &DoodadCatalog::default(),
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            unit_id,
            3,
            None,
        )
        .unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn inspector_reads_weapon_and_combat_fields() {
        let catalog = UnitCatalog::default();
        let weapons = crate::world::WeaponCatalog::from_definitions(
            crate::world::starter_weapon_definitions(),
        )
        .unwrap();
        let mut world = WorldData::new(layout());
        insert_flat(&mut world);
        let unit_id = spawn_wolf(&mut world, &catalog, pos(1.0, 1.0));
        let snap = capture_unit_inspector_snapshot(
            &world,
            &catalog,
            &weapons,
            &DoodadCatalog::default(),
            &BuildingCatalog::default(),
            &FootprintCatalog::default(),
            unit_id,
            0,
            None,
        )
        .unwrap();
        assert_eq!(snap.combat.weapon_name.as_deref(), Some("Wolf Bite"));
        assert!(snap.combat.attack_phase.is_none());
    }

    #[test]
    fn projectile_inspector_reads_projectile_record_only() {
        use crate::world::{
            DamageType, ProjectileId, ProjectileLaunchSnapshot, ProjectileRecord,
            WeaponDefinitionId,
        };
        let mut world = WorldData::new(layout());
        insert_flat(&mut world);
        let record = ProjectileRecord::new_in_flight(
            ProjectileId::new(1),
            UnitId::new(1),
            UnitId::new(2),
            WeaponDefinitionId::new("weapon_bow"),
            5.0,
            DamageType::Piercing,
            pos(0.0, 0.0),
            pos(10.0, 0.0),
            20.0,
            ProjectileLaunchSnapshot::render_test_placeholder(UnitId::new(1)),
        );
        world.insert_projectile(record.clone());
        let snap = capture_projectile_inspector_snapshot(&world, ProjectileId::new(1)).unwrap();
        assert_eq!(snap.projectile_id, ProjectileId::new(1));
        assert_eq!(snap.speed_mps, 20.0);
        assert_eq!(snap.status, "InFlight");
    }
}
