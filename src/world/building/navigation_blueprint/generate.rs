//! Automatic navigation blueprint generation from building meshes (NV1.2).

use std::path::{Path, PathBuf};

use bevy::prelude::*;

use super::cache::NAVIGATION_BLUEPRINT_GENERATOR_VERSION;
use super::definition::{
    BuildingNavigationBlueprint, BuildingNavigationBlueprintMetadata, NavigationEntranceDefinition,
    NavigationFloorDefinition, NavigationPolygon2d, NavigationVerticalTransitionDefinition,
    NavigationVerticalTransitionKind,
};
use super::id::BuildingNavigationBlueprintId;
use super::mesh::{BuildingMeshAnalysisInput, LocalTriangle3d, PortalMarker3d};
use super::report::{
    EntranceGenerationDiagnostics, NavigationBlueprintGenerationReport,
    NavigationBlueprintGenerationStatus,
};
use crate::world::authoring_transform::BuildingTransformSafetyClass;
use crate::world::building::catalog::BuildingDefinition;
use crate::world::occupancy::bake::source_file_hash_hex;

const WALKABLE_NORMAL_MIN_Y: f32 = 0.72;
const FLOOR_CLUSTER_GAP_METERS: f32 = 2.5;
const MIN_ENTRANCE_RADIUS: f32 = 0.75;
const DEFAULT_ENTRANCE_RADIUS: f32 = 1.5;
const HULL_SIMPLIFY_EPSILON: f32 = 0.15;

/// Input for a single building generation pass.
#[derive(Debug, Clone)]
pub struct NavigationBlueprintGenerateInput {
    pub blueprint_id: BuildingNavigationBlueprintId,
    pub display_name: String,
    pub collision_asset_path: PathBuf,
    pub render_asset_path: Option<PathBuf>,
    pub baseline_scale: f32,
    pub mesh: BuildingMeshAnalysisInput,
}

/// Output of mesh analysis + blueprint synthesis.
#[derive(Debug, Clone)]
pub struct NavigationBlueprintGenerateOutput {
    pub blueprint: BuildingNavigationBlueprint,
    pub warnings: Vec<String>,
    pub entrance_diagnostics: EntranceGenerationDiagnostics,
}

pub fn should_generate_navigation_blueprint(definition: &BuildingDefinition) -> bool {
    definition.transform_safety_class == BuildingTransformSafetyClass::Navigable
}

/// Human-readable rejection when [`should_generate_navigation_blueprint`] is false.
pub fn navigation_blueprint_generation_rejection(
    definition: &BuildingDefinition,
) -> Option<&'static str> {
    if should_generate_navigation_blueprint(definition) {
        None
    } else {
        Some("building is not Navigable")
    }
}

pub fn blueprint_id_for_building(definition: &BuildingDefinition) -> BuildingNavigationBlueprintId {
    if let Some(id) = &definition.navigation_blueprint_id {
        BuildingNavigationBlueprintId::new(id.clone())
    } else if let Some(id) = &definition.interior_profile_id {
        BuildingNavigationBlueprintId::new(id.clone())
    } else {
        BuildingNavigationBlueprintId::new(format!("{}_nav", definition.id.as_str()))
    }
}

pub fn navigation_mesh_source_label(mesh: &BuildingMeshAnalysisInput) -> &'static str {
    if mesh.used_collision_node {
        "occupancy_collision"
    } else {
        "visible GLB geometry fallback"
    }
}

pub fn navigation_mesh_source_display(mesh: &BuildingMeshAnalysisInput) -> String {
    format!(
        "Regeneration source: {}",
        navigation_mesh_source_label(mesh)
    )
}

