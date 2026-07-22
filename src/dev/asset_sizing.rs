//! Read-only asset sizing calibration display for Dev Mode (ADR-097 DT1, ADR-127 AT1).

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
            .map(|def| (&def.asset_sizing, def.render_key.0.as_deref(), None)),
        DefinitionId::Doodad(id) => doodad_catalog.get(id).map(|def| {
            (
                &def.asset_sizing,
                def.render_key.0.as_deref(),
                Some(format!(
                    "Collision/pick (instance 1): {:.3} m radius (authored {:.3} × baseline XZ)",
                    crate::world::doodad_definition_placement_radius_meters(def),
                    def.placement_radius_meters
                        .max(def.block_radius_meters)
                )),
            )
        }),
        DefinitionId::Building(id) => building_catalog.get(id).map(|def| {
            let footprint = match &def.footprint {
                crate::world::FootprintSpec::Rectangle {
                    width_meters,
                    depth_meters,
                } => format!("Footprint: {width_meters:.3} × {depth_meters:.3} m (× instance)"),
                crate::world::FootprintSpec::Circle { radius_meters } => {
                    format!("Footprint: circle r={radius_meters:.3} m (× instance)")
                }
                crate::world::FootprintSpec::MeshDerived => "Footprint: mesh-derived".into(),
            };
            (&def.asset_sizing, def.render_key.0.as_deref(), Some(footprint))
        }),
        DefinitionId::Item(_) | DefinitionId::InventoryProfile(_) => None,
    };

    let Some((sizing, render_key, gameplay)) = sizing else {
        return format!("Asset sizing: {} not found in catalog", selection.id_str());
    };

    let mut text = format_sizing_definition(selection.id_str(), render_key, sizing);
    if let Some(line) = gameplay {
        text.push('\n');
        text.push_str(&line);
    }
    text
}

fn format_sizing_definition(
    definition_id: &str,
    render_key: Option<&str>,
    sizing: &AssetSizingDefinition,
) -> String {
    let mut lines = vec![
        format!("Asset sizing (catalog authority): {definition_id}"),
        "AT3: gameplay (collision/pick/occupancy) uses the same metric composition as visuals".into(),
    ];
    if let Some(key) = render_key {
        lines.push(format!("Render key: {key}"));
    }

    match sizing.migration_state {
        SizingMigrationState::MetricConfigured => {
            lines.push("Migration: MetricConfigured ✓".into());
        }
        SizingMigrationState::LegacyExplicitScale => {
            lines.push("Migration: LegacyExplicitScale — migrate to Desired meters".into());
        }
        SizingMigrationState::MissingSizingData => {
            lines.push("Migration: ★ MissingSizingData ★ — add Desired*M or bake baseline".into());
        }
    }

    if let Some(source) = sizing.authoritative_source_dimensions() {
        lines.push(format!(
            "Source (measured/explicit): {:.3} × {:.3} × {:.3} m ({:?})",
            source.width_meters,
            source.height_meters,
            source.depth_meters,
            sizing.source_bounds_origin
        ));
    } else {
        lines.push("Source (measured/explicit): (none)".into());
    }

    let desired = format_desired(sizing);
    if desired.is_empty() {
        lines.push("Desired: (none — author Desired Width/Height/Depth M)".into());
    } else {
        lines.push(desired);
    }

    if let Some(axis) = sizing.size_reference_axis {
        lines.push(format!("Reference axis: {}", axis.label()));
    }

    let baseline = sizing.authoritative_baseline_scale().to_vec3();
    lines.push(format!(
        "Definition baseline: {:.3}, {:.3}, {:.3}",
        baseline.x, baseline.y, baseline.z
    ));
    if let Some(explicit) = sizing.explicit_baseline_scale {
        let e = explicit.to_vec3();
        lines.push(format!(
            "Explicit baseline (escape hatch): {:.3}, {:.3}, {:.3}",
            e.x, e.y, e.z
        ));
    }

    lines.push(format!(
        "Composed visual (baseline × instance 1.0): {:.3}, {:.3}, {:.3}",
        baseline.x, baseline.y, baseline.z
    ));
    lines.push(
        "Instance scale lives on placement records — ECS Transform = composed presentation only"
            .into(),
    );

    if let Some(final_dims) = sizing.approximate_final_dimensions_meters() {
        lines.push(format!(
            "Approx final (source × baseline): {:.3} × {:.3} × {:.3} m",
            final_dims.width_meters, final_dims.height_meters, final_dims.depth_meters
        ));
    }

    let offset = sizing.authoritative_pivot_offset_meters();
    lines.push(format!(
        "Pivot correction: {:.3}, {:.3}, {:.3} m",
        offset.x, offset.y, offset.z
    ));

    let rot = sizing.authoritative_rotation_correction();
    lines.push(format!(
        "Import rotation correction: yaw {:.1}° pitch {:.1}° roll {:.1}°",
        rot.yaw_degrees(),
        rot.pitch_degrees(),
        rot.roll_degrees()
    ));

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
    let offset = sizing.model_local_offset_meters;
    push(
        &mut rows,
        "model_offset_x_m",
        format!("{:.6}", offset.x),
    );
    push(
        &mut rows,
        "model_offset_y_m",
        format!("{:.6}", offset.y),
    );
    push(
        &mut rows,
        "model_offset_z_m",
        format!("{:.6}", offset.z),
    );
    let rot = sizing.rotation_correction;
    push(
        &mut rows,
        "rotation_correction_yaw_deg",
        format!("{:.3}", rot.yaw_degrees()),
    );
    rows.join("\n")
}
