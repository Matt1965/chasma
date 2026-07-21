//! Default metric sizing targets when Excel Desired*M columns are absent.
//!
//! These are import-time hints only — GLB measurement still produces the baked baseline.
//! Author explicit Desired*M in `Chasma Design.xlsx` to override.

use crate::world::asset_sizing::{AssetSizingDefinition, SizeReferenceAxis};

/// Typical standing height for a unit when Desired Height M is missing.
pub fn unit_default_desired_height_meters(unit_id: &str, collision_radius_meters: f32) -> f32 {
    let id = unit_id.trim().to_ascii_lowercase();
    match id.as_str() {
        "robot" | "player" | "player_robot" => 1.75,
        "wolf" | "fox" | "dog" | "coyote" => 0.9,
        "deer" | "elk" => 1.35,
        "bear" => 1.6,
        "bandit" | "villager" | "human" => 1.75,
        _ => (collision_radius_meters * 1.75).clamp(0.35, 2.5),
    }
}

/// Fill desired meters from an authored building footprint so shell visuals align with
/// interior space and occupancy (ADR-126).
pub fn apply_building_footprint_sizing_targets(
    sizing: &mut AssetSizingDefinition,
    width_meters: f32,
    depth_meters: f32,
) {
    let width = width_meters.max(0.1);
    let depth = depth_meters.max(0.1);
    let long_edge = width.max(depth);

    sizing.desired_width_meters = Some(width);
    sizing.desired_depth_meters = Some(depth);
    sizing.desired_height_meters = Some(building_default_height_meters(long_edge));
    sizing.size_reference_axis = Some(if width >= depth {
        SizeReferenceAxis::Width
    } else {
        SizeReferenceAxis::Depth
    });
}

fn building_default_height_meters(long_footprint_edge_meters: f32) -> f32 {
    let height = if long_footprint_edge_meters <= 1.5 {
        // Chests, crates, interior props placed as buildings.
        long_footprint_edge_meters * 0.85
    } else if long_footprint_edge_meters <= 6.0 {
        // Huts, smelters, small structures.
        long_footprint_edge_meters * 0.75
    } else {
        // Barns and large shells.
        long_footprint_edge_meters * 0.55
    };
    height.clamp(0.5, 12.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wolf_height_target() {
        assert!((unit_default_desired_height_meters("wolf", 0.6) - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn fox_height_target() {
        assert!((unit_default_desired_height_meters("fox", 0.25) - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn chest_footprint_targets() {
        let mut sizing = AssetSizingDefinition::default();
        apply_building_footprint_sizing_targets(&mut sizing, 1.0, 0.8);
        assert_eq!(sizing.desired_width_meters, Some(1.0));
        assert_eq!(sizing.desired_depth_meters, Some(0.8));
        assert_eq!(sizing.size_reference_axis, Some(SizeReferenceAxis::Width));
        assert!(sizing.desired_height_meters.unwrap() < 1.0);
    }

    #[test]
    fn hut_footprint_targets() {
        let mut sizing = AssetSizingDefinition::default();
        apply_building_footprint_sizing_targets(&mut sizing, 4.0, 4.0);
        assert_eq!(sizing.desired_height_meters, Some(3.0));
    }
}

#[cfg(all(test, feature = "data-import"))]
mod integration_tests {
    use super::*;
    use crate::world::BuildingCatalog;
    use crate::world::BuildingDefinitionId;
    use crate::world::asset_sizing::building_visual_scale;
    use crate::world::asset_sizing::finalize_building_definition;
    use crate::world::asset_sizing::SizingMigrationState;

    #[test]
    fn storage_chest_footprint_sizing_bakes_baseline() {
        let mut definition = BuildingCatalog::default()
            .get(&BuildingDefinitionId::new("storage_chest"))
            .expect("starter catalog includes storage_chest")
            .clone();

        let report = finalize_building_definition(&mut definition);
        assert!(
            report.errors.is_empty(),
            "sizing errors: {:?}, warnings: {:?}",
            report.errors,
            report.warnings
        );
        assert_eq!(
            definition.asset_sizing.migration_state,
            SizingMigrationState::MetricConfigured
        );
        assert!(
            (definition.asset_sizing.source_bounds_unit_divisor - 1000.0).abs() < f32::EPSILON,
            "chest GLB is mm export; expected ÷1000 divisor, got {}",
            definition.asset_sizing.source_bounds_unit_divisor
        );
        let source = definition
            .asset_sizing
            .calculated_source_bounds
            .expect("source bounds baked");
        assert!(
            source.width_meters < 5.0,
            "expected mm→m correction on chest GLB, got {source:?}"
        );
        let final_dims = definition
            .asset_sizing
            .approximate_final_dimensions_meters()
            .expect("final dims");
        assert!(
            (final_dims.width_meters - 1.0).abs() < 0.15,
            "chest width should match footprint, got {final_dims:?}"
        );
        let visual_scale = building_visual_scale(&definition, 1.0);
        assert!(
            (visual_scale.x - 1.0 / 1200.0).abs() < 0.0002,
            "runtime scale should correct raw mm vertices, got {visual_scale:?}"
        );
    }
}