pub fn generate_navigation_blueprint(
    input: NavigationBlueprintGenerateInput,
) -> Result<NavigationBlueprintGenerateOutput, String> {
    let mut warnings = Vec::new();
    if !input.baseline_scale.is_finite() || input.baseline_scale <= 0.0 {
        return Err("invalid baseline scale for navigation generation".into());
    }

    let scale = input.baseline_scale;
    let triangles = scale_triangles(&input.mesh.triangles, scale);
    let portal_markers = scale_portal_markers(&input.mesh.portal_markers, scale);

    if triangles.is_empty() {
        return Err("mesh contains no triangles after scaling".into());
    }

    if !input.mesh.used_collision_node {
        warnings.push(
            "occupancy_collision node missing — used visible mesh geometry for analysis".into(),
        );
    }

    let walkable_clusters = cluster_walkable_floors(&triangles);
    if walkable_clusters.is_empty() {
        return Err("no walkable horizontal surfaces detected".into());
    }

    let mut floors = build_floor_definitions(&walkable_clusters, &mut warnings);
    assign_floor_ids(&mut floors);

    let mut entrance_diag = EntranceGenerationDiagnostics::default();
    let mut entrances = entrances_from_portal_markers(
        &portal_markers,
        &floors,
        &triangles,
        &mut warnings,
        &mut entrance_diag,
    );
    if entrances.is_empty() {
        if let Some(entrance) = heuristic_ground_entrance(&floors, &mut warnings) {
            entrances.push(entrance);
            entrance_diag.synthesized_entrances = 1;
            entrance_diag
                .candidate_details
                .push("synthesized fallback exterior_entrance (no portal__ markers)".into());
        }
    }
    entrance_diag.entrances_generated = entrances.len();

    let vertical_transitions =
        vertical_transitions_from_portals(&portal_markers, &floors, &mut warnings);

    let render_key = input
        .render_asset_path
        .as_ref()
        .and_then(|path| path.file_stem())
        .and_then(|stem| stem.to_str())
        .map(str::to_string);

    let mut metadata = BuildingNavigationBlueprintMetadata {
        source_render_key: render_key,
        generation_revision: Some(NAVIGATION_BLUEPRINT_GENERATOR_VERSION),
        ..Default::default()
    };
    metadata.extensions.insert(
        "nv12_collision_path".into(),
        input.collision_asset_path.display().to_string(),
    );
    if input.mesh.used_collision_node {
        metadata
            .extensions
            .insert("nv12_mesh_source".into(), "occupancy_collision".into());
    } else {
        metadata
            .extensions
            .insert("nv12_mesh_source".into(), "visible_meshes".into());
    }

    let blueprint = BuildingNavigationBlueprint {
        id: input.blueprint_id.clone(),
        display_name: input.display_name,
        schema_version: super::definition::BUILDING_NAVIGATION_BLUEPRINT_SCHEMA_VERSION,
        metadata,
        floors,
        entrances,
        vertical_transitions,
        enabled: true,
    };

    blueprint
        .validate()
        .map_err(|err| format!("generated blueprint failed validation: {err}"))?;

    if blueprint.entrances.is_empty() {
        warnings.push("no entrances generated — manual authoring required".into());
    }

    Ok(NavigationBlueprintGenerateOutput {
        blueprint,
        warnings,
        entrance_diagnostics: entrance_diag,
    })
}

pub fn hash_asset_path(path: &Path) -> Option<String> {
    source_file_hash_hex(path).ok()
}

pub fn failed_report(
    building_id: &str,
    blueprint_id: BuildingNavigationBlueprintId,
    error: impl Into<String>,
) -> NavigationBlueprintGenerationReport {
    NavigationBlueprintGenerationReport {
        building_id: building_id.to_string(),
        blueprint_id,
        status: NavigationBlueprintGenerationStatus::Failed,
        mesh_source_label: None,
        warnings: Vec::new(),
        errors: vec![error.into()],
        entrance_diagnostics: EntranceGenerationDiagnostics::default(),
    }
}

fn scale_triangles(triangles: &[LocalTriangle3d], scale: f32) -> Vec<LocalTriangle3d> {
    triangles
        .iter()
        .map(|tri| LocalTriangle3d {
            a: tri.a * scale,
            b: tri.b * scale,
            c: tri.c * scale,
        })
        .collect()
}

fn scale_portal_markers(markers: &[PortalMarker3d], scale: f32) -> Vec<PortalMarker3d> {
    markers
        .iter()
        .map(|marker| PortalMarker3d {
            name: marker.name.clone(),
            position: marker.position * scale,
            scene_path: marker.scene_path.clone(),
        })
        .collect()
}

#[derive(Debug, Clone)]
struct WalkableCluster {
    elevation: f32,
    points_xz: Vec<Vec2>,
}

fn cluster_walkable_floors(triangles: &[LocalTriangle3d]) -> Vec<WalkableCluster> {
    let mut samples: Vec<(f32, Vec2)> = Vec::new();
    for tri in triangles {
        let normal = tri.normal();
        if normal.y < WALKABLE_NORMAL_MIN_Y {
            continue;
        }
        let centroid = tri.centroid();
        samples.push((centroid.y, Vec2::new(centroid.x, centroid.z)));
        for v in [tri.a, tri.b, tri.c] {
            samples.push((v.y, Vec2::new(v.x, v.z)));
        }
    }
    if samples.is_empty() {
        return Vec::new();
    }
    samples.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut clusters: Vec<WalkableCluster> = Vec::new();
    for (y, point) in samples {
        if let Some(cluster) = clusters
            .iter_mut()
            .find(|c| (c.elevation - y).abs() <= FLOOR_CLUSTER_GAP_METERS * 0.45)
        {
            cluster.points_xz.push(point);
            cluster.elevation = (cluster.elevation * 0.9) + (y * 0.1);
        } else if let Some(cluster) = clusters.last_mut() {
            if y - cluster.elevation >= FLOOR_CLUSTER_GAP_METERS {
                clusters.push(WalkableCluster {
                    elevation: y,
                    points_xz: vec![point],
                });
            } else {
                cluster.points_xz.push(point);
                cluster.elevation = (cluster.elevation + y) * 0.5;
            }
        } else {
            clusters.push(WalkableCluster {
                elevation: y,
                points_xz: vec![point],
            });
        }
    }
    clusters
}

