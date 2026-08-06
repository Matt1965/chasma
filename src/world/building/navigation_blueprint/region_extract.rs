//! Multi-region walkable-surface extraction from triangle meshes (IN-09).

use std::collections::BTreeMap;

use bevy::prelude::*;

use super::definition::{NavigationPolygon2d, NavigationRegionDefinition};
use super::mesh::LocalTriangle3d;

/// Generator geometry tuning (deterministic defaults).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegionGeneratorConfig {
    pub walkable_normal_min_y: f32,
    pub floor_cluster_gap_meters: f32,
    pub edge_merge_epsilon: f32,
    pub simplify_epsilon: f32,
    pub min_region_area: f32,
    pub min_edge_length: f32,
}

impl Default for RegionGeneratorConfig {
    fn default() -> Self {
        Self {
            walkable_normal_min_y: 0.72,
            floor_cluster_gap_meters: 2.5,
            edge_merge_epsilon: 0.05,
            simplify_epsilon: 0.15,
            min_region_area: 0.5,
            min_edge_length: 0.1,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RegionExtractionStats {
    pub source_triangle_count: usize,
    pub walkable_triangle_count: usize,
    pub steep_triangle_discarded: usize,
    pub floor_cluster_count: usize,
    pub connected_component_count: usize,
    pub boundary_edge_count: usize,
    pub regions_emitted: usize,
    pub regions_discarded: usize,
    pub convex_hull_fallback_count: usize,
    pub multiple_loop_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExtractedFloorRegions {
    pub elevation_meters: f32,
    pub regions: Vec<ExtractedRegion>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExtractedRegion {
    pub key: String,
    pub display_label: String,
    pub outline: NavigationPolygon2d,
    pub used_convex_hull_fallback: bool,
    pub centroid_xz: Vec2,
}

pub fn cluster_walkable_triangles_by_elevation(
    triangles: &[LocalTriangle3d],
    config: &RegionGeneratorConfig,
) -> (Vec<f32>, RegionExtractionStats) {
    let mut stats = RegionExtractionStats {
        source_triangle_count: triangles.len(),
        ..Default::default()
    };
    let mut elevations = Vec::new();
    for tri in triangles {
        if tri.normal().y < config.walkable_normal_min_y {
            stats.steep_triangle_discarded += 1;
            continue;
        }
        stats.walkable_triangle_count += 1;
        elevations.push(tri.centroid().y);
    }
    if elevations.is_empty() {
        return (Vec::new(), stats);
    }
    elevations.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut clusters = Vec::new();
    for y in elevations {
        if let Some(cluster) = clusters
            .iter_mut()
            .find(|c: &&mut f32| (**c - y).abs() <= config.floor_cluster_gap_meters * 0.45)
        {
            *cluster = (*cluster * 0.9) + (y * 0.1);
        } else if let Some(last) = clusters.last_mut() {
            if y - *last >= config.floor_cluster_gap_meters {
                clusters.push(y);
            } else {
                *last = (*last + y) * 0.5;
            }
        } else {
            clusters.push(y);
        }
    }
    stats.floor_cluster_count = clusters.len();
    (clusters, stats)
}

pub fn extract_regions_for_elevation(
    triangles: &[LocalTriangle3d],
    elevation: f32,
    config: &RegionGeneratorConfig,
    region_key_offset: usize,
) -> (ExtractedFloorRegions, RegionExtractionStats) {
    let mut stats = RegionExtractionStats::default();
    let tolerance = config.floor_cluster_gap_meters * 0.45;
    let walkable: Vec<&LocalTriangle3d> = triangles
        .iter()
        .filter(|tri| {
            tri.normal().y >= config.walkable_normal_min_y
                && (tri.centroid().y - elevation).abs() <= tolerance
        })
        .collect();
    stats.walkable_triangle_count = walkable.len();
    stats.source_triangle_count = triangles.len();

    let components = connected_components(&walkable, config.edge_merge_epsilon);
    stats.connected_component_count = components.len();
    let sole_component = components.len() == 1;

    let mut regions = Vec::new();
    for (index, component) in components.into_iter().enumerate() {
        let region_index = region_key_offset + index;
        let key = if sole_component && region_key_offset == 0 {
            "main".to_string()
        } else {
            format!("region_{}", region_index + 1)
        };
        match boundary_outline_from_component(&component, config) {
            Ok(outline) => {
                stats.boundary_edge_count += outline.vertices_xz.len();
                if polygon_area(&outline) < config.min_region_area {
                    stats.regions_discarded += 1;
                    stats.warnings.push(format!(
                        "generator_small_region_discarded: region `{key}` area below minimum"
                    ));
                    continue;
                }
                let centroid = polygon_centroid(&outline);
                regions.push(ExtractedRegion {
                    display_label: if key == "main" {
                        "Main".into()
                    } else {
                        format!("Region {}", region_index + 1)
                    },
                    key,
                    outline,
                    used_convex_hull_fallback: false,
                    centroid_xz: centroid,
                });
            }
            Err(reason) => {
                if reason.contains("multiple_boundary_loops") {
                    stats.multiple_loop_count += 1;
                    stats
                        .warnings
                        .push(format!("generator_hole_not_supported: {reason}"));
                } else {
                    stats.warnings.push(reason.clone());
                }
                if let Some(fallback) = convex_hull_fallback(&component, config) {
                    stats.convex_hull_fallback_count += 1;
                    stats.warnings.push(format!(
                        "generator_convex_hull_fallback: region `{key}` used low-confidence hull"
                    ));
                    let centroid = polygon_centroid(&fallback);
                    regions.push(ExtractedRegion {
                        display_label: format!("Region {} (fallback)", region_index + 1),
                        key,
                        outline: fallback,
                        used_convex_hull_fallback: true,
                        centroid_xz: centroid,
                    });
                } else {
                    stats.regions_discarded += 1;
                }
            }
        }
    }
    stats.regions_emitted = regions.len();
    (
        ExtractedFloorRegions {
            elevation_meters: elevation,
            regions,
        },
        stats,
    )
}

fn connected_components<'a>(
    triangles: &[&'a LocalTriangle3d],
    epsilon: f32,
) -> Vec<Vec<&'a LocalTriangle3d>> {
    if triangles.is_empty() {
        return Vec::new();
    }
    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); triangles.len()];
    for i in 0..triangles.len() {
        for j in (i + 1)..triangles.len() {
            if triangles_share_edge(triangles[i], triangles[j], epsilon) {
                adjacency[i].push(j);
                adjacency[j].push(i);
            }
        }
    }
    let mut visited = vec![false; triangles.len()];
    let mut components = Vec::new();
    for start in 0..triangles.len() {
        if visited[start] {
            continue;
        }
        let mut stack = vec![start];
        let mut component = Vec::new();
        visited[start] = true;
        while let Some(index) = stack.pop() {
            component.push(triangles[index]);
            for &next in &adjacency[index] {
                if !visited[next] {
                    visited[next] = true;
                    stack.push(next);
                }
            }
        }
        components.push(component);
    }
    components.sort_by(|a, b| {
        let ca = component_centroid(a);
        let cb = component_centroid(b);
        ca.x.partial_cmp(&cb.x)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| ca.y.partial_cmp(&cb.y).unwrap_or(std::cmp::Ordering::Equal))
    });
    components
}

fn component_centroid(component: &[&LocalTriangle3d]) -> Vec2 {
    let mut sum = Vec2::ZERO;
    let mut count = 0usize;
    for tri in component {
        let c = tri.centroid();
        sum += Vec2::new(c.x, c.z);
        count += 1;
    }
    if count == 0 {
        Vec2::ZERO
    } else {
        sum / count as f32
    }
}

fn triangles_share_edge(a: &LocalTriangle3d, b: &LocalTriangle3d, epsilon: f32) -> bool {
    let edges_a = triangle_edges_xz(a);
    let edges_b = triangle_edges_xz(b);
    edges_a
        .iter()
        .any(|ea| edges_b.iter().any(|eb| edges_equal_xz(*ea, *eb, epsilon)))
}

fn triangle_edges_xz(tri: &LocalTriangle3d) -> [(Vec2, Vec2); 3] {
    let verts = [
        Vec2::new(tri.a.x, tri.a.z),
        Vec2::new(tri.b.x, tri.b.z),
        Vec2::new(tri.c.x, tri.c.z),
    ];
    [
        (verts[0], verts[1]),
        (verts[1], verts[2]),
        (verts[2], verts[0]),
    ]
}

fn edges_equal_xz(a: (Vec2, Vec2), b: (Vec2, Vec2), epsilon: f32) -> bool {
    (a.0.distance(b.0) <= epsilon && a.1.distance(b.1) <= epsilon)
        || (a.0.distance(b.1) <= epsilon && a.1.distance(b.0) <= epsilon)
}

fn edge_key(a: Vec2, b: Vec2, epsilon: f32) -> (i64, i64, i64, i64) {
    let qa = quantize(a, epsilon);
    let qb = quantize(b, epsilon);
    if qa <= qb {
        (qa.0, qa.1, qb.0, qb.1)
    } else {
        (qb.0, qb.1, qa.0, qa.1)
    }
}

fn quantize(v: Vec2, epsilon: f32) -> (i64, i64) {
    let scale = 1.0 / epsilon.max(1e-6);
    ((v.x * scale).round() as i64, (v.y * scale).round() as i64)
}

fn boundary_outline_from_component(
    component: &[&LocalTriangle3d],
    config: &RegionGeneratorConfig,
) -> Result<NavigationPolygon2d, String> {
    let mut edge_multiplicity: BTreeMap<(i64, i64, i64, i64), (Vec2, Vec2)> = BTreeMap::new();
    for tri in component {
        for (a, b) in triangle_edges_xz(tri) {
            let key = edge_key(a, b, config.edge_merge_epsilon);
            edge_multiplicity
                .entry(key)
                .and_modify(|_| {})
                .or_insert((a, b));
        }
    }
    let mut counts: BTreeMap<(i64, i64, i64, i64), u32> = BTreeMap::new();
    for tri in component {
        for (a, b) in triangle_edges_xz(tri) {
            let key = edge_key(a, b, config.edge_merge_epsilon);
            *counts.entry(key).or_insert(0) += 1;
        }
    }
    let mut boundary: Vec<(Vec2, Vec2)> = Vec::new();
    for (key, edge) in edge_multiplicity {
        if counts.get(&key).copied().unwrap_or(0) == 1 {
            boundary.push(edge);
        }
    }
    if boundary.is_empty() {
        return Err("no boundary edges found for walkable component".into());
    }
    let loops = order_boundary_loops(&boundary, config.edge_merge_epsilon)?;
    if loops.len() != 1 {
        return Err(format!(
            "multiple_boundary_loops: found {} loops (holes not supported)",
            loops.len()
        ));
    }
    let mut vertices = loops[0].clone();
    vertices = simplify_collinear(&vertices, config.simplify_epsilon);
    if vertices.len() < 3 {
        return Err("boundary loop degenerate after simplification".into());
    }
    if signed_area(&vertices) < 0.0 {
        vertices.reverse();
    }
    Ok(NavigationPolygon2d {
        vertices_xz: vertices.iter().map(|v| [v.x, v.y]).collect(),
    })
}

fn order_boundary_loops(edges: &[(Vec2, Vec2)], epsilon: f32) -> Result<Vec<Vec<Vec2>>, String> {
    let mut remaining: Vec<(Vec2, Vec2)> = edges.to_vec();
    let mut loops = Vec::new();
    while !remaining.is_empty() {
        let start = remaining.remove(0);
        let mut loop_vertices = vec![start.0, start.1];
        let mut current = start.1;
        let mut guard = 0usize;
        while guard < edges.len() * 4 {
            guard += 1;
            if current.distance(loop_vertices[0]) <= epsilon && loop_vertices.len() >= 3 {
                loop_vertices.pop();
                break;
            }
            let next_index = remaining.iter().position(|(a, b)| {
                a.distance(current) <= epsilon || b.distance(current) <= epsilon
            });
            let Some(index) = next_index else {
                break;
            };
            let (a, b) = remaining.remove(index);
            current = if a.distance(current) <= epsilon { b } else { a };
            if loop_vertices
                .last()
                .is_some_and(|last| last.distance(current) <= epsilon)
            {
                continue;
            }
            loop_vertices.push(current);
        }
        if loop_vertices.len() >= 3 {
            loops.push(loop_vertices);
        }
    }
    if loops.is_empty() {
        return Err("failed to order boundary loop".into());
    }
    Ok(loops)
}

fn convex_hull_fallback(
    component: &[&LocalTriangle3d],
    config: &RegionGeneratorConfig,
) -> Option<NavigationPolygon2d> {
    let mut points = Vec::new();
    for tri in component {
        for v in [tri.a, tri.b, tri.c] {
            points.push(Vec2::new(v.x, v.z));
        }
    }
    let hull = convex_hull(&points);
    let simplified = simplify_collinear(&hull, config.simplify_epsilon);
    if simplified.len() < 3 {
        return None;
    }
    Some(NavigationPolygon2d {
        vertices_xz: simplified.iter().map(|p| [p.x, p.y]).collect(),
    })
}

pub fn convex_hull(points: &[Vec2]) -> Vec<Vec2> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut pts: Vec<Vec2> = points.to_vec();
    pts.sort_by(|a, b| {
        a.x.partial_cmp(&b.x)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
    });
    pts.dedup_by(|a, b| a.distance(*b) < 0.01);
    if pts.len() < 3 {
        return pts;
    }
    let cross = |o: Vec2, a: Vec2, b: Vec2| (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
    let mut lower = Vec::new();
    for p in &pts {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], *p) <= 0.0 {
            lower.pop();
        }
        lower.push(*p);
    }
    let mut upper = Vec::new();
    for p in pts.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], *p) <= 0.0 {
            upper.pop();
        }
        upper.push(*p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

pub fn simplify_collinear(points: &[Vec2], epsilon: f32) -> Vec<Vec2> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut out = Vec::new();
    for i in 0..points.len() {
        let prev = points[(i + points.len() - 1) % points.len()];
        let curr = points[i];
        let next = points[(i + 1) % points.len()];
        let v1 = (curr - prev).normalize_or_zero();
        let v2 = (next - curr).normalize_or_zero();
        if v1.distance(v2) > epsilon * 0.1 {
            out.push(curr);
        }
    }
    if out.len() < 3 { points.to_vec() } else { out }
}

