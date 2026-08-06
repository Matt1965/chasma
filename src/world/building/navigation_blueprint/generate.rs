//! Automatic navigation blueprint generation from building meshes (NV1.2).

use std::path::{Path, PathBuf};

use bevy::prelude::*;

use super::cache::NAVIGATION_BLUEPRINT_GENERATOR_VERSION;
use super::definition::{
    BuildingNavigationBlueprint, BuildingNavigationBlueprintMetadata, MIN_CONNECTION_RADIUS,
    NavigationEntranceDefinition, NavigationFloorDefinition, NavigationPolygon2d,
    NavigationRegionConnectionDefinition, NavigationRegionConnectionKind,
    NavigationVerticalTransitionDefinition, NavigationVerticalTransitionKind,
};
use super::id::{BuildingNavigationBlueprintId, blueprint_id_for_building};
use super::mesh::{BuildingMeshAnalysisInput, LocalTriangle3d, PortalMarker3d};
use super::region_extract::point_in_polygon as region_point_in_polygon;
use super::region_extract::{
    ExtractedRegion, RegionGeneratorConfig, cluster_walkable_triangles_by_elevation,
    extract_regions_for_elevation, find_containing_regions, region_definitions_from_extracted,
};
use super::report::{
    EntranceGenerationDiagnostics, GeometryGenerationDiagnostics,
    NavigationBlueprintGenerationReport, NavigationBlueprintGenerationStatus,
};
use crate::world::BlueprintInspectionValidation;
use crate::world::authoring_transform::BuildingTransformSafetyClass;
use crate::world::building::catalog::BuildingDefinition;
use crate::world::occupancy::bake::source_file_hash_hex;
use crate::world::validate_blueprint_for_inspection;

const WALKABLE_NORMAL_MIN_Y: f32 = 0.72;
const FLOOR_CLUSTER_GAP_METERS: f32 = 2.5;
const MIN_ENTRANCE_RADIUS: f32 = 0.75;
const DEFAULT_ENTRANCE_RADIUS: f32 = 1.5;
const REGION_ENDPOINT_INSET_MIN: f32 = 0.15;
const DEFAULT_CONNECTION_RADIUS: f32 = 0.8;

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
    pub geometry_diagnostics: GeometryGenerationDiagnostics,
    pub validation: BlueprintInspectionValidation,
}