fn build_floor_definitions(
    clusters: &[WalkableCluster],
    warnings: &mut Vec<String>,
) -> Vec<NavigationFloorDefinition> {
    let mut floors = Vec::new();
    for (index, cluster) in clusters.iter().enumerate() {
        let key = format!("floor_{index}");
        let hull = convex_hull(&cluster.points_xz);
        let simplified = simplify_collinear(&hull, HULL_SIMPLIFY_EPSILON);
        if simplified.len() < 3 {
            warnings.push(format!(
                "floor `{key}` at y={:.2} produced degenerate outline — skipped",
                cluster.elevation
            ));
            continue;
        }
        let outline = NavigationPolygon2d {
            vertices_xz: simplified.iter().map(|p| [p.x, p.y]).collect(),
        };
        floors.push(NavigationFloorDefinition {
            floor_id: index as i32,
            key,
            display_label: format!("Floor {:.1}m", cluster.elevation),
            elevation_meters: cluster.elevation,
            visibility_group_id: (index + 1) as u32,
            room_tag: None,
            walkable_outline: outline,
        });
    }
    floors
}

fn assign_floor_ids(floors: &mut [NavigationFloorDefinition]) {
    if floors.is_empty() {
        return;
    }
    floors.sort_by(|a, b| {
        a.elevation_meters
            .partial_cmp(&b.elevation_meters)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let has_basement = floors.first().is_some_and(|f| f.elevation_meters < -0.5);
    for (index, floor) in floors.iter_mut().enumerate() {
        floor.floor_id = if has_basement {
            index as i32 - 1
        } else {
            index as i32
        };
        floor.key = format!("floor_{}", floor.floor_id);
    }
}

/// Markers closer than this (meters) on the same floor with overlapping discs merge.
const ENTRANCE_DEDUP_DISTANCE_METERS: f32 = 0.45;

fn entrances_from_portal_markers(
    markers: &[PortalMarker3d],
    floors: &[NavigationFloorDefinition],
    triangles: &[LocalTriangle3d],
    warnings: &mut Vec<String>,
    diag: &mut EntranceGenerationDiagnostics,
) -> Vec<NavigationEntranceDefinition> {
    let ground = floors.iter().min_by(|a, b| {
        a.elevation_meters
            .partial_cmp(&b.elevation_meters)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let Some(ground) = ground else {
        return Vec::new();
    };

    let entrance_markers: Vec<&PortalMarker3d> = markers
        .iter()
        .filter(|marker| portal_kind(&marker.name).is_entrance())
        .collect();
    diag.explicit_markers = entrance_markers.len();
    if entrance_markers.is_empty() {
        return Vec::new();
    }

    for marker in &entrance_markers {
        diag.candidate_details.push(format!(
            "{} @ [{:.2},{:.2}] path={} ({})",
            marker.name,
            marker.position.x,
            marker.position.z,
            if marker.scene_path.is_empty() {
                "-"
            } else {
                marker.scene_path.as_str()
            },
            portal_role_label(&marker.name),
        ));
    }

    let groups = group_portal_entrance_markers(&entrance_markers);
    diag.deduplicated_candidates = entrance_markers.len().saturating_sub(groups.len());

    let mut entrances = Vec::new();
    for (group_index, group) in groups.into_iter().enumerate() {
        let exterior_marker = group
            .outside
            .or(group.root)
            .or(group.inside)
            .expect("group has at least one marker");
        let floor = nearest_floor(floors, exterior_marker.position.y).unwrap_or(ground);
        let centroid = floor_centroid(floor);
        let exterior = Vec2::new(exterior_marker.position.x, exterior_marker.position.z);
        let interior = if let Some(inside) = group.inside {
            Vec3::new(inside.position.x, floor.elevation_meters, inside.position.z)
        } else {
            Vec3::new(
                exterior.x + (centroid.x - exterior.x) * 0.35,
                floor.elevation_meters,
                exterior.y + (centroid.y - exterior.y) * 0.35,
            )
        };
        let radius = DEFAULT_ENTRANCE_RADIUS;
        if !point_inside_floor(floor, exterior) && !near_mesh_boundary(triangles, exterior) {
            warnings.push(format!(
                "portal `{}` is not near a walkable boundary — using marker position",
                exterior_marker.name
            ));
        }
        let key = portal_key_suffix(&group.logical_key)
            .unwrap_or_else(|| format!("entrance_{group_index}"));
        entrances.push(NavigationEntranceDefinition {
            key,
            floor_key: floor.key.clone(),
            local_position_xz: [exterior.x, exterior.y],
            radius_meters: radius.max(MIN_ENTRANCE_RADIUS),
            interior_spawn_local: [interior.x, interior.y, interior.z],
            bidirectional: true,
        });
    }

    let before_spatial = entrances.len();
    entrances = dedupe_nearby_entrances(entrances);
    diag.deduplicated_candidates += before_spatial.saturating_sub(entrances.len());
    entrances
}

#[derive(Debug, Clone)]
struct PortalEntranceGroup<'a> {
    logical_key: String,
    root: Option<&'a PortalMarker3d>,
    outside: Option<&'a PortalMarker3d>,
    inside: Option<&'a PortalMarker3d>,
}

fn group_portal_entrance_markers<'a>(
    markers: &[&'a PortalMarker3d],
) -> Vec<PortalEntranceGroup<'a>> {
    let mut groups: Vec<PortalEntranceGroup<'a>> = Vec::new();
    for marker in markers {
        let key = logical_portal_group_key(&marker.name);
        let slot = groups.iter_mut().find(|g| g.logical_key == key);
        let group = if let Some(group) = slot {
            group
        } else {
            groups.push(PortalEntranceGroup {
                logical_key: key,
                root: None,
                outside: None,
                inside: None,
            });
            groups.last_mut().unwrap()
        };
        match portal_role(&marker.name) {
            PortalRole::Outside => group.outside = Some(marker),
            PortalRole::Inside => group.inside = Some(marker),
            PortalRole::Root => group.root = Some(marker),
        }
    }
    groups
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PortalRole {
    Root,
    Outside,
    Inside,
}

fn portal_role(name: &str) -> PortalRole {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with("__outside") || lower.ends_with("__exterior") || lower.ends_with("__out") {
        PortalRole::Outside
    } else if lower.ends_with("__inside")
        || lower.ends_with("__interior")
        || lower.ends_with("__in")
    {
        PortalRole::Inside
    } else {
        PortalRole::Root
    }
}

fn portal_role_label(name: &str) -> &'static str {
    match portal_role(name) {
        PortalRole::Outside => "explicit portal outside",
        PortalRole::Inside => "explicit portal inside",
        PortalRole::Root => "explicit portal marker",
    }
}

/// Strip role suffixes so `portal__entrance`, `portal__entrance__outside`, and
/// `portal__entrance__inside` share one logical doorway identity.
pub fn logical_portal_group_key(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    let rest = lower.strip_prefix("portal__").unwrap_or(&lower);
    let rest = rest
        .strip_suffix("__outside")
        .or_else(|| rest.strip_suffix("__exterior"))
        .or_else(|| rest.strip_suffix("__inside"))
        .or_else(|| rest.strip_suffix("__interior"))
        .or_else(|| rest.strip_suffix("__out"))
        .or_else(|| rest.strip_suffix("__in"))
        .unwrap_or(rest);
    format!("portal__{rest}")
}

fn dedupe_nearby_entrances(
    mut entrances: Vec<NavigationEntranceDefinition>,
) -> Vec<NavigationEntranceDefinition> {
    if entrances.len() < 2 {
        return entrances;
    }
    let mut keep = vec![true; entrances.len()];
    for i in 0..entrances.len() {
        if !keep[i] {
            continue;
        }
        for j in (i + 1)..entrances.len() {
            if !keep[j] {
                continue;
            }
            if entrances[i].floor_key != entrances[j].floor_key {
                continue;
            }
            let a = Vec2::new(
                entrances[i].local_position_xz[0],
                entrances[i].local_position_xz[1],
            );
            let b = Vec2::new(
                entrances[j].local_position_xz[0],
                entrances[j].local_position_xz[1],
            );
            let dist = a.distance(b);
            let overlap = dist
                <= ENTRANCE_DEDUP_DISTANCE_METERS
                    .max(entrances[i].radius_meters.min(entrances[j].radius_meters) * 0.35);
            if overlap {
                // Prefer the earlier (group) entrance; drop the duplicate.
                keep[j] = false;
            }
        }
    }
    entrances
        .into_iter()
        .zip(keep)
        .filter_map(|(entrance, kept)| kept.then_some(entrance))
        .collect()
}

fn vertical_transitions_from_portals(
    markers: &[PortalMarker3d],
    floors: &[NavigationFloorDefinition],
    warnings: &mut Vec<String>,
) -> Vec<NavigationVerticalTransitionDefinition> {
    let mut transitions = Vec::new();
    for (index, marker) in markers.iter().enumerate() {
        let kind = portal_kind(&marker.name);
        let transition_kind = match kind {
            PortalKind::Stair => NavigationVerticalTransitionKind::Stair,
            PortalKind::Ramp => NavigationVerticalTransitionKind::Ramp,
            PortalKind::Ladder => NavigationVerticalTransitionKind::Ladder,
            _ => continue,
        };
        let (from_floor, to_floor) = match (
            nearest_floor(floors, marker.position.y - 0.5),
            nearest_floor(floors, marker.position.y + 0.5),
        ) {
            (Some(from), Some(to)) if from.key != to.key => (from, to),
            _ => {
                if floors.len() < 2 {
                    warnings.push(format!(
                        "portal `{}` suggests vertical transition but only one floor exists",
                        marker.name
                    ));
                }
                continue;
            }
        };
        transitions.push(NavigationVerticalTransitionDefinition {
            key: portal_key_suffix(&marker.name).unwrap_or_else(|| format!("transition_{index}")),
            kind: transition_kind,
            from_floor_key: from_floor.key.clone(),
            to_floor_key: to_floor.key.clone(),
            from_local_position_xz: [marker.position.x, marker.position.z],
            from_radius_meters: DEFAULT_ENTRANCE_RADIUS,
            to_local_position: [
                marker.position.x,
                to_floor.elevation_meters,
                marker.position.z,
            ],
            bidirectional: true,
        });
    }
    transitions
}

fn heuristic_ground_entrance(
    floors: &[NavigationFloorDefinition],
    warnings: &mut Vec<String>,
) -> Option<NavigationEntranceDefinition> {
    let ground = floors.iter().min_by(|a, b| {
        a.elevation_meters
            .partial_cmp(&b.elevation_meters)
            .unwrap_or(std::cmp::Ordering::Equal)
    })?;
    let vertices: Vec<Vec2> = ground
        .walkable_outline
        .vertices_xz
        .iter()
        .map(|[x, z]| Vec2::new(*x, *z))
        .collect();
    if vertices.len() < 2 {
        return None;
    }
    let mut best_edge = (0usize, f32::MAX);
    for i in 0..vertices.len() {
        let a = vertices[i];
        let b = vertices[(i + 1) % vertices.len()];
        let mid = (a + b) * 0.5;
        if mid.y < best_edge.1 {
            best_edge = (i, mid.y);
        }
    }
    let a = vertices[best_edge.0];
    let b = vertices[(best_edge.0 + 1) % vertices.len()];
    let mid = (a + b) * 0.5;
    let edge_len = a.distance(b);
    let radius = (edge_len * 0.35).clamp(MIN_ENTRANCE_RADIUS, 2.5);
    let centroid = floor_centroid(ground);
    warnings.push("no portal__ markers found — synthesized entrance from floor outline".into());
    Some(NavigationEntranceDefinition {
        key: "exterior_entrance".to_string(),
        floor_key: ground.key.clone(),
        local_position_xz: [mid.x, mid.y],
        radius_meters: radius,
        interior_spawn_local: [
            mid.x + (centroid.x - mid.x) * 0.4,
            ground.elevation_meters,
            mid.y + (centroid.y - mid.y) * 0.4,
        ],
        bidirectional: true,
    })
}

fn floor_centroid(floor: &NavigationFloorDefinition) -> Vec2 {
    let verts = &floor.walkable_outline.vertices_xz;
    if verts.is_empty() {
        return Vec2::ZERO;
    }
    let sum = verts
        .iter()
        .fold(Vec2::ZERO, |acc, [x, z]| acc + Vec2::new(*x, *z));
    sum / verts.len() as f32
}

fn nearest_floor<'a>(
    floors: &'a [NavigationFloorDefinition],
    y: f32,
) -> Option<&'a NavigationFloorDefinition> {
    floors.iter().min_by(|a, b| {
        (a.elevation_meters - y)
            .abs()
            .partial_cmp(&(b.elevation_meters - y).abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

fn point_inside_floor(floor: &NavigationFloorDefinition, point: Vec2) -> bool {
    let verts: Vec<Vec2> = floor
        .walkable_outline
        .vertices_xz
        .iter()
        .map(|[x, z]| Vec2::new(*x, *z))
        .collect();
    point_in_polygon(point, &verts)
}

fn near_mesh_boundary(triangles: &[LocalTriangle3d], point: Vec2) -> bool {
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for tri in triangles {
        for v in [tri.a, tri.b, tri.c] {
            let p = Vec2::new(v.x, v.z);
            min = min.min(p);
            max = max.max(p);
        }
    }
    let margin = 0.35;
    point.x <= min.x + margin
        || point.x >= max.x - margin
        || point.y <= min.y + margin
        || point.y >= max.y - margin
}

fn point_in_polygon(point: Vec2, polygon: &[Vec2]) -> bool {
    let mut inside = false;
    let mut j = polygon.len().wrapping_sub(1);
    for (i, vertex) in polygon.iter().enumerate() {
        let pi = *vertex;
        let pj = polygon[j];
        if ((pi.y > point.y) != (pj.y > point.y))
            && (point.x < (pj.x - pi.x) * (point.y - pi.y) / (pj.y - pi.y + f32::EPSILON) + pi.x)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PortalKind {
    Entrance,
    Stair,
    Ramp,
    Ladder,
    Other,
}

impl PortalKind {
    fn is_entrance(self) -> bool {
        matches!(self, Self::Entrance | Self::Other)
    }
}

fn portal_kind(name: &str) -> PortalKind {
    let lower = name.to_ascii_lowercase();
    if lower.contains("stair") {
        PortalKind::Stair
    } else if lower.contains("ramp") {
        PortalKind::Ramp
    } else if lower.contains("ladder") {
        PortalKind::Ladder
    } else if lower.contains("entrance") || lower.contains("door") {
        PortalKind::Entrance
    } else {
        PortalKind::Other
    }
}

fn portal_key_suffix(name: &str) -> Option<String> {
    name.split_once("__").map(|(_, suffix)| suffix.to_string())
}

fn convex_hull(points: &[Vec2]) -> Vec<Vec2> {
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

fn simplify_collinear(points: &[Vec2], epsilon: f32) -> Vec<Vec2> {
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
        if v1.distance(v2) > epsilon {
            out.push(curr);
        }
    }
    if out.len() < 3 { points.to_vec() } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::building::navigation_blueprint::mesh::load_building_mesh_for_navigation;

    #[test]
    fn navigable_building_without_profile_or_blueprint_id_is_eligible() {
        use crate::world::authoring_transform::BuildingTransformSafetyClass;
        use crate::world::building::catalog::BuildingDefinitionId;
        use crate::world::building::footprint::FootprintSpec;

        let definition = BuildingDefinition::new(
            BuildingDefinitionId::new("hut"),
            "Hut",
            crate::world::BuildingCategoryId::new("residential"),
            crate::world::BuildingRenderKey::reserved("hut"),
            crate::world::BuildingRenderKey::reserved("hut_collision"),
            100,
            10.0,
            FootprintSpec::Rectangle {
                width_meters: 4.0,
                depth_meters: 4.0,
            },
            35.0,
            true,
        );
        assert_eq!(
            definition.transform_safety_class,
            BuildingTransformSafetyClass::Navigable
        );
        assert!(definition.interior_profile_id.is_none());
        assert!(definition.navigation_blueprint_id.is_none());
        assert!(should_generate_navigation_blueprint(&definition));
        assert!(navigation_blueprint_generation_rejection(&definition).is_none());
    }

    #[test]
    fn decorative_building_is_not_eligible_for_generation() {
        use crate::world::authoring_transform::BuildingTransformSafetyClass;
        use crate::world::building::catalog::BuildingDefinitionId;
        use crate::world::building::footprint::FootprintSpec;

        let mut definition = BuildingDefinition::new(
            BuildingDefinitionId::new("prop"),
            "Prop",
            crate::world::BuildingCategoryId::new("residential"),
            crate::world::BuildingRenderKey::reserved("hut"),
            crate::world::BuildingRenderKey::reserved("hut_collision"),
            100,
            10.0,
            FootprintSpec::Rectangle {
                width_meters: 4.0,
                depth_meters: 4.0,
            },
            35.0,
            true,
        );
        definition.transform_safety_class = BuildingTransformSafetyClass::DecorativeNonNavigable;
        assert!(!should_generate_navigation_blueprint(&definition));
        assert_eq!(
            navigation_blueprint_generation_rejection(&definition),
            Some("building is not Navigable")
        );
    }

    #[test]
    fn navigation_mesh_source_label_prefers_collision_node() {
        use super::super::mesh::BuildingMeshAnalysisInput;
        let mut mesh = BuildingMeshAnalysisInput::default();
        mesh.used_collision_node = true;
        assert_eq!(navigation_mesh_source_label(&mesh), "occupancy_collision");
        mesh.used_collision_node = false;
        assert_eq!(
            navigation_mesh_source_label(&mesh),
            "visible GLB geometry fallback"
        );
    }

    #[test]
    fn convex_hull_rectangle() {
        let points = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(4.0, 0.0),
            Vec2::new(4.0, 4.0),
            Vec2::new(0.0, 4.0),
            Vec2::new(2.0, 2.0),
        ];
        let hull = convex_hull(&points);
        assert!(hull.len() >= 4);
    }

    #[test]
    fn generate_uses_mesh_geometry_not_saved_blueprint_outline() {
        // A flat walkable quad at y=0 — generator must slice this mesh, not any
        // pre-existing blueprint polygon.
        let triangles = vec![
            LocalTriangle3d {
                a: Vec3::new(0.0, 0.0, 0.0),
                b: Vec3::new(6.0, 0.0, 6.0),
                c: Vec3::new(6.0, 0.0, 0.0),
            },
            LocalTriangle3d {
                a: Vec3::new(0.0, 0.0, 0.0),
                b: Vec3::new(0.0, 0.0, 6.0),
                c: Vec3::new(6.0, 0.0, 6.0),
            },
        ];
        let mesh = BuildingMeshAnalysisInput {
            triangles,
            portal_markers: Vec::new(),
            source_path: "synthetic".into(),
            used_collision_node: true,
        };
        assert_eq!(navigation_mesh_source_label(&mesh), "occupancy_collision");

        let output = generate_navigation_blueprint(NavigationBlueprintGenerateInput {
            blueprint_id: BuildingNavigationBlueprintId::new("synthetic_nav"),
            display_name: "Synthetic".into(),
            collision_asset_path: PathBuf::from("synthetic.glb"),
            render_asset_path: None,
            baseline_scale: 1.0,
            mesh,
        })
        .expect("mesh slice");

        assert_eq!(
            output
                .blueprint
                .metadata
                .extensions
                .get("nv12_mesh_source")
                .map(String::as_str),
            Some("occupancy_collision")
        );
        assert!(!output.blueprint.floors.is_empty());
        // Hull of the 6×6 quad should span near that extent — not a tiny saved outline.
        let outline = &output.blueprint.floors[0].walkable_outline.vertices_xz;
        let max_x = outline
            .iter()
            .map(|v| v[0])
            .fold(f32::NEG_INFINITY, f32::max);
        let max_z = outline
            .iter()
            .map(|v| v[1])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(max_x > 4.0, "expected mesh extent, got max_x={max_x}");
        assert!(max_z > 4.0, "expected mesh extent, got max_z={max_z}");
    }

    #[test]
    fn generates_blueprint_for_hut_when_asset_present() {
        let path = PathBuf::from("assets/buildings/hut.glb");
        if !path.is_file() {
            return;
        }
        let mesh = load_building_mesh_for_navigation(&path).expect("mesh");
        // Hut authors one logical door as portal__entrance + __outside + __inside.
        assert_eq!(mesh.portal_markers.len(), 3);
        let output = generate_navigation_blueprint(NavigationBlueprintGenerateInput {
            blueprint_id: "two_story_hut".into(),
            display_name: "Hut Generated".into(),
            collision_asset_path: path.clone(),
            render_asset_path: Some(path),
            baseline_scale: 1.0,
            mesh,
        })
        .expect("generated");
        assert!(!output.blueprint.floors.is_empty());
        assert_eq!(
            output.blueprint.entrances.len(),
            1,
            "one physical door must yield one entrance; got {:?}",
            output
                .blueprint
                .entrances
                .iter()
                .map(|e| &e.key)
                .collect::<Vec<_>>()
        );
        assert_eq!(output.entrance_diagnostics.explicit_markers, 3);
        assert_eq!(output.entrance_diagnostics.synthesized_entrances, 0);
        assert!(output.entrance_diagnostics.deduplicated_candidates >= 2);
        assert_eq!(output.entrance_diagnostics.entrances_generated, 1);
    }

    fn flat_floor_mesh_with_portals(portals: Vec<PortalMarker3d>) -> BuildingMeshAnalysisInput {
        BuildingMeshAnalysisInput {
            triangles: vec![
                LocalTriangle3d {
                    a: Vec3::new(0.0, 0.0, 0.0),
                    b: Vec3::new(8.0, 0.0, 8.0),
                    c: Vec3::new(8.0, 0.0, 0.0),
                },
                LocalTriangle3d {
                    a: Vec3::new(0.0, 0.0, 0.0),
                    b: Vec3::new(0.0, 0.0, 8.0),
                    c: Vec3::new(8.0, 0.0, 8.0),
                },
            ],
            portal_markers: portals,
            source_path: "synthetic".into(),
            used_collision_node: true,
        }
    }

    fn marker(name: &str, x: f32, z: f32, path: &str) -> PortalMarker3d {
        PortalMarker3d {
            name: name.into(),
            position: Vec3::new(x, 0.0, z),
            scene_path: path.into(),
        }
    }

    #[test]
    fn one_explicit_portal_marker_yields_one_entrance() {
        let output = generate_navigation_blueprint(NavigationBlueprintGenerateInput {
            blueprint_id: BuildingNavigationBlueprintId::new("one_door"),
            display_name: "One".into(),
            collision_asset_path: PathBuf::from("one.glb"),
            render_asset_path: None,
            baseline_scale: 1.0,
            mesh: flat_floor_mesh_with_portals(vec![marker(
                "portal__front",
                4.0,
                0.1,
                "portal__front",
            )]),
        })
        .expect("gen");
        assert_eq!(output.blueprint.entrances.len(), 1);
        assert_eq!(output.entrance_diagnostics.synthesized_entrances, 0);
        assert_eq!(output.entrance_diagnostics.explicit_markers, 1);
        let entrance = &output.blueprint.entrances[0];
        assert!((entrance.local_position_xz[0] - 4.0).abs() < 1e-3);
        assert!((entrance.local_position_xz[1] - 0.1).abs() < 1e-3);
        assert!(entrance.radius_meters >= MIN_ENTRANCE_RADIUS);
        assert_eq!(entrance.floor_key, "floor_0");
    }

    #[test]
    fn marker_with_outside_and_inside_children_yields_one_entrance() {
        let output = generate_navigation_blueprint(NavigationBlueprintGenerateInput {
            blueprint_id: BuildingNavigationBlueprintId::new("grouped_door"),
            display_name: "Grouped".into(),
            collision_asset_path: PathBuf::from("grouped.glb"),
            render_asset_path: None,
            baseline_scale: 1.0,
            mesh: flat_floor_mesh_with_portals(vec![
                marker("portal__entrance", 4.0, 0.0, "portal__entrance"),
                marker(
                    "portal__entrance__outside",
                    4.0,
                    -0.2,
                    "portal__entrance/portal__entrance__outside",
                ),
                marker(
                    "portal__entrance__inside",
                    4.0,
                    1.0,
                    "portal__entrance/portal__entrance__inside",
                ),
            ]),
        })
        .expect("gen");
        assert_eq!(output.blueprint.entrances.len(), 1);
        assert_eq!(output.entrance_diagnostics.explicit_markers, 3);
        assert!(output.entrance_diagnostics.deduplicated_candidates >= 2);
        let entrance = &output.blueprint.entrances[0];
        // Outside marker wins for exterior position.
        assert!((entrance.local_position_xz[1] + 0.2).abs() < 1e-3);
        // Inside marker wins for interior spawn.
        assert!((entrance.interior_spawn_local[2] - 1.0).abs() < 1e-3);
        assert_eq!(output.entrance_diagnostics.synthesized_entrances, 0);
    }

    #[test]
    fn duplicate_logical_marker_nodes_still_one_entrance() {
        let output = generate_navigation_blueprint(NavigationBlueprintGenerateInput {
            blueprint_id: BuildingNavigationBlueprintId::new("dup"),
            display_name: "Dup".into(),
            collision_asset_path: PathBuf::from("dup.glb"),
            render_asset_path: None,
            baseline_scale: 1.0,
            mesh: flat_floor_mesh_with_portals(vec![
                marker("portal__door", 2.0, 0.0, "a/portal__door"),
                marker("portal__door", 2.0, 0.0, "b/portal__door"),
            ]),
        })
        .expect("gen");
        assert_eq!(output.blueprint.entrances.len(), 1);
    }

    #[test]
    fn two_distinct_markers_yield_two_entrances() {
        let output = generate_navigation_blueprint(NavigationBlueprintGenerateInput {
            blueprint_id: BuildingNavigationBlueprintId::new("two_doors"),
            display_name: "Two".into(),
            collision_asset_path: PathBuf::from("two.glb"),
            render_asset_path: None,
            baseline_scale: 1.0,
            mesh: flat_floor_mesh_with_portals(vec![
                marker("portal__north", 4.0, 0.0, "portal__north"),
                marker("portal__south", 4.0, 8.0, "portal__south"),
            ]),
        })
        .expect("gen");
        assert_eq!(output.blueprint.entrances.len(), 2);
        assert_eq!(output.entrance_diagnostics.entrances_generated, 2);
        assert_eq!(output.entrance_diagnostics.synthesized_entrances, 0);
    }

    #[test]
    fn no_markers_synthesize_exactly_one_fallback_entrance() {
        let output = generate_navigation_blueprint(NavigationBlueprintGenerateInput {
            blueprint_id: BuildingNavigationBlueprintId::new("fallback"),
            display_name: "Fallback".into(),
            collision_asset_path: PathBuf::from("fallback.glb"),
            render_asset_path: None,
            baseline_scale: 1.0,
            mesh: flat_floor_mesh_with_portals(Vec::new()),
        })
        .expect("gen");
        assert_eq!(output.blueprint.entrances.len(), 1);
        assert_eq!(output.blueprint.entrances[0].key, "exterior_entrance");
        assert_eq!(output.entrance_diagnostics.synthesized_entrances, 1);
        assert_eq!(output.entrance_diagnostics.explicit_markers, 0);
        assert!(
            output
                .warnings
                .iter()
                .any(|w| w.contains("synthesized entrance"))
        );
    }

    #[test]
    fn explicit_marker_suppresses_fallback_entrance() {
        let output = generate_navigation_blueprint(NavigationBlueprintGenerateInput {
            blueprint_id: BuildingNavigationBlueprintId::new("no_fallback"),
            display_name: "NoFallback".into(),
            collision_asset_path: PathBuf::from("nofallback.glb"),
            render_asset_path: None,
            baseline_scale: 1.0,
            mesh: flat_floor_mesh_with_portals(vec![marker(
                "portal__main",
                1.0,
                0.0,
                "portal__main",
            )]),
        })
        .expect("gen");
        assert_eq!(output.blueprint.entrances.len(), 1);
        assert_ne!(output.blueprint.entrances[0].key, "exterior_entrance");
        assert_eq!(output.entrance_diagnostics.synthesized_entrances, 0);
    }

    #[test]
    fn close_but_distinct_doors_are_not_merged() {
        let output = generate_navigation_blueprint(NavigationBlueprintGenerateInput {
            blueprint_id: BuildingNavigationBlueprintId::new("close_doors"),
            display_name: "Close".into(),
            collision_asset_path: PathBuf::from("close.glb"),
            render_asset_path: None,
            baseline_scale: 1.0,
            mesh: flat_floor_mesh_with_portals(vec![
                marker("portal__left", 2.0, 0.0, "portal__left"),
                marker("portal__right", 4.0, 0.0, "portal__right"),
            ]),
        })
        .expect("gen");
        assert_eq!(
            output.blueprint.entrances.len(),
            2,
            "2m-apart doors must remain distinct"
        );
    }

    #[test]
    fn logical_portal_group_key_strips_role_suffixes() {
        assert_eq!(
            logical_portal_group_key("portal__entrance__outside"),
            "portal__entrance"
        );
        assert_eq!(
            logical_portal_group_key("portal__entrance__inside"),
            "portal__entrance"
        );
        assert_eq!(
            logical_portal_group_key("portal__entrance"),
            "portal__entrance"
        );
    }
}
