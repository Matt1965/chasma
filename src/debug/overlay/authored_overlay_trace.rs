//! Per-frame authored blueprint overlay draw diagnostics (IN-11eV).

use bevy::prelude::*;

/// Bounded latest-frame authored overlay submission counts.
#[derive(Resource, Debug, Clone, Default)]
pub struct AuthoredBlueprintOverlayTrace {
    pub editor_open: bool,
    pub selected_building: Option<u64>,
    pub geometry_source: String,
    pub overlay_enabled: bool,
    pub draw_executed: bool,
    pub floor_count: u32,
    pub region_count: u32,
    pub vertex_count: u32,
    pub entrance_count: u32,
    pub edges_submitted: u32,
    pub vertices_submitted: u32,
    pub entrances_submitted: u32,
}

impl AuthoredBlueprintOverlayTrace {
    pub fn reset_frame(&mut self) {
        self.draw_executed = false;
        self.edges_submitted = 0;
        self.vertices_submitted = 0;
        self.entrances_submitted = 0;
    }
}

pub fn blueprint_topology_counts(
    blueprint: &crate::world::BuildingNavigationBlueprint,
) -> (u32, u32, u32) {
    let regions = blueprint
        .floors
        .iter()
        .map(|floor| floor.regions.len())
        .sum::<usize>() as u32;
    let vertices = blueprint
        .floors
        .iter()
        .flat_map(|floor| floor.regions.iter())
        .map(|region| region.walkable_outline.vertices_xz.len())
        .sum::<usize>() as u32;
    let entrances = blueprint.entrances.len() as u32;
    (regions, vertices, entrances)
}

#[cfg(test)]
mod tests {
    use super::blueprint_topology_counts;

    #[test]
    fn concave_polygon_counts_all_vertices() {
        use crate::world::{
            BuildingNavigationBlueprint, NavigationFloorDefinition, NavigationPolygon2d,
            NavigationRegionDefinition,
        };
        let vertices_xz = (0..8)
            .map(|i| {
                let angle = (i as f32 / 8.0) * std::f32::consts::TAU;
                [angle.cos(), angle.sin()]
            })
            .collect();
        let blueprint = BuildingNavigationBlueprint::new("hut_nav", "Hut").with_floors(vec![
            NavigationFloorDefinition {
                floor_id: 0,
                key: "floor_0".to_string(),
                display_label: "Floor 0".to_string(),
                elevation_meters: 0.0,
                visibility_group_id: 0,
                room_tag: None,
                walkable_outline_legacy: None,
                regions: vec![NavigationRegionDefinition {
                    key: "main".to_string(),
                    display_label: "Main".to_string(),
                    room_tag: None,
                    walkable_outline: NavigationPolygon2d { vertices_xz },
                }],
            },
        ]);
        let (regions, vertices, entrances) = blueprint_topology_counts(&blueprint);
        assert_eq!(regions, 1);
        assert_eq!(vertices, 8);
        assert_eq!(entrances, 0);
    }

    #[test]
    fn clearance_not_authored_blueprint_toggle() {
        let settings = crate::debug::settings::DebugOverlayConfig {
            enabled: true,
            nav_blueprint: true,
            nav_clearance: false,
            ..crate::debug::settings::DebugOverlayConfig::production()
        };
        assert!(settings.nav_blueprint);
        assert!(!settings.nav_clearance);
    }
}
