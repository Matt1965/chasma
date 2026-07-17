//! Apply offline asset sizing resolution to catalog definitions (ADR-097 DT1).

use crate::world::asset_sizing::doodad_visual_collision_mismatch_warning;
use crate::world::asset_sizing::{AssetSizingReport, unit_baseline_render_scale};
use crate::world::{BuildingDefinition, DoodadDefinition, UnitDefinition};

#[cfg(feature = "data-import")]
use crate::data_import::asset_sizing::{
    ContentSizingKind, SizingResolveInput, resolve_content_sizing,
};

#[cfg(feature = "data-import")]
pub fn finalize_unit_definition(
    definition: &mut UnitDefinition,
    legacy_render_scale: f32,
) -> AssetSizingReport {
    let Some(render_key) = definition.render_key.0.as_deref() else {
        return empty_report("Unit", definition.id.as_str());
    };
    let report = resolve_content_sizing(SizingResolveInput {
        definition_id: definition.id.as_str(),
        render_key,
        asset_root: "units",
        kind: ContentSizingKind::Unit,
        sizing: &mut definition.asset_sizing,
        legacy_uniform_scale: Some(legacy_render_scale),
        building_footprint_width_meters: None,
        building_footprint_depth_meters: None,
    });
    definition.render_scale = unit_baseline_render_scale(definition);
    report
}

#[cfg(feature = "data-import")]
pub fn finalize_doodad_definition(definition: &mut DoodadDefinition) -> AssetSizingReport {
    let Some(render_key) = definition.render_key.0.as_deref() else {
        return empty_report("Doodad", definition.id.as_str());
    };
    let mut report = resolve_content_sizing(SizingResolveInput {
        definition_id: definition.id.as_str(),
        render_key,
        asset_root: "doodads",
        kind: ContentSizingKind::Doodad,
        sizing: &mut definition.asset_sizing,
        legacy_uniform_scale: None,
        building_footprint_width_meters: None,
        building_footprint_depth_meters: None,
    });
    if let Some(warning) = doodad_visual_collision_mismatch_warning(definition) {
        report.warnings.push(warning);
    }
    report
}

#[cfg(feature = "data-import")]
pub fn finalize_building_definition(definition: &mut BuildingDefinition) -> AssetSizingReport {
    let Some(render_key) = definition.render_key.0.as_deref() else {
        return empty_report("Building", definition.id.as_str());
    };
    let (footprint_width, footprint_depth) = match &definition.footprint {
        crate::world::FootprintSpec::Rectangle {
            width_meters,
            depth_meters,
        } => (Some(*width_meters), Some(*depth_meters)),
        _ => (None, None),
    };
    resolve_content_sizing(SizingResolveInput {
        definition_id: definition.id.as_str(),
        render_key,
        asset_root: "buildings",
        kind: ContentSizingKind::Building {
            safety_class: definition.transform_safety_class,
        },
        sizing: &mut definition.asset_sizing,
        legacy_uniform_scale: None,
        building_footprint_width_meters: footprint_width,
        building_footprint_depth_meters: footprint_depth,
    })
}

fn empty_report(kind: &str, id: &str) -> AssetSizingReport {
    AssetSizingReport {
        definition_kind: kind.to_string(),
        definition_id: id.to_string(),
        ..Default::default()
    }
}

#[cfg(not(feature = "data-import"))]
pub fn finalize_unit_definition(
    definition: &mut UnitDefinition,
    legacy_render_scale: f32,
) -> AssetSizingReport {
    definition.render_scale = legacy_render_scale;
    empty_report("Unit", definition.id.as_str())
}

#[cfg(not(feature = "data-import"))]
pub fn finalize_doodad_definition(_definition: &mut DoodadDefinition) -> AssetSizingReport {
    AssetSizingReport::default()
}

#[cfg(not(feature = "data-import"))]
pub fn finalize_building_definition(_definition: &mut BuildingDefinition) -> AssetSizingReport {
    AssetSizingReport::default()
}
