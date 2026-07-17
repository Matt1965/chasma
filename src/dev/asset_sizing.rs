//! Read-only asset sizing calibration display for Dev Mode (ADR-097 DT1).

use crate::world::asset_sizing::{AssetSizingDefinition, SizingMigrationState};
use crate::world::{BuildingCatalog, DoodadCatalog, UnitCatalog};

use super::dev_mode::DefinitionId;

pub fn format_asset_sizing_panel(
    selection: Option<&DefinitionId>,
    unit_catalog: &UnitCatalog,
    doodad_catalog: &DoodadCatalog,
    building_catalog: &BuildingCatalog,
) -> String {
    let Some(selection) = selection else {
        return "Asset sizing: select a Unit, Doodad, or Building".into();
    };

    let sizing = match selection {
        DefinitionId::Unit(id) => unit_catalog
            .get(id)
            .map(|def| (&def.asset_sizing, def.render_key.0.as_deref())),
        DefinitionId::Doodad(id) => doodad_catalog
            .get(id)
            .map(|def| (&def.asset_sizing, def.render_key.0.as_deref())),
        DefinitionId::Building(id) => building_catalog
            .get(id)
            .map(|def| (&def.asset_sizing, def.render_key.0.as_deref())),
    };

    let Some((sizing, render_key)) = sizing else {
        return format!("Asset sizing: {} not found in catalog", selection.id_str());
    };

    format_sizing_definition(selection.id_str(), render_key, sizing)
}

fn format_sizing_definition(
    definition_id: &str,
    render_key: Option<&str>,
    sizing: &AssetSizingDefinition,
) -> String {
    let mut lines = vec![format!("Asset sizing: {definition_id}")];
    if let Some(key) = render_key {
        lines.push(format!("Render key: {key}"));
    }
    lines.push(format!("Migration: {:?}", sizing.migration_state));

    if let Some(source) = sizing.resolved_source_bounds() {
        lines.push(format!(
            "Source: {:.3} × {:.3} × {:.3} m ({:?})",
            source.width_meters,
            source.height_meters,
            source.depth_meters,
            sizing.source_bounds_origin
        ));
    } else {
        lines.push("Source: (not measured)".into());
    }

    let desired = format_desired(sizing);
    if !desired.is_empty() {
        lines.push(desired);
    }

    if let Some(axis) = sizing.size_reference_axis {
        lines.push(format!("Reference axis: {}", axis.label()));
    }

    let baseline = sizing.resolved_baseline_scale().to_vec3();
    lines.push(format!(
        "Baseline scale: {:.3}, {:.3}, {:.3}",
        baseline.x, baseline.y, baseline.z
    ));

    if let Some(source) = sizing.resolved_source_bounds() {
        let final_dims = crate::world::asset_sizing::SourceDimensions {
            width_meters: source.width_meters * baseline.x,
            height_meters: source.height_meters * baseline.y,
            depth_meters: source.depth_meters * baseline.z,
        };
        lines.push(format!(
            "Approx final: {:.3} × {:.3} × {:.3} m",
            final_dims.width_meters, final_dims.height_meters, final_dims.depth_meters
        ));
    }

    let offset = sizing.model_local_offset_meters;
    if offset != bevy::prelude::Vec3::ZERO {
        lines.push(format!(
            "Model offset: {:.3}, {:.3}, {:.3} m",
            offset.x, offset.y, offset.z
        ));
    }

    let rot = sizing.rotation_correction;
    if rot != crate::world::authoring_transform::QuantizedOrientation::IDENTITY {
        lines.push(format!(
            "Rotation correction: {:.1}°, {:.1}°, {:.1}°",
            rot.yaw_degrees(),
            rot.pitch_degrees(),
            rot.roll_degrees()
        ));
    }

    match sizing.migration_state {
        SizingMigrationState::LegacyExplicitScale => {
            lines.push("Note: legacy scale — add Desired dimensions to migrate".into());
        }
        SizingMigrationState::MissingSizingData => {
            lines.push("Note: missing sizing metadata — using default scale".into());
        }
        SizingMigrationState::MetricConfigured => {}
    }

    lines.join("\n")
}

fn format_desired(sizing: &AssetSizingDefinition) -> String {
    let mut parts = Vec::new();
    if let Some(w) = sizing.desired_width_meters {
        parts.push(format!("W={w:.3}m"));
    }
    if let Some(h) = sizing.desired_height_meters {
        parts.push(format!("H={h:.3}m"));
    }
    if let Some(d) = sizing.desired_depth_meters {
        parts.push(format!("D={d:.3}m"));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("Desired: {}", parts.join(" × "))
    }
}

/// Export recommended calibration values as CSV (ADR-100 DT4).
pub fn export_calibration_csv(definition_id: &str, sizing: &AssetSizingDefinition) -> String {
    let mut rows = vec!["sheet,id,column,value".to_string()];
    let push = |rows: &mut Vec<String>, column: &str, value: String| {
        rows.push(format!("AssetSizing,{definition_id},{column},{value}"));
    };
    if let Some(source) = sizing.resolved_source_bounds() {
        push(
            &mut rows,
            "source_width_m",
            format!("{:.6}", source.width_meters),
        );
        push(
            &mut rows,
            "source_height_m",
            format!("{:.6}", source.height_meters),
        );
        push(
            &mut rows,
            "source_depth_m",
            format!("{:.6}", source.depth_meters),
        );
    }
    if let Some(w) = sizing.desired_width_meters {
        push(&mut rows, "desired_width_m", format!("{w:.6}"));
    }
    if let Some(h) = sizing.desired_height_meters {
        push(&mut rows, "desired_height_m", format!("{h:.6}"));
    }
    if let Some(d) = sizing.desired_depth_meters {
        push(&mut rows, "desired_depth_m", format!("{d:.6}"));
    }
    let baseline = sizing.resolved_baseline_scale().to_vec3();
    push(&mut rows, "baseline_scale_x", format!("{:.6}", baseline.x));
    push(&mut rows, "baseline_scale_y", format!("{:.6}", baseline.y));
    push(&mut rows, "baseline_scale_z", format!("{:.6}", baseline.z));
    rows.join("\n")
}

#[cfg(test)]
mod calibration_tests {
    use super::*;

    #[test]
    fn export_csv_is_deterministic() {
        let sizing = AssetSizingDefinition::default();
        let a = export_calibration_csv("robot", &sizing);
        let b = export_calibration_csv("robot", &sizing);
        assert_eq!(a, b);
        assert!(a.starts_with("sheet,id,column,value"));
    }
}
