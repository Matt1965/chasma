//! Contextual building diagnostics for the Selected Object Diagnostics toggle.

use crate::dev::inspector::{BuildingDevCapabilities, BuildingInspectorSnapshot};

/// Format building diagnostics for the existing top-level Diagnostics section.
pub fn format_contextual_building_diagnostics(
    snapshot: &BuildingInspectorSnapshot,
    caps: BuildingDevCapabilities,
) -> String {
    let mut sections = Vec::new();

    let mut general = vec![
        snapshot.display_name.clone(),
        format!(
            "State: {}  HP: {} / {}",
            snapshot.lifecycle_state, snapshot.current_hp, snapshot.max_hp
        ),
    ];
    if snapshot.lifecycle_state != "Complete" && snapshot.progress_percent < 100.0 {
        general.push(format!("Construction: {:.0}%", snapshot.progress_percent));
    }
    sections.push(general.join("\n"));

    if caps.production {
        sections.push(format_production_section(snapshot));
    }

    if caps.inventory {
        if let Some(section) = format_inventory_section(snapshot) {
            sections.push(section);
        }
    }

    if caps.terrain {
        if let Some(section) = format_terrain_section(snapshot) {
            sections.push(section);
        }
    }

    sections.join("\n\n")
}

fn format_production_section(snapshot: &BuildingInspectorSnapshot) -> String {
    let mut lines = Vec::new();

    let operation = snapshot
        .diagnostics_operation_display
        .as_deref()
        .map(str::to_string)
        .unwrap_or_else(|| "None".into());
    lines.push(format!("Operation: {operation}"));

    lines.push(format!("State: {}", production_state_label(snapshot)));

    if let Some(progress) = meaningful_progress(snapshot) {
        lines.push(format!("Progress: {progress}"));
    }

    if let Some(count) = snapshot.active_worker_count {
        lines.push(format!("Workers: {count}"));
    }

    if let Some(control) = snapshot
        .control_source
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        lines.push(format!("Control: {control}"));
    }

    if let Some(reason) = meaningful_blocking_reason(snapshot) {
        lines.push(format!("Blocking reason: {reason}"));
    }

    if let Some(supported) = snapshot
        .diagnostics_supported_operations_display
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        lines.push(format!("Supported: {supported}"));
    }

    if let Some(error) = snapshot.diagnostics_production_error.as_deref() {
        lines.push(error.to_string());
    }

    lines.join("\n")
}

fn format_inventory_section(snapshot: &BuildingInspectorSnapshot) -> Option<String> {
    if snapshot.diagnostics_inventory_lines.is_empty()
        && snapshot.diagnostics_inventory_error.is_none()
    {
        return None;
    }
    let mut lines = snapshot.diagnostics_inventory_lines.clone();
    if let Some(error) = snapshot.diagnostics_inventory_error.as_deref() {
        lines.push(error.to_string());
    }
    Some(lines.join("\n"))
}

fn format_terrain_section(snapshot: &BuildingInspectorSnapshot) -> Option<String> {
    let mut lines = snapshot.diagnostics_terrain_lines.clone();

    if let Some(efficiency) = snapshot.diagnostics_terrain_efficiency.as_deref() {
        lines.push(format!("Terrain efficiency: {efficiency}"));
    }

    if let Some(limiting) = snapshot
        .diagnostics_terrain_limiting
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        lines.push(format!("Limiting factor: {limiting}"));
    }

    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

fn production_state_label(snapshot: &BuildingInspectorSnapshot) -> String {
    if snapshot.policy_enabled == Some(false) {
        return "Disabled".into();
    }
    if snapshot.policy_paused == Some(true) {
        return "Paused".into();
    }
    if meaningful_blocking_reason(snapshot).is_some() {
        return "Blocked".into();
    }
    match snapshot.production_lifecycle.as_deref() {
        Some("Running") | Some("running") => "Running".into(),
        Some("Idle") | Some("idle") => "Idle".into(),
        Some(other) if !other.is_empty() && !looks_like_empty_collection(other) => {
            other.to_string()
        }
        _ => {
            if snapshot.active_worker_count.unwrap_or(0) > 0 {
                "Running".into()
            } else {
                "Idle".into()
            }
        }
    }
}

fn meaningful_progress(snapshot: &BuildingInspectorSnapshot) -> Option<&str> {
    snapshot
        .operation_progress
        .as_deref()
        .filter(|value| !value.is_empty() && *value != "0%" && *value != "0.0%")
}