pub fn polygon_area(polygon: &NavigationPolygon2d) -> f32 {
    signed_area(
        &polygon
            .vertices_xz
            .iter()
            .map(|&[x, z]| Vec2::new(x, z))
            .collect::<Vec<_>>(),
    )
    .abs()
}

fn signed_area(points: &[Vec2]) -> f32 {
    if points.len() < 3 {
        return 0.0;
    }
    let mut area = 0.0_f32;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        area += a.x * b.y - b.x * a.y;
    }
    area * 0.5
}

pub fn polygon_centroid(polygon: &NavigationPolygon2d) -> Vec2 {
    let verts: Vec<Vec2> = polygon
        .vertices_xz
        .iter()
        .map(|&[x, z]| Vec2::new(x, z))
        .collect();
    if verts.is_empty() {
        return Vec2::ZERO;
    }
    verts.iter().fold(Vec2::ZERO, |acc, v| acc + *v) / verts.len() as f32
}

pub fn point_in_polygon(point: Vec2, polygon: &NavigationPolygon2d) -> bool {
    let verts: Vec<Vec2> = polygon
        .vertices_xz
        .iter()
        .map(|&[x, z]| Vec2::new(x, z))
        .collect();
    if verts.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = verts.len() - 1;
    for (i, vertex) in verts.iter().enumerate() {
        let vi = *vertex;
        let vj = verts[j];
        if ((vi.y > point.y) != (vj.y > point.y))
            && (point.x < (vj.x - vi.x) * (point.y - vi.y) / (vj.y - vi.y + f32::EPSILON) + vi.x)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

pub fn region_definitions_from_extracted(
    regions: &[ExtractedRegion],
) -> Vec<NavigationRegionDefinition> {
    regions
        .iter()
        .map(|region| NavigationRegionDefinition {
            key: region.key.clone(),
            display_label: region.display_label.clone(),
            room_tag: None,
            walkable_outline: region.outline.clone(),
        })
        .collect()
}

pub fn find_containing_regions(point: Vec2, regions: &[ExtractedRegion]) -> Vec<&ExtractedRegion> {
    regions
        .iter()
        .filter(|region| point_in_polygon(point, &region.outline))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect_triangles(x0: f32, z0: f32, x1: f32, z1: f32, y: f32) -> Vec<LocalTriangle3d> {
        let p = |x: f32, z: f32| Vec3::new(x, y, z);
        vec![
            LocalTriangle3d {
                a: p(x0, z0),
                b: p(x0, z1),
                c: p(x1, z1),
            },
            LocalTriangle3d {
                a: p(x0, z0),
                b: p(x1, z1),
                c: p(x1, z0),
            },
        ]
    }

    fn l_shape_triangles() -> Vec<LocalTriangle3d> {
        let y = 0.0;
        let p = |x: f32, z: f32| Vec3::new(x, y, z);
        vec![
            LocalTriangle3d {
                a: p(0.0, 0.0),
                b: p(8.0, 3.0),
                c: p(8.0, 0.0),
            },
            LocalTriangle3d {
                a: p(0.0, 0.0),
                b: p(3.0, 3.0),
                c: p(8.0, 3.0),
            },
            LocalTriangle3d {
                a: p(0.0, 0.0),
                b: p(0.0, 8.0),
                c: p(3.0, 3.0),
            },
            LocalTriangle3d {
                a: p(0.0, 8.0),
                b: p(3.0, 3.0),
                c: p(3.0, 8.0),
            },
        ]
    }

    #[test]
    fn l_shape_extracts_single_concave_region() {
        let config = RegionGeneratorConfig::default();
        let tris = l_shape_triangles();
        let (floor, stats) = extract_regions_for_elevation(&tris, 0.0, &config, 0);
        assert_eq!(floor.regions.len(), 1);
        assert!(!floor.regions[0].used_convex_hull_fallback);
        assert_eq!(stats.convex_hull_fallback_count, 0);
        let verts = &floor.regions[0].outline.vertices_xz;
        assert!(verts.len() >= 5);
        // Missing upper-right corner must not be inside.
        assert!(!point_in_polygon(
            Vec2::new(7.0, 7.0),
            &floor.regions[0].outline
        ));
        assert!(point_in_polygon(
            Vec2::new(1.0, 1.0),
            &floor.regions[0].outline
        ));
    }

    #[test]
    fn disconnected_islands_become_two_regions() {
        let config = RegionGeneratorConfig::default();
        let mut tris = rect_triangles(0.0, 0.0, 4.0, 4.0, 0.0);
        tris.extend(rect_triangles(10.0, 0.0, 14.0, 4.0, 0.0));
        let (floor, stats) = extract_regions_for_elevation(&tris, 0.0, &config, 0);
        assert_eq!(floor.regions.len(), 2);
        assert_eq!(stats.connected_component_count, 2);
        let keys: Vec<_> = floor.regions.iter().map(|r| r.key.as_str()).collect();
        assert!(keys.contains(&"region_1"));
        assert!(keys.contains(&"region_2"));
    }

    #[test]
    fn extraction_is_deterministic() {
        let config = RegionGeneratorConfig::default();
        let tris = l_shape_triangles();
        let (a, _) = extract_regions_for_elevation(&tris, 0.0, &config, 0);
        let (b, _) = extract_regions_for_elevation(&tris, 0.0, &config, 0);
        assert_eq!(
            a.regions[0].outline.vertices_xz,
            b.regions[0].outline.vertices_xz
        );
    }

    #[test]
    fn simplify_preserves_l_shape_corner() {
        let config = RegionGeneratorConfig::default();
        let tris = l_shape_triangles();
        let (floor, _) = extract_regions_for_elevation(&tris, 0.0, &config, 0);
        let verts = &floor.regions[0].outline.vertices_xz;
        let has_corner = verts
            .iter()
            .any(|&[x, z]| (x - 3.0).abs() < 0.2 && (z - 3.0).abs() < 0.2);
        assert!(has_corner, "L-shape inner corner should be preserved");
    }

    #[test]
    fn courtyard_ring_reports_hole_not_supported() {
        let mut tris = rect_triangles(0.0, 0.0, 10.0, 1.0, 0.0);
        tris.extend(rect_triangles(0.0, 9.0, 10.0, 10.0, 0.0));
        tris.extend(rect_triangles(0.0, 0.0, 1.0, 10.0, 0.0));
        tris.extend(rect_triangles(9.0, 0.0, 10.0, 10.0, 0.0));
        let config = RegionGeneratorConfig::default();
        let (floor, stats) = extract_regions_for_elevation(&tris, 0.0, &config, 0);
        assert!(stats.multiple_loop_count > 0 || stats.convex_hull_fallback_count > 0);
        assert!(
            stats
                .warnings
                .iter()
                .any(|w| w.contains("generator_hole_not_supported"))
        );
        assert!(
            floor.regions.is_empty() || floor.regions.iter().any(|r| r.used_convex_hull_fallback),
            "courtyard must not silently become a filled walkable polygon"
        );
    }
}
