//! Emergency evaluation / reweighting tests (SA8).

use bevy::prelude::{Quat, Vec3};

use super::*;
use crate::world::building::catalog::BuildingCatalog;
use crate::world::inventory::InventoryCatalogCtx;
use crate::world::settlement::needs::{evaluate_settlement_needs_now, NeedCatalog};
use crate::world::settlement::response::{
    discover_settlement_responses_now, ResponseCatalog,
};
use crate::world::settlement::state::SettlementKind;
use crate::world::settlement::{
    create_settlement_with_treasury, reconcile_settlement_building_membership,
    ActiveEmergencyInstance, SettlementOwnership,
};
use crate::world::{
    may_preempt_with_override, sync_construction_tasks, TaskPriority, PreemptPolicyOverride,
};
use crate::world::{
    Affiliation, BuildingCategoryCatalog, BuildingDefinitionId, BuildingLifecycleState,
    BuildingOwnership, BuildingSource, ChunkCoord, ChunkExtent, LocalPosition, WorldData,
    WorldPosition, create_building_with_inventory, starter_building_definitions,
    starter_inventory_profile_definitions, starter_item_category_definitions,
    starter_item_definitions,
};

fn flat_world() -> WorldData {
    let layout = crate::world::WorldConfig::default().chunk_layout();
    let mut world = WorldData::new(layout);
    world.set_authored_extent(ChunkExtent {
        min: ChunkCoord::new(0, 0),
        max: ChunkCoord::new(1, 1),
    });
    world
}

fn inventory_ctx() -> &'static InventoryCatalogCtx<'static> {
    static CTX: std::sync::OnceLock<InventoryCatalogCtx<'static>> = std::sync::OnceLock::new();
    CTX.get_or_init(|| {
        let categories =
            crate::world::ItemCategoryCatalog::from_definitions(starter_item_category_definitions())
                .unwrap();
        let items =
            crate::world::ItemCatalog::from_definitions(starter_item_definitions(), &categories)
                .unwrap();
        let profiles = crate::world::InventoryProfileCatalog::from_definitions(
            starter_inventory_profile_definitions(),
        )
        .unwrap();
        InventoryCatalogCtx::new(
            Box::leak(Box::new(items)),
            Box::leak(Box::new(categories)),
            Box::leak(Box::new(profiles)),
        )
    })
}

fn pos(x: f32, z: f32) -> WorldPosition {
    WorldPosition::new(
        ChunkCoord::new(0, 0),
        LocalPosition::new(Vec3::new(x, 0.0, z)),
    )
}

fn setup_settlement() -> (WorldData, crate::world::SettlementId, BuildingCatalog) {
    let mut world = flat_world();
    let categories = BuildingCategoryCatalog::default();
    let building_catalog =
        BuildingCatalog::from_definitions(starter_building_definitions(), &categories).unwrap();
    let interaction = crate::world::BuildingInteractionProfileCatalog::default();
    let ctx = inventory_ctx();
    let ownership = BuildingOwnership::with_affiliation(Affiliation::Player);
    let core = create_building_with_inventory(
        &building_catalog,
        &mut world,
        &BuildingDefinitionId::new("settlement_core"),
        pos(50.0, 50.0),
        Quat::IDENTITY,
        BuildingSource::Authored,
        ownership,
        None,
        ctx,
    )
    .unwrap()
    .id;
    world.mutate_building(core, |r| {
        r.lifecycle_state = BuildingLifecycleState::Complete;
    });
    let settlement = create_settlement_with_treasury(
        &mut world,
        &building_catalog,
        &interaction,
        core,
        "SA8",
        SettlementOwnership::player_default(),
        pos(50.0, 50.0),
        0,
    )
    .unwrap();
    reconcile_settlement_building_membership(&mut world);
    if let Some(state) = world
        .settlement_state_store_mut()
        .get_mut(settlement.settlement_id)
    {
        state.kind = SettlementKind::Town;
    }
    (world, settlement.settlement_id, building_catalog)
}

#[test]
fn starvation_activates_from_empty_food_reserves() {
    let (mut world, id, buildings) = setup_settlement();
    let catalog = EmergencyCatalog::default();
    let ctx = inventory_ctx();
    evaluate_settlement_emergencies_now(
        &mut world,
        &catalog,
        &buildings,
        ctx.items,
        ctx,
        id,
        10,
    );
    let state = world.settlement_state_store().get(id).unwrap();
    assert!(
        state.emergencies.instance("starvation").is_some(),
        "expected starvation; instances={:?}",
        state.emergencies.instances
    );
    assert!(state.emergencies.starvation);
}

