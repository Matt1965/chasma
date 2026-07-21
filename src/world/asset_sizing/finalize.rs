//! Apply offline asset sizing resolution to catalog definitions (ADR-097 DT1, ADR-127 AT1).

use crate::world::asset_sizing::{
    normalize_building_sizing_authority, AssetSizingReport,
};
use crate::world::{BuildingDefinition, DoodadDefinition, UnitDefinition};

#[cfg(feature = "data-import")]
use crate::data_import::kind_default_visual_height_meters;
#[cfg(feature = "data-import")]
use crate::data_import::asset_sizing::{
    resolve_content_sizing, unit_default_desired_height_meters, ContentSizingKind,
    SizingResolveInput,
};
#[cfg(feature = "data-import")]
use crate::world::asset_sizing::{
    building_visual_footprint_mismatch_warning, doodad_visual_collision_mismatch_warning,
    sync_building_legacy_mirrors_from_sizing, unit_baseline_render_scale,
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
        doodad_kind_height_hint_meters: None,
        unit_height_hint_meters: Some(unit_default_desired_height_meters(
            definition.id.as_str(),
            definition.collision_radius_meters,
        )),
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
        doodad_kind_height_hint_meters: Some(kind_default_visual_height_meters(definition.kind)),
        unit_height_hint_meters: None,
    });
    if let Some(warning) = doodad_visual_collision_mismatch_warning(definition) {
        report.warnings.push(warning);
    }
    report
}

#[cfg(feature = "data-import")]
pub fn finalize_building_definition(definition: &mut BuildingDefinition) -> AssetSizingReport {
    let Some(render_key) = definition.render_key.0.clone() else {
        return empty_report("Building", definition.id.as_str());
    };
    // AT1: fold legacy corrections into asset_sizing before measuring/baking baseline.
    normalize_building_sizing_authority(definition);
    let (footprint_width, footprint_depth) = match &definition.footprint {
        crate::world::FootprintSpec::Rectangle {
            width_meters,
            depth_meters,
        } => (Some(*width_meters), Some(*depth_meters)),
        crate::world::FootprintSpec::Circle { radius_meters } => {
            let diameter = radius_meters * 2.0;
            (Some(diameter), Some(diameter))
        }
        _ => (None, None),
    };
    let mut report = resolve_content_sizing(SizingResolveInput {
        definition_id: definition.id.as_str(),
        render_key: render_key.as_str(),
        asset_root: "buildings",
        kind: ContentSizingKind::Building {
            safety_class: definition.transform_safety_class,
        },
        sizing: &mut definition.asset_sizing,
        legacy_uniform_scale: None,
        building_footprint_width_meters: footprint_width,
        building_footprint_depth_meters: footprint_depth,
        doodad_kind_height_hint_meters: None,
        unit_height_hint_meters: None,
    });
    // Keep legacy mirrors aligned after import bake.
    sync_building_legacy_mirrors_from_sizing(definition);
    if let Some(warning) = building_visual_footprint_mismatch_warning(definition) {
        report.warnings.push(warning);
    }
    report
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
pub fn finalize_building_definition(definition: &mut BuildingDefinition) -> AssetSizingReport {
    normalize_building_sizing_authority(definition);
    empty_report("Building", definition.id.as_str())
}