/// Generation result with blueprint draft and diagnostics report (IN-09).
#[derive(Debug, Clone)]
pub struct NavigationBlueprintGenerationResult {
    pub blueprint: BuildingNavigationBlueprint,
    pub report: NavigationBlueprintGenerationReport,
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
            "generator_render_mesh_fallback: occupancy_collision node missing — used visible mesh geometry for analysis".into(),
        );
    }

    let config = RegionGeneratorConfig {
        walkable_normal_min_y: WALKABLE_NORMAL_MIN_Y,
        floor_cluster_gap_meters: FLOOR_CLUSTER_GAP_METERS,
        ..RegionGeneratorConfig::default()
    };

    let (mut floors, mut geometry_diag) =
        build_floor_definitions_from_mesh(&triangles, &config, &mut warnings);
    if floors.is_empty() {
        return Err("no walkable horizontal surfaces detected".into());
    }
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
    resolve_entrance_region_targets(&mut entrances, &floors, &mut warnings);

    let mut vertical_transitions =
        vertical_transitions_from_portals(&portal_markers, &floors, &mut warnings);
    resolve_transition_region_targets(&mut vertical_transitions, &floors, &mut warnings);

    let region_connections = connections_from_portal_markers(
        &portal_markers,
        &floors,
        &mut warnings,
        &mut geometry_diag,
    );

    geometry_diag.candidate_connection_count = region_connections.len();
    geometry_diag.used_collision_mesh = input.mesh.used_collision_node;
    geometry_diag.used_render_fallback = !input.mesh.used_collision_node;

    for floor in &floors {
        if floor.regions.len() == 1
            && count_doorway_marker_groups_on_floor(&portal_markers, floor, &floors) >= 2
        {
            warnings.push(format!(
                "generator_region_split_ambiguous: floor `{}` has one generated region but multiple doorway markers — manual region splitting recommended",
                floor.key
            ));
        }
    }

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
        region_connections,
        enabled: true,
    };

    let validation = validate_blueprint_for_inspection(&blueprint);
    for diagnostic in &validation.diagnostics {
        if diagnostic.level == crate::world::BlueprintDiagnosticLevel::Error {
            warnings.push(format!("validation: {}", diagnostic.message));
        }
    }

    if blueprint.entrances.is_empty() {
        warnings.push("no entrances generated — manual authoring required".into());
    }

    Ok(NavigationBlueprintGenerateOutput {
        blueprint,
        warnings,
        entrance_diagnostics: entrance_diag,
        geometry_diagnostics: geometry_diag,
        validation,
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
        geometry_diagnostics: GeometryGenerationDiagnostics::default(),
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

fn build_floor_definitions_from_mesh(
    triangles: &[LocalTriangle3d],
    config: &RegionGeneratorConfig,
    warnings: &mut Vec<String>,
) -> (
    Vec<NavigationFloorDefinition>,
    GeometryGenerationDiagnostics,
) {
    let (elevations, cluster_stats) = cluster_walkable_triangles_by_elevation(triangles, config);
    let mut geometry = GeometryGenerationDiagnostics {
        source_triangle_count: cluster_stats.source_triangle_count,
        walkable_triangle_count: cluster_stats.walkable_triangle_count,
        steep_triangle_discarded: cluster_stats.steep_triangle_discarded,
        floor_cluster_count: cluster_stats.floor_cluster_count,
        ..Default::default()
    };
    let mut floors = Vec::new();
    let mut region_offset = 0usize;
    for (index, elevation) in elevations.iter().enumerate() {
        let (extracted, floor_stats) =
            extract_regions_for_elevation(triangles, *elevation, config, region_offset);
        geometry.connected_component_count += floor_stats.connected_component_count;
        geometry.regions_discarded += floor_stats.regions_discarded;
        geometry.convex_hull_fallback_count += floor_stats.convex_hull_fallback_count;
        geometry.multiple_boundary_loops += floor_stats.multiple_loop_count;
        warnings.extend(floor_stats.warnings);
        region_offset += extracted.regions.len();
        if extracted.regions.is_empty() {
            warnings.push(format!(
                "floor at y={elevation:.2} produced no valid regions — skipped"
            ));
            continue;
        }
        geometry.candidate_region_count += extracted.regions.len();
        let regions = region_definitions_from_extracted(&extracted.regions);
        floors.push(NavigationFloorDefinition {
            floor_id: index as i32,
            key: format!("floor_{index}"),
            display_label: format!("Floor {:.1}m", elevation),
            elevation_meters: *elevation,
            visibility_group_id: (index + 1) as u32,
            room_tag: None,
            walkable_outline_legacy: None,
            regions,
        });
    }
    (floors, geometry)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GeneratedRegionTargetKind {
    Entrance,
    TransitionFrom,
    TransitionTo,
    Connection,
}

fn floor_regions_for_targeting(floor: &NavigationFloorDefinition) -> Vec<ExtractedRegion> {
    floor
        .regions
        .iter()
        .map(|region| ExtractedRegion {
            key: region.key.clone(),
            display_label: region.display_label.clone(),
            outline: region.walkable_outline.clone(),
            used_convex_hull_fallback: false,
            centroid_xz: region_extract_centroid(&region.walkable_outline),
        })
        .collect()
}

fn resolve_region_target(
    floor: &NavigationFloorDefinition,
    point: Vec2,
    feature_key: &str,
    kind: GeneratedRegionTargetKind,
    warnings: &mut Vec<String>,
) -> Option<String> {
    let extracted = floor_regions_for_targeting(floor);
    let containing = find_containing_regions(point, &extracted);
    match containing.len() {
        0 => {
            let code = match kind {
                GeneratedRegionTargetKind::Entrance => "generator_entrance_region_unresolved",
                GeneratedRegionTargetKind::TransitionFrom
                | GeneratedRegionTargetKind::TransitionTo => {
                    "generator_transition_region_unresolved"
                }
                GeneratedRegionTargetKind::Connection => "generator_connection_ambiguous",
            };
            warnings.push(format!(
                "{code}: feature `{feature_key}` on floor `{}` at [{:.2},{:.2}] matches 0 regions",
                floor.key, point.x, point.y
            ));
            None
        }
        1 => Some(containing[0].key.clone()),
        _ => {
            let keys = containing
                .iter()
                .map(|region| region.key.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let code = match kind {
                GeneratedRegionTargetKind::Entrance => "generator_entrance_region_ambiguous",
                GeneratedRegionTargetKind::TransitionFrom
                | GeneratedRegionTargetKind::TransitionTo => {
                    "generator_transition_region_ambiguous"
                }
                GeneratedRegionTargetKind::Connection => "generator_connection_ambiguous",
            };
            warnings.push(format!(
                "{code}: feature `{feature_key}` on floor `{}` at [{:.2},{:.2}] matches [{keys}]",
                floor.key, point.x, point.y
            ));
            None
        }
    }
}

fn resolve_entrance_region_targets(
    entrances: &mut [NavigationEntranceDefinition],
    floors: &[NavigationFloorDefinition],
    warnings: &mut Vec<String>,
) {
    for entrance in entrances.iter_mut() {
        let Some(floor) = floors.iter().find(|floor| floor.key == entrance.floor_key) else {
            entrance.region_key = None;
            continue;
        };
        let spawn = Vec2::new(
            entrance.interior_spawn_local[0],
            entrance.interior_spawn_local[2],
        );
        entrance.region_key = resolve_region_target(
            floor,
            spawn,
            &entrance.key,
            GeneratedRegionTargetKind::Entrance,
            warnings,
        );
    }
}

fn resolve_transition_region_targets(
    transitions: &mut [NavigationVerticalTransitionDefinition],
    floors: &[NavigationFloorDefinition],
    warnings: &mut Vec<String>,
) {
    for transition in transitions.iter_mut() {
        if let Some(from_floor) = floors
            .iter()
            .find(|floor| floor.key == transition.from_floor_key)
        {
            let point = Vec2::new(
                transition.from_local_position_xz[0],
                transition.from_local_position_xz[1],
            );
            transition.from_region_key = resolve_region_target(
                from_floor,
                point,
                &transition.key,
                GeneratedRegionTargetKind::TransitionFrom,
                warnings,
            );
        } else {
            transition.from_region_key = None;
        }
        if let Some(to_floor) = floors
            .iter()
            .find(|floor| floor.key == transition.to_floor_key)
        {
            let point = Vec2::new(
                transition.to_local_position[0],
                transition.to_local_position[2],
            );
            transition.to_region_key = resolve_region_target(
                to_floor,
                point,
                &transition.key,
                GeneratedRegionTargetKind::TransitionTo,
                warnings,
            );
        } else {
            transition.to_region_key = None;
        }
    }
}

fn region_extract_centroid(outline: &NavigationPolygon2d) -> Vec2 {
    let verts: Vec<Vec2> = outline
        .vertices_xz
        .iter()
        .map(|&[x, z]| Vec2::new(x, z))
        .collect();
    if verts.is_empty() {
        Vec2::ZERO
    } else {
        verts.iter().fold(Vec2::ZERO, |acc, v| acc + *v) / verts.len() as f32
    }
}

fn connections_from_portal_markers(
    markers: &[PortalMarker3d],
    floors: &[NavigationFloorDefinition],
    warnings: &mut Vec<String>,
    geometry: &mut GeometryGenerationDiagnostics,
) -> Vec<NavigationRegionConnectionDefinition> {
    let connection_markers: Vec<&PortalMarker3d> = markers
        .iter()
        .filter(|marker| portal_kind(&marker.name).is_interior_connection())
        .collect();
    if connection_markers.is_empty() {
        return Vec::new();
    }

    let groups = group_portal_connection_markers(&connection_markers);
    let mut connections = Vec::new();
    for (group_index, group) in groups.into_iter().enumerate() {
        let Some(floor) = group
            .markers
            .first()
            .and_then(|marker| nearest_floor(floors, marker.position.y))
        else {
            continue;
        };
        let mut region_positions: Vec<(String, Vec2)> = Vec::new();
        for marker in &group.markers {
            let point = Vec2::new(marker.position.x, marker.position.z);
            if let Some(region_key) = resolve_region_target(
                floor,
                point,
                &group.logical_key,
                GeneratedRegionTargetKind::Connection,
                warnings,
            ) {
                if let Some(inset) = inset_point_in_region(floor, &region_key, point) {
                    region_positions.push((region_key, inset));
                } else {
                    warnings.push(format!(
                        "generator_connection_ambiguous: could not inset endpoint for portal `{}`",
                        marker.name
                    ));
                    geometry.ambiguous_opening_count += 1;
                }
            } else {
                geometry.ambiguous_opening_count += 1;
            }
        }
        region_positions.sort_by(|a, b| a.0.cmp(&b.0));
        region_positions.dedup_by(|a, b| a.0 == b.0);
        if region_positions.len() < 2 {
            warnings.push(format!(
                "generator_connection_ambiguous: portal group `{}` lacks two distinct region endpoints",
                group.logical_key
            ));
            geometry.ambiguous_opening_count += 1;
            continue;
        }
        let (from_region, from_point) = region_positions[0].clone();
        let (to_region, to_point) = region_positions[1].clone();
        let span = from_point.distance(to_point);
        let radius = (span * 0.35)
            .clamp(MIN_CONNECTION_RADIUS, DEFAULT_CONNECTION_RADIUS)
            .max(MIN_CONNECTION_RADIUS);
        let kind = if group
            .markers
            .iter()
            .any(|marker| marker.name.to_ascii_lowercase().contains("door"))
        {
            NavigationRegionConnectionKind::Doorway
        } else {
            NavigationRegionConnectionKind::OpenArch
        };
        connections.push(NavigationRegionConnectionDefinition {
            key: portal_key_suffix(&group.logical_key)
                .unwrap_or_else(|| format!("connection_{group_index}")),
            kind,
            floor_key: floor.key.clone(),
            from_region_key: from_region,
            to_region_key: to_region,
            from_local_position_xz: [from_point.x, from_point.y],
            to_local_position_xz: [to_point.x, to_point.y],
            radius_meters: radius,
            bidirectional: true,
            enabled: true,
            door_key: None,
        });
    }
    connections.sort_by(|a, b| a.key.cmp(&b.key));
    connections
}

#[derive(Debug, Clone)]
struct PortalConnectionGroup<'a> {
    logical_key: String,
    markers: Vec<&'a PortalMarker3d>,
}

fn group_portal_connection_markers<'a>(
    markers: &[&'a PortalMarker3d],
) -> Vec<PortalConnectionGroup<'a>> {
    let mut groups: Vec<PortalConnectionGroup<'a>> = Vec::new();
    for marker in markers {
        let key = logical_portal_group_key(&marker.name);
        if let Some(group) = groups.iter_mut().find(|group| group.logical_key == key) {
            group.markers.push(marker);
        } else {
            groups.push(PortalConnectionGroup {
                logical_key: key,
                markers: vec![marker],
            });
        }
    }
    groups
}

fn inset_point_in_region(
    floor: &NavigationFloorDefinition,
    region_key: &str,
    point: Vec2,
) -> Option<Vec2> {
    let region = floor.region_by_key(region_key)?;
    if !region_point_in_polygon(point, &region.walkable_outline) {
        return None;
    }
    let centroid = region_extract_centroid(&region.walkable_outline);
    let direction = (centroid - point).normalize_or_zero();
    let inset = point + direction * REGION_ENDPOINT_INSET_MIN;
    if region_point_in_polygon(inset, &region.walkable_outline) {
        Some(inset)
    } else {
        Some(point)
    }
}

fn count_doorway_marker_groups_on_floor(
    markers: &[PortalMarker3d],
    floor: &NavigationFloorDefinition,
    floors: &[NavigationFloorDefinition],
) -> usize {
    let mut keys = std::collections::BTreeSet::new();
    for marker in markers {
        if !portal_kind(&marker.name).is_interior_connection() {
            continue;
        }
        let Some(marker_floor) = nearest_floor(floors, marker.position.y) else {
            continue;
        };
        if marker_floor.key != floor.key {
            continue;
        }
        keys.insert(logical_portal_group_key(&marker.name));
    }
    keys.len()
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
            region_key: None,
            local_position_xz: [exterior.x, exterior.y],
            radius_meters: radius.max(MIN_ENTRANCE_RADIUS),
            interior_spawn_local: [interior.x, interior.y, interior.z],
            bidirectional: true,
            door_key: None,
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
            from_region_key: None,
            to_region_key: None,
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
        .sole_region_outline()?
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
        region_key: None,
        local_position_xz: [mid.x, mid.y],
        radius_meters: radius,
        interior_spawn_local: [
            mid.x + (centroid.x - mid.x) * 0.4,
            ground.elevation_meters,
            mid.y + (centroid.y - mid.y) * 0.4,
        ],
        bidirectional: true,
        door_key: None,
    })
}

