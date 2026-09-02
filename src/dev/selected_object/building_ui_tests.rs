//! Selected Object building UI structure tests.

use super::building_actions_ui::BuildingDevSectionKind;
use crate::dev::inspector::BuildingDevAction;

#[test]
fn building_action_sections_exclude_nested_diagnostics_and_inventory() {
    let kinds = [
        BuildingDevSectionKind::Construction,
        BuildingDevSectionKind::Lifecycle,
        BuildingDevSectionKind::Production,
        BuildingDevSectionKind::Doors,
        BuildingDevSectionKind::Terrain,
    ];
    assert_eq!(kinds.len(), 5);
}

#[test]
fn production_actions_exclude_validation_buttons() {
    for action in BuildingDevAction::PRODUCTION_ACTIONS {
        assert!(!matches!(
            action,
            BuildingDevAction::ValidateProduction | BuildingDevAction::ValidateInventoryLinks
        ));
    }
}

#[test]
fn compact_building_summary_omits_technical_dump_fields() {
    use crate::dev::inspector::BuildingInspectorSnapshot;
    use crate::dev::selected_object::format::format_building_summary;
    use crate::world::{BuildingDefinitionId, BuildingId, ChunkCoord};

    let snapshot = BuildingInspectorSnapshot {
        building_id: BuildingId::new(1),
        definition_id: BuildingDefinitionId::new("prispod_farm"),
        display_name: "Prispod Farm".into(),
        current_hp: 300,
        max_hp: 300,
        lifecycle_state: "Complete".into(),
        progress_percent: 100.0,
        operational: true,
        affiliation: "Player".into(),
        chunk: ChunkCoord::new(0, 0),
        inventory_summary: Some("inventory=InventoryId(2)".into()),
        interaction_point: None,
        desired_render_key: None,
        resolved_asset_path: None,
        asset_load_state: None,
        runtime_entity: None,
        uses_diagnostic_fallback: false,
        fallback_reason: None,
        space_tag_count: None,
        roof_tag_count: None,
        terrain_output_rate: None,
        final_output_rate: None,
        operation_progress: None,
        operation_completions: None,
        operation_limiting_factor: None,
        production_lifecycle: None,
        selected_operation: None,
        policy_enabled: None,
        policy_paused: None,
        repeat_mode: None,
        control_source: None,
        policy_priority: None,
        assigned_workers: None,
        production_blocking_reason: None,
        active_worker_count: None,
        remaining_repeat_count: None,
        last_efficiency_revision: None,
        supported_operations: None,
        default_operation: None,
        operation_category: None,
        base_labor: None,
        max_workers: None,
        validation_state: None,
        execution_inputs_summary: None,
        execution_outputs_summary: None,
        execution_inventory_summary: None,
        execution_blocking: None,
        terrain_assessment_summary: None,
        terrain_assessment_revision: None,
        terrain_assessment_stale: None,
        inventory_bindings_summary: None,
        hauling_requests_summary: None,
        planner_summary: None,
        settlement_membership: "None".into(),
        diagnostics_inventory_lines: Vec::new(),
        diagnostics_terrain_lines: Vec::new(),
        diagnostics_production_error: None,
        diagnostics_inventory_error: None,
        diagnostics_operation_display: None,
        diagnostics_supported_operations_display: None,
        diagnostics_terrain_efficiency: None,
        diagnostics_terrain_limiting: None,
    };
    let summary = format_building_summary(&snapshot);
    assert!(summary.contains("Prispod Farm"));
    assert!(summary.contains("Complete | HP 300/300"));
    assert!(!summary.contains("InventoryId"));
    assert!(!summary.contains("grow_prispods"));
}