fn meaningful_blocking_reason(snapshot: &BuildingInspectorSnapshot) -> Option<String> {
    snapshot
        .production_blocking_reason
        .as_deref()
        .filter(|value| !value.is_empty() && !looks_like_empty_collection(value))
        .map(str::to_string)
        .or_else(|| {
            snapshot
                .execution_blocking
                .as_deref()
                .filter(|value| !value.is_empty() && !looks_like_empty_collection(value))
                .map(str::to_string)
        })
}

fn looks_like_empty_collection(value: &str) -> bool {
    value == "[]" || value == "—" || value == "-"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dev::inspector::BuildingInspectorSnapshot;
    use crate::world::{BuildingDefinitionId, BuildingId, ChunkCoord};

    fn farm_caps() -> BuildingDevCapabilities {
        BuildingDevCapabilities {
            construction: true,
            lifecycle: true,
            production: true,
            production_operation_selector: false,
            inventory: true,
            doors: false,
            terrain: true,
        }
    }

    fn chest_caps() -> BuildingDevCapabilities {
        BuildingDevCapabilities {
            construction: true,
            lifecycle: true,
            production: false,
            production_operation_selector: false,
            inventory: true,
            doors: false,
            terrain: false,
        }
    }

    fn base_snapshot() -> BuildingInspectorSnapshot {
        BuildingInspectorSnapshot {
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
            inventory_summary: None,
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
            operation_progress: Some("42%".into()),
            operation_completions: None,
            operation_limiting_factor: None,
            production_lifecycle: Some("Running".into()),
            selected_operation: Some("grow_prispods".into()),
            policy_enabled: Some(true),
            policy_paused: Some(false),
            repeat_mode: None,
            control_source: Some("Settlement AI".into()),
            policy_priority: None,
            assigned_workers: None,
            production_blocking_reason: None,
            active_worker_count: Some(1),
            remaining_repeat_count: None,
            last_efficiency_revision: None,
            supported_operations: Some("grow_prispods".into()),
            default_operation: None,
            operation_category: None,
            base_labor: None,
            max_workers: None,
            validation_state: Some("OK".into()),
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
            diagnostics_inventory_lines: vec!["Output — Prispods: 7 / 20".into()],
            diagnostics_terrain_lines: vec!["Water: 14%".into()],
            diagnostics_production_error: None,
            diagnostics_inventory_error: None,
            diagnostics_operation_display: Some("Grow Prispods".into()),
            diagnostics_supported_operations_display: None,
            diagnostics_terrain_efficiency: Some("63%".into()),
            diagnostics_terrain_limiting: Some("Water".into()),
        }
    }

    #[test]
    fn farm_diagnostics_omit_validation_noise_and_raw_ids() {
        let text = format_contextual_building_diagnostics(&base_snapshot(), farm_caps());
        assert!(text.contains("Prispod Farm"));
        assert!(text.contains("Operation: Grow Prispods"));
        assert!(text.contains("Output — Prispods: 7 / 20"));
        assert!(text.contains("Water: 14%"));
        assert!(!text.contains("InventoryId"));
        assert!(!text.contains("[]"));
        assert!(!text.contains("Validation: OK"));
        assert!(!text.contains("primary_output"));
    }

    #[test]
    fn production_error_shown_automatically_when_invalid() {
        let mut snapshot = base_snapshot();
        snapshot.diagnostics_production_error =
            Some("Production error: No operation selected".into());
        let text = format_contextual_building_diagnostics(&snapshot, farm_caps());
        assert!(text.contains("Production error: No operation selected"));
    }

    #[test]
    fn chest_diagnostics_omit_production_and_terrain() {
        let mut snapshot = base_snapshot();
        snapshot.display_name = "Storage Chest".into();
        snapshot.diagnostics_terrain_lines.clear();
        snapshot.diagnostics_terrain_efficiency = None;
        snapshot.diagnostics_inventory_lines = vec!["General — Gold: 5 / 50".into()];
        let text = format_contextual_building_diagnostics(&snapshot, chest_caps());
        assert!(!text.contains("Operation:"));
        assert!(!text.contains("Water:"));
        assert!(text.contains("General — Gold: 5 / 50"));
    }

    #[test]
    fn no_operation_selected_reads_as_none() {
        let mut snapshot = base_snapshot();
        snapshot.diagnostics_operation_display = None;
        let text = format_contextual_building_diagnostics(&snapshot, farm_caps());
        assert!(text.contains("Operation: None"));
    }
}