fn floor_centroid(floor: &NavigationFloorDefinition) -> Vec2 {
    let Some(outline) = floor.sole_region_outline() else {
        return Vec2::ZERO;
    };
    let verts = &outline.vertices_xz;
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
    let Some(outline) = floor.sole_region_outline() else {
        return false;
    };
    let verts: Vec<Vec2> = outline
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
    InteriorConnection,
    Stair,
    Ramp,
    Ladder,
    Other,
}

impl PortalKind {
    fn is_entrance(self) -> bool {
        matches!(self, Self::Entrance | Self::Other)
    }

    fn is_interior_connection(self) -> bool {
        matches!(self, Self::InteriorConnection)
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
    } else if lower.contains("doorway") || lower.contains("interior_door") {
        PortalKind::InteriorConnection
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
        let outline = &output.blueprint.floors[0]
            .sole_region_outline()
            .expect("region")
            .vertices_xz;
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
        let entrance = &output.blueprint.entrances[0];
        assert!(
            entrance.region_key.is_some(),
            "hut entrance must target exactly one generated region via interior spawn; got {:?}",
            entrance.region_key
        );
        assert!(
            !output
                .warnings
                .iter()
                .any(|w| w.contains("generator_entrance_region_unresolved")
                    || w.contains("generator_entrance_region_ambiguous")),
            "entrance targeting should resolve when interior spawn lies in one region"
        );
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

    fn synthetic_mesh(
        triangles: Vec<LocalTriangle3d>,
        portals: Vec<PortalMarker3d>,
        collision: bool,
    ) -> BuildingMeshAnalysisInput {
        BuildingMeshAnalysisInput {
            triangles,
            portal_markers: portals,
            source_path: "synthetic".into(),
            used_collision_node: collision,
        }
    }

    fn synthetic_generate(
        triangles: Vec<LocalTriangle3d>,
        portals: Vec<PortalMarker3d>,
    ) -> NavigationBlueprintGenerateOutput {
        generate_navigation_blueprint(NavigationBlueprintGenerateInput {
            blueprint_id: BuildingNavigationBlueprintId::new("synthetic"),
            display_name: "Synthetic".into(),
            collision_asset_path: PathBuf::from("synthetic.glb"),
            render_asset_path: None,
            baseline_scale: 1.0,
            mesh: synthetic_mesh(triangles, portals, true),
        })
        .expect("generate")
    }

    #[test]
    fn l_shape_floor_stays_concave_without_hull_fallback() {
        let output = synthetic_generate(l_shape_triangles(), Vec::new());
        assert_eq!(output.blueprint.floors.len(), 1);
        assert_eq!(output.blueprint.floors[0].regions.len(), 1);
        assert_eq!(output.geometry_diagnostics.convex_hull_fallback_count, 0);
        let outline = &output.blueprint.floors[0].regions[0].walkable_outline;
        assert!(!super::super::region_extract::point_in_polygon(
            Vec2::new(7.0, 7.0),
            outline
        ));
    }

    #[test]
    fn disconnected_islands_become_two_regions_without_connection() {
        let mut tris = rect_triangles(0.0, 0.0, 4.0, 4.0, 0.0);
        tris.extend(rect_triangles(10.0, 0.0, 14.0, 4.0, 0.0));
        let output = synthetic_generate(tris, Vec::new());
        assert_eq!(output.blueprint.floors.len(), 1);
        assert_eq!(output.blueprint.floors[0].regions.len(), 2);
        assert!(output.blueprint.region_connections.is_empty());
        assert_eq!(output.geometry_diagnostics.connected_component_count, 2);
    }

    #[test]
    fn two_rooms_with_doorway_markers_yield_connection() {
        let mut tris = rect_triangles(0.0, 0.0, 6.0, 4.0, 0.0);
        tris.extend(rect_triangles(6.4, 0.0, 12.4, 4.0, 0.0));
        let portals = vec![
            marker("portal__room_doorway__inside", 5.8, 2.0, "a"),
            marker("portal__room_doorway__outside", 6.6, 2.0, "b"),
        ];
        let output = synthetic_generate(tris, portals);
        assert_eq!(output.blueprint.floors[0].regions.len(), 2);
        assert_eq!(output.blueprint.region_connections.len(), 1);
        let connection = &output.blueprint.region_connections[0];
        assert_ne!(connection.from_region_key, connection.to_region_key);
    }

    #[test]
    fn multi_floor_surfaces_stay_separate() {
        let mut tris = rect_triangles(0.0, 0.0, 8.0, 8.0, 0.0);
        tris.extend(rect_triangles(0.0, 0.0, 8.0, 8.0, 3.0));
        let output = synthetic_generate(tris, Vec::new());
        assert_eq!(output.blueprint.floors.len(), 2);
        assert_eq!(output.geometry_diagnostics.floor_cluster_count, 2);
    }

    #[test]
    fn render_mesh_fallback_emits_warning() {
        let output = generate_navigation_blueprint(NavigationBlueprintGenerateInput {
            blueprint_id: BuildingNavigationBlueprintId::new("fallback_mesh"),
            display_name: "Fallback".into(),
            collision_asset_path: PathBuf::from("synthetic.glb"),
            render_asset_path: None,
            baseline_scale: 1.0,
            mesh: synthetic_mesh(l_shape_triangles(), Vec::new(), false),
        })
        .expect("generate");
        assert!(output.geometry_diagnostics.used_render_fallback);
        assert!(
            output
                .warnings
                .iter()
                .any(|w| w.contains("generator_render_mesh_fallback"))
        );
    }

    #[test]
    fn entrance_targets_region_containing_interior_spawn_not_exterior() {
        let mut tris = rect_triangles(0.0, 0.0, 6.0, 4.0, 0.0);
        tris.extend(rect_triangles(6.4, 0.0, 12.4, 4.0, 0.0));
        let portals = vec![
            marker("portal__entrance__outside", 3.0, -0.2, "outside"),
            marker("portal__entrance__inside", 3.0, 1.0, "inside"),
        ];
        let output = synthetic_generate(tris, portals);
        let entrance = &output.blueprint.entrances[0];
        assert_eq!(entrance.region_key.as_deref(), Some("region_1"));
        assert!(
            !output
                .warnings
                .iter()
                .any(|w| w.contains("generator_entrance_region_ambiguous"))
        );
    }

    #[test]
    fn entrance_spawn_in_second_region_selects_region_b() {
        let mut tris = rect_triangles(0.0, 0.0, 6.0, 4.0, 0.0);
        tris.extend(rect_triangles(6.4, 0.0, 12.4, 4.0, 0.0));
        let portals = vec![
            marker("portal__entrance__outside", 3.0, -0.2, "outside"),
            marker("portal__entrance__inside", 9.0, 2.0, "inside"),
        ];
        let output = synthetic_generate(tris, portals);
        assert_eq!(
            output.blueprint.entrances[0].region_key.as_deref(),
            Some("region_2")
        );
    }

    #[test]
    fn entrance_spawn_outside_all_regions_keeps_unresolved_invalid_draft() {
        let mut tris = rect_triangles(0.0, 0.0, 6.0, 4.0, 0.0);
        tris.extend(rect_triangles(6.4, 0.0, 12.4, 4.0, 0.0));
        let portals = vec![
            marker("portal__entrance__outside", 3.0, -0.2, "outside"),
            marker("portal__entrance__inside", 6.2, 2.0, "inside"),
        ];
        let output = synthetic_generate(tris, portals);
        assert!(output.blueprint.entrances[0].region_key.is_none());
        assert!(!output.validation.valid());
        assert!(
            output
                .warnings
                .iter()
                .any(|w| w.contains("generator_entrance_region_unresolved"))
        );
    }

    #[test]
    fn generation_returns_blueprint_when_validation_fails() {
        let mut tris = rect_triangles(0.0, 0.0, 6.0, 4.0, 0.0);
        tris.extend(rect_triangles(6.4, 0.0, 12.4, 4.0, 0.0));
        let portals = vec![
            marker("portal__entrance__outside", 3.0, -0.2, "outside"),
            marker("portal__entrance__inside", 6.2, 2.0, "inside"),
        ];
        let output = synthetic_generate(tris, portals);
        assert!(!output.blueprint.floors.is_empty());
        assert!(!output.validation.valid());
    }

    #[test]
    fn generation_is_deterministic_for_l_shape() {
        let a = synthetic_generate(l_shape_triangles(), Vec::new());
        let b = synthetic_generate(l_shape_triangles(), Vec::new());
        assert_eq!(
            a.blueprint.floors[0].regions[0]
                .walkable_outline
                .vertices_xz,
            b.blueprint.floors[0].regions[0]
                .walkable_outline
                .vertices_xz
        );
    }
}