#[test]
fn below_activation_threshold_does_not_activate_attack() {
    let (mut world, id, buildings) = setup_settlement();
    if let Some(state) = world.settlement_state_store_mut().get_mut(id) {
        state
            .extension_seams
            .insert("hostile_threat".into(), "0.2".into());
    }
    let catalog = EmergencyCatalog::default();
    let ctx = inventory_ctx();
    evaluate_settlement_emergencies_now(
        &mut world,
        &catalog,
        &buildings,
        ctx.items,
        ctx,
        id,
        10,
    );
    let state = world.settlement_state_store().get(id).unwrap();
    assert!(state.emergencies.instance("active_attack").is_none());
}

#[test]
fn hysteresis_prevents_rapid_toggle() {
    let (mut world, id, buildings) = setup_settlement();
    let catalog = EmergencyCatalog::default();
    let ctx = inventory_ctx();
    if let Some(state) = world.settlement_state_store_mut().get_mut(id) {
        state
            .extension_seams
            .insert("hostile_threat".into(), "0.9".into());
    }
    evaluate_settlement_emergencies_now(
        &mut world,
        &catalog,
        &buildings,
        ctx.items,
        ctx,
        id,
        10,
    );
    assert!(world
        .settlement_state_store()
        .get(id)
        .unwrap()
        .emergencies
        .instance("active_attack")
        .is_some());

    // Drop below activation but still above deactivation — stay active (and min duration).
    if let Some(state) = world.settlement_state_store_mut().get_mut(id) {
        state
            .extension_seams
            .insert("hostile_threat".into(), "0.4".into());
    }
    evaluate_settlement_emergencies_now(
        &mut world,
        &catalog,
        &buildings,
        ctx.items,
        ctx,
        id,
        20,
    );
    assert!(world
        .settlement_state_store()
        .get(id)
        .unwrap()
        .emergencies
        .instance("active_attack")
        .is_some());

    // Below deactivation but before min_active_duration — still active.
    if let Some(state) = world.settlement_state_store_mut().get_mut(id) {
        state
            .extension_seams
            .insert("hostile_threat".into(), "0.1".into());
    }
    evaluate_settlement_emergencies_now(
        &mut world,
        &catalog,
        &buildings,
        ctx.items,
        ctx,
        id,
        30,
    );
    assert!(world
        .settlement_state_store()
        .get(id)
        .unwrap()
        .emergencies
        .instance("active_attack")
        .is_some());

    // After min duration + below deactivation — clears.
    evaluate_settlement_emergencies_now(
        &mut world,
        &catalog,
        &buildings,
        ctx.items,
        ctx,
        id,
        80,
    );
    assert!(world
        .settlement_state_store()
        .get(id)
        .unwrap()
        .emergencies
        .instance("active_attack")
        .is_none());
}

#[test]
fn severity_scales_with_signal() {
    let (mut world, id, buildings) = setup_settlement();
    let catalog = EmergencyCatalog::default();
    let ctx = inventory_ctx();
    if let Some(state) = world.settlement_state_store_mut().get_mut(id) {
        state
            .extension_seams
            .insert("hostile_threat".into(), "0.7".into());
    }
    evaluate_settlement_emergencies_now(
        &mut world,
        &catalog,
        &buildings,
        ctx.items,
        ctx,
        id,
        10,
    );
    let sev = world
        .settlement_state_store()
        .get(id)
        .unwrap()
        .emergencies
        .instance("active_attack")
        .unwrap()
        .severity;
    assert!((sev - 0.7).abs() < 0.001);
}

#[test]
fn emergency_modifiers_alter_need_and_response_scores() {
    let (mut world, id, buildings) = setup_settlement();
    let emergency_catalog = EmergencyCatalog::default();
    let need_catalog = NeedCatalog::default();
    let response_catalog = ResponseCatalog::default();
    let ctx = inventory_ctx();

    // Force starvation active at full severity (empty stock already saturates base pressure).
    if let Some(state) = world.settlement_state_store_mut().get_mut(id) {
        state.emergencies.instances = vec![ActiveEmergencyInstance::new("starvation", 1, 1.0)];
        state.emergencies.sync_legacy_flags();
    }
    let state = world.settlement_state_store().get(id).unwrap();
    let delta = emergency_need_pressure_delta(state, &emergency_catalog, "food");
    assert!(delta > 0.0, "authored starvation pressure delta expected; got {delta}");

    evaluate_settlement_needs_now(
        &mut world,
        &need_catalog,
        &buildings,
        ctx.items,
        ctx,
        &emergency_catalog,
        id,
        2,
    );

    discover_settlement_responses_now(
        &mut world,
        &need_catalog,
        &response_catalog,
        &emergency_catalog,
        &buildings,
        id,
        2,
    );
    let luxury_blocked = world
        .response_candidate_store()
        .get(id)
        .unwrap()
        .candidates
        .iter()
        .filter(|c| c.response_id.as_str().contains("luxury"))
        .all(|c| !c.is_available());
    assert!(luxury_blocked, "luxury responses should be blocked under starvation");
}

