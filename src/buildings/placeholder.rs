use bevy::prelude::*;

use crate::world::{Affiliation, BuildingDefinition, FootprintSpec};

const PLACEHOLDER_HEIGHT_METERS: f32 = 2.5;

/// Cuboid dimensions for placeholder building presentation (B2).
pub fn placeholder_mesh_size(definition: &BuildingDefinition) -> Vec3 {
    match &definition.footprint {
        FootprintSpec::Rectangle {
            width_meters,
            depth_meters,
        } => Vec3::new(*width_meters, PLACEHOLDER_HEIGHT_METERS, *depth_meters),
        FootprintSpec::Circle { radius_meters } => {
            let diameter = radius_meters * 2.0;
            Vec3::new(diameter, PLACEHOLDER_HEIGHT_METERS, diameter)
        }
        FootprintSpec::MeshDerived => Vec3::new(2.0, PLACEHOLDER_HEIGHT_METERS, 2.0),
    }
}

/// Ray-pick radius from catalog footprint × instance uniform scale (ADR-126 AT3).
pub fn building_pick_radius(definition: &BuildingDefinition, uniform_scale: f32) -> f32 {
    let radius = match &definition.footprint {
        FootprintSpec::Rectangle {
            width_meters,
            depth_meters,
        } => width_meters.max(*depth_meters) * 0.5,
        FootprintSpec::Circle { radius_meters } => *radius_meters,
        FootprintSpec::MeshDerived => 1.5,
    };
    (radius * uniform_scale.max(0.0)).max(1.0)
}

pub fn affiliation_color(affiliation: Affiliation) -> Color {
    match affiliation {
        Affiliation::Player => Color::srgba(0.25, 0.55, 1.0, 0.9),
        Affiliation::Neutral => Color::srgba(0.65, 0.65, 0.65, 0.9),
        Affiliation::Hostile => Color::srgba(1.0, 0.3, 0.25, 0.9),
        Affiliation::Wildlife => Color::srgba(0.45, 0.85, 0.35, 0.9),
        Affiliation::Dev => Color::srgba(0.2, 0.9, 0.9, 0.9),
        Affiliation::Unknown => Color::srgba(0.85, 0.8, 0.2, 0.9),
    }
}

/// Planned construction presentation (B4) — translucent blueprint tint.
pub fn planned_building_color(affiliation: Affiliation) -> Color {
    let base = affiliation_color(affiliation);
    let s = base.to_srgba();
    Color::srgba(s.red, s.green, s.blue, 0.45)
}

/// Magenta/error diagnostic tint for missing or failed building assets (ADR-095 BA1).
pub fn diagnostic_fallback_color(
    lifecycle: crate::world::BuildingLifecycleState,
    affiliation: Affiliation,
) -> Color {
    let _ = affiliation;
    let lifecycle_tint = lifecycle_building_color(lifecycle, Affiliation::Dev);
    let s = lifecycle_tint.to_srgba();
    Color::srgba(
        (s.red * 0.35 + 0.95 * 0.65).min(1.0),
        (s.green * 0.35 + 0.1 * 0.65).min(1.0),
        (s.blue * 0.35 + 0.95 * 0.65).min(1.0),
        0.92,
    )
}

/// Lifecycle-specific scene tint (B5, ADR-095 BA1).
pub fn lifecycle_building_color(
    lifecycle: crate::world::BuildingLifecycleState,
    affiliation: Affiliation,
) -> Color {
    use crate::world::BuildingLifecycleState;
    match lifecycle {
        BuildingLifecycleState::Planned => planned_building_color(affiliation),
        BuildingLifecycleState::Foundation => Color::srgba(0.85, 0.55, 0.2, 0.75),
        BuildingLifecycleState::InProgress => Color::srgba(0.95, 0.75, 0.15, 0.85),
        BuildingLifecycleState::Complete => affiliation_color(affiliation),
        BuildingLifecycleState::Destroyed => Color::srgba(0.35, 0.1, 0.1, 0.9),
        BuildingLifecycleState::Ruins => Color::srgba(0.45, 0.42, 0.4, 0.7),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        BuildingCategoryId, BuildingDefinition, BuildingDefinitionId, BuildingRenderKey,
    };

    fn hut_definition() -> BuildingDefinition {
        BuildingDefinition::new(
            BuildingDefinitionId::new("hut"),
            "Hut",
            BuildingCategoryId::new("residential"),
            BuildingRenderKey::reserved("hut"),
            BuildingRenderKey::reserved("hut_collision"),
            250,
            45.0,
            FootprintSpec::Rectangle {
                width_meters: 4.0,
                depth_meters: 4.0,
            },
            35.0,
            true,
        )
    }

    #[test]
    fn rectangle_placeholder_size_matches_footprint() {
        let size = placeholder_mesh_size(&hut_definition());
        assert_eq!(size, Vec3::new(4.0, PLACEHOLDER_HEIGHT_METERS, 4.0));
    }

    #[test]
    fn pick_radius_from_footprint() {
        assert_eq!(building_pick_radius(&hut_definition(), 1.0), 2.0);
    }
}
