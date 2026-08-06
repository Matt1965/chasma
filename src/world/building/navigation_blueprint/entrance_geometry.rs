//! Edge-anchored exterior entrance geometry (IN-11e).

use bevy::prelude::*;

use super::definition::{
    BuildingNavigationBlueprint, NavigationEntranceDefinition, NavigationFloorDefinition,
    NavigationRegionDefinition,
};

/// Distance from a point to count as lying on a region boundary (blueprint-local units).
pub const ENTRANCE_BOUNDARY_TOLERANCE: f32 = 0.2;
/// Maximum pointer distance for snapping a new entrance onto a boundary edge.
pub const ENTRANCE_EDGE_SNAP_MAX_DISTANCE: f32 = 2.5;
/// Minimum distance from edge endpoints when placing or dragging a threshold.
pub const ENTRANCE_CORNER_MARGIN: f32 = 0.25;
/// Inward offset from threshold to interior landing (blueprint-local units).
pub const DEFAULT_INTERIOR_LANDING_OFFSET: f32 = 1.0;
/// Outward offset from threshold to exterior staging (blueprint-local units).
/// Must clear typical centered building footprints south/north of the nav boundary edge.
pub const DEFAULT_EXTERIOR_STAGING_OFFSET: f32 = 2.5;
/// Migration snap tolerance for legacy floating thresholds.
pub const ENTRANCE_MIGRATION_SNAP_TOLERANCE: f32 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundaryProjection {
    pub edge_index: usize,
    pub point: Vec2,
    pub distance_to_edge: f32,
    pub t_along_edge: f32,
    /// Unit tangent along the owning boundary edge (doorway width direction).
    pub edge_tangent: Vec2,
    pub inward_normal: Vec2,
    pub outward_normal: Vec2,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EntranceReanchorOutcome {
    AlreadyAnchored,
    Snapped { distance: f32 },
    TooFar { distance: f32 },
}

pub fn distance_point_to_segment(point: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq <= f32::EPSILON {
        return point.distance(a);
    }
    let t = ((point - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    point.distance(a + ab * t)
}

pub fn project_point_to_boundary(
    vertices: &[[f32; 2]],
    point: Vec2,
    corner_margin: f32,
) -> Option<BoundaryProjection> {
    nearest_boundary_projection(vertices, point, f32::INFINITY, corner_margin)
}

pub fn nearest_boundary_projection(
    vertices: &[[f32; 2]],
    point: Vec2,
    max_distance: f32,
    corner_margin: f32,
) -> Option<BoundaryProjection> {
    let count = vertices.len();
    if count < 2 {
        return None;
    }
    let mut best: Option<BoundaryProjection> = None;
    for index in 0..count {
        let a = Vec2::new(vertices[index][0], vertices[index][1]);
        let b = Vec2::new(
            vertices[(index + 1) % count][0],
            vertices[(index + 1) % count][1],
        );
        let edge = b - a;
        let len_sq = edge.length_squared();
        if len_sq <= f32::EPSILON {
            continue;
        }
        let edge_len = edge.length();
        if edge_len < corner_margin * 2.0 {
            continue;
        }
        let mut t = ((point - a).dot(edge) / len_sq).clamp(0.0, 1.0);
        let min_t = corner_margin / edge_len;
        let max_t = 1.0 - min_t;
        if max_t > min_t {
            t = t.clamp(min_t, max_t);
        }
        let projected = a + edge * t;
        let distance = point.distance(projected);
        if distance > max_distance {
            continue;
        }
        let inward = edge_inward_normal(vertices, index);
        let outward = -inward;
        let edge_tangent = edge / edge_len;
        let candidate = BoundaryProjection {
            edge_index: index,
            point: projected,
            distance_to_edge: distance,
            t_along_edge: t,
            edge_tangent,
            inward_normal: inward,
            outward_normal: outward,
        };
        if best
            .as_ref()
            .is_none_or(|prev| distance < prev.distance_to_edge)
        {
            best = Some(candidate);
        }
    }
    best
}

fn edge_inward_normal(vertices: &[[f32; 2]], edge_index: usize) -> Vec2 {
    let count = vertices.len();
    let a = Vec2::new(vertices[edge_index][0], vertices[edge_index][1]);
    let b = Vec2::new(
        vertices[(edge_index + 1) % count][0],
        vertices[(edge_index + 1) % count][1],
    );
    let edge = b - a;
    let left = Vec2::new(-edge.y, edge.x);
    if polygon_signed_area(vertices) >= 0.0 {
        normalize_or_axis(left)
    } else {
        normalize_or_axis(-left)
    }
}

fn normalize_or_axis(v: Vec2) -> Vec2 {
    let len = v.length();
    if len <= f32::EPSILON {
        Vec2::Y
    } else {
        v / len
    }
}

fn polygon_signed_area(vertices: &[[f32; 2]]) -> f32 {
    let mut area = 0.0;
    let count = vertices.len();
    if count < 3 {
        return area;
    }
    for index in 0..count {
        let a = Vec2::new(vertices[index][0], vertices[index][1]);
        let b = Vec2::new(
            vertices[(index + 1) % count][0],
            vertices[(index + 1) % count][1],
        );
        area += a.x * b.y - b.x * a.y;
    }
    area * 0.5
}

pub fn point_on_boundary_within_tolerance(
    vertices: &[[f32; 2]],
    point: Vec2,
    tolerance: f32,
) -> bool {
    nearest_boundary_projection(vertices, point, tolerance, ENTRANCE_CORNER_MARGIN)
        .is_some_and(|proj| proj.distance_to_edge <= tolerance)
}

pub fn derive_exterior_staging_xz(threshold: Vec2, outward_normal: Vec2, offset: f32) -> Vec2 {
    threshold + outward_normal * offset
}

pub fn apply_threshold_geometry(
    entrance: &mut NavigationEntranceDefinition,
    floor_elevation: f32,
    projection: &BoundaryProjection,
    interior_offset: f32,
) {
    entrance.local_position_xz = [projection.point.x, projection.point.y];
    entrance.interior_spawn_local = [
        projection.point.x + projection.inward_normal.x * interior_offset,
        floor_elevation,
        projection.point.y + projection.inward_normal.y * interior_offset,
    ];
}

pub fn exterior_staging_for_entrance(
    entrance: &NavigationEntranceDefinition,
    region: &NavigationRegionDefinition,
    exterior_offset: f32,
) -> Option<Vec2> {
    let threshold = Vec2::new(entrance.local_position_xz[0], entrance.local_position_xz[1]);
    let projection = project_point_to_boundary(
        &region.walkable_outline.vertices_xz,
        threshold,
        ENTRANCE_CORNER_MARGIN,
    )?;
    Some(derive_exterior_staging_xz(
        projection.point,
        projection.outward_normal,
        exterior_offset,
    ))
}

pub fn reanchor_entrance_to_region_boundary(
    entrance: &mut NavigationEntranceDefinition,
    region_vertices: &[[f32; 2]],
    floor_elevation: f32,
    snap_tolerance: f32,
) -> EntranceReanchorOutcome {
    let threshold = Vec2::new(entrance.local_position_xz[0], entrance.local_position_xz[1]);
    if point_on_boundary_within_tolerance(region_vertices, threshold, ENTRANCE_BOUNDARY_TOLERANCE) {
        return EntranceReanchorOutcome::AlreadyAnchored;
    }
    let Some(projection) = nearest_boundary_projection(
        region_vertices,
        threshold,
        snap_tolerance,
        ENTRANCE_CORNER_MARGIN,
    ) else {
        return EntranceReanchorOutcome::TooFar {
            distance: nearest_boundary_distance(region_vertices, threshold),
        };
    };
    apply_threshold_geometry(
        entrance,
        floor_elevation,
        &projection,
        DEFAULT_INTERIOR_LANDING_OFFSET,
    );
    EntranceReanchorOutcome::Snapped {
        distance: projection.distance_to_edge,
    }
}

fn nearest_boundary_distance(vertices: &[[f32; 2]], point: Vec2) -> f32 {
    nearest_boundary_projection(vertices, point, f32::INFINITY, ENTRANCE_CORNER_MARGIN)
        .map(|proj| proj.distance_to_edge)
        .unwrap_or(f32::INFINITY)
}

pub fn migrate_entrances_toward_boundaries(
    blueprint: &mut BuildingNavigationBlueprint,
) -> Vec<String> {
    let mut warnings = Vec::new();
    let entrance_keys: Vec<String> = blueprint.entrances.iter().map(|e| e.key.clone()).collect();
    for entrance_key in entrance_keys {
        let floor_key = blueprint
            .entrances
            .iter()
            .find(|entrance| entrance.key == entrance_key)
            .map(|entrance| entrance.floor_key.clone());
        let region_key_hint = blueprint
            .entrances
            .iter()
            .find(|entrance| entrance.key == entrance_key)
            .and_then(|entrance| entrance.region_key.clone());
        let Some(floor_key) = floor_key else {
            continue;
        };
        let region_key = match blueprint.resolve_region_key(
            &floor_key,
            region_key_hint.as_deref(),
            &entrance_key,
        ) {
            Ok(key) => key,
            Err(_) => continue,
        };
        let Some(floor) = blueprint.floor_by_key(&floor_key) else {
            continue;
        };
        let Some(region) = floor.region_by_key(region_key) else {
            continue;
        };
        let elevation = floor.elevation_meters;
        let vertices = region.walkable_outline.vertices_xz.clone();
        let region_label = region.key.clone();
        let Some(entrance) = blueprint
            .entrances
            .iter_mut()
            .find(|entrance| entrance.key == entrance_key)
        else {
            continue;
        };
        match reanchor_entrance_to_region_boundary(entrance, &vertices, elevation, f32::INFINITY) {
            EntranceReanchorOutcome::AlreadyAnchored => {}
            EntranceReanchorOutcome::Snapped { distance } => {
                warnings.push(format!(
                    "Entrance \"{}\" was projected {:.2} units onto the boundary of region \"{}\".",
                    entrance.key, distance, region_label
                ));
            }
            EntranceReanchorOutcome::TooFar { distance } => {
                warnings.push(format!(
                    "Entrance \"{}\" is not anchored to the boundary of region \"{}\" (distance {:.2}).",
                    entrance.key, region_label, distance
                ));
            }
        }
    }
    warnings
}

pub fn reanchor_entrances_after_region_edit(
    blueprint: &mut BuildingNavigationBlueprint,
    floor_key: &str,
    region_key: &str,
) {
    let Some(floor) = blueprint.floor_by_key(floor_key) else {
        return;
    };
    let Some(region) = floor.region_by_key(region_key) else {
        return;
    };
    let elevation = floor.elevation_meters;
    let vertices = region.walkable_outline.vertices_xz.clone();
    let entrance_keys: Vec<String> = blueprint
        .entrances
        .iter()
        .filter(|entrance| entrance.floor_key == floor_key)
        .map(|entrance| entrance.key.clone())
        .collect();
    for entrance_key in entrance_keys {
        let owns_region = blueprint
            .entrances
            .iter()
            .find(|entrance| entrance.key == entrance_key)
            .and_then(|entrance| {
                blueprint
                    .resolve_region_key(
                        &entrance.floor_key,
                        entrance.region_key.as_deref(),
                        &entrance.key,
                    )
                    .ok()
            })
            == Some(region_key);
        if !owns_region {
            continue;
        }
        let Some(entrance) = blueprint
            .entrances
            .iter_mut()
            .find(|entrance| entrance.key == entrance_key)
        else {
            continue;
        };
        let _ = reanchor_entrance_to_region_boundary(entrance, &vertices, elevation, f32::INFINITY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Vec<[f32; 2]> {
        vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]]
    }

    #[test]
    fn projects_pointer_onto_nearest_edge() {
        let projection = nearest_boundary_projection(
            &square(),
            Vec2::new(2.0, 4.5),
            ENTRANCE_EDGE_SNAP_MAX_DISTANCE,
            ENTRANCE_CORNER_MARGIN,
        )
        .expect("projection");
        assert!((projection.point.y - 4.0).abs() < 0.01);
        assert!((projection.point.x - 2.0).abs() < 0.01);
    }

    #[test]
    fn rejects_far_pointer_without_silent_snap() {
        assert!(
            nearest_boundary_projection(
                &square(),
                Vec2::new(2.0, 10.0),
                ENTRANCE_EDGE_SNAP_MAX_DISTANCE,
                ENTRANCE_CORNER_MARGIN,
            )
            .is_none()
        );
    }

    #[test]
    fn interior_landing_points_inward() {
        let projection =
            project_point_to_boundary(&square(), Vec2::new(2.0, 0.0), ENTRANCE_CORNER_MARGIN)
                .expect("projection");
        let mut entrance = NavigationEntranceDefinition {
            key: "e".into(),
            floor_key: "f".into(),
            region_key: Some("r".into()),
            local_position_xz: [0.0, 0.0],
            radius_meters: 1.0,
            interior_spawn_local: [0.0, 0.0, 0.0],
            bidirectional: true,
            door_key: None,
        };
        apply_threshold_geometry(
            &mut entrance,
            1.0,
            &projection,
            DEFAULT_INTERIOR_LANDING_OFFSET,
        );
        assert!(entrance.interior_spawn_local[2] > entrance.local_position_xz[1]);
    }

    #[test]
    fn exterior_staging_points_outward() {
        let projection =
            project_point_to_boundary(&square(), Vec2::new(2.0, 0.0), ENTRANCE_CORNER_MARGIN)
                .expect("projection");
        let staging = derive_exterior_staging_xz(
            projection.point,
            projection.outward_normal,
            DEFAULT_EXTERIOR_STAGING_OFFSET,
        );
        assert!(staging.y < projection.point.y);
    }

    #[test]
    fn edge_tangent_is_perpendicular_to_outward_normal() {
        let projection =
            project_point_to_boundary(&square(), Vec2::new(2.0, 0.0), ENTRANCE_CORNER_MARGIN)
                .expect("projection");
        let dot = projection.edge_tangent.dot(projection.outward_normal);
        assert!(
            dot.abs() < 1e-4,
            "tangent and outward normal must be perpendicular"
        );
        assert!(projection.edge_tangent.length() > 0.9);
    }
}