#[test]
fn emergency_interruption_policy_relaxes_preempt() {
    let relax = EmergencyPreemptRelaxation {
        min_stick_ticks: 15,
        min_priority_rank_gap: 1,
        max_interruptible: TaskPriority::Normal,
    };
    let policy = PreemptPolicyOverride {
        min_stick_ticks: Some(relax.min_stick_ticks),
        min_priority_rank_gap: Some(relax.min_priority_rank_gap),
        max_interruptible: Some(relax.max_interruptible),
    };
    assert!(may_preempt_with_override(
        TaskPriority::High,
        TaskPriority::Low,
        20,
        None,
        policy,
    ));
    // High current work is protected when max_interruptible is Normal.
    assert!(!may_preempt_with_override(
        TaskPriority::High,
        TaskPriority::High,
        20,
        None,
        policy,
    ));
    // PlayerAssigned never interrupted.
    assert!(!may_preempt_with_override(
        TaskPriority::High,
        TaskPriority::PlayerAssigned,
        100,
        None,
        policy,
    ));
}

#[test]
fn player_and_ai_share_emergency_runtime() {
    let (mut world, player_id, buildings) = setup_settlement();
    // Create AI settlement state sharing same emergency catalog path.
    let ai_id = crate::world::SettlementId::new(99);
    world.settlement_state_store_mut().insert(
        crate::world::SettlementState::new(ai_id, SettlementKind::Town, false),
    );
    let catalog = EmergencyCatalog::default();
    let ctx = inventory_ctx();
    for id in [player_id, ai_id] {
        if let Some(state) = world.settlement_state_store_mut().get_mut(id) {
            state
                .extension_seams
                .insert("fire_severity".into(), "0.95".into());
        }
        evaluate_settlement_emergencies_now(
            &mut world,
            &catalog,
            &buildings,
            ctx.items,
            ctx,
            id,
            5,
        );
        assert!(world
            .settlement_state_store()
            .get(id)
            .unwrap()
            .emergencies
            .instance("critical_fire")
            .is_some());
    }
}

#[test]
fn emergency_continuity_survives_serialize_without_reports() {
    let (mut world, id, _) = setup_settlement();
    if let Some(state) = world.settlement_state_store_mut().get_mut(id) {
        state.emergencies.instances =
            vec![ActiveEmergencyInstance::new("starvation", 42, 0.8)];
        state.emergencies.sync_legacy_flags();
    }
    world.emergency_evaluation_store_mut().insert(EmergencyEvaluationReport {
        settlement_id: id,
        evaluated_tick: 99,
        signals: Vec::new(),
        activated: vec!["starvation".into()],
        deactivated: Vec::new(),
        diagnostics: vec!["temp".into()],
    });

    let save = world.settlement_state_store().export_save_state();
    let serialized = ron::ser::to_string(&save).unwrap();
    assert!(serialized.contains("starvation"));
    assert!(!serialized.contains("evaluated_tick")); // report not in SettlementState save

    let mut world2 = flat_world();
    world2
        .settlement_state_store_mut()
        .import_save_state(save);
    let restored = world2.settlement_state_store().get(id).unwrap();
    assert!(restored.emergencies.instance("starvation").is_some());
    assert!(world2.emergency_evaluation_store().get(id).is_none());
}

#[test]
fn catalog_validation_rejects_bad_thresholds() {
    let mut defs = starter_emergency_definitions();
    defs[0].deactivation_threshold = 0.9;
    defs[0].activation_threshold = 0.5;
    let catalog = EmergencyCatalog::from_definitions(defs).unwrap();
    let errors = validate_emergency_catalog(
        &catalog,
        &NeedCatalog::default(),
        &ResponseCatalog::default(),
    );
    assert!(errors.iter().any(|e| matches!(
        e,
        EmergencyValidationError::InvalidThresholds { .. }
    )));
}

#[test]
fn no_direct_worker_command_in_emergency_api() {
    // Compile-time / API surface check: evaluation only mutates SettlementEmergencyState.
    let (mut world, id, buildings) = setup_settlement();
    sync_construction_tasks(&mut world, &buildings, 1);
    let tasks_before = world.task_store().sorted_task_ids().len();
    let catalog = EmergencyCatalog::default();
    let ctx = inventory_ctx();
    evaluate_settlement_emergencies_now(
        &mut world,
        &catalog,
        &buildings,
        ctx.items,
        ctx,
        id,
        1,
    );
    assert_eq!(world.task_store().sorted_task_ids().len(), tasks_before);
}
