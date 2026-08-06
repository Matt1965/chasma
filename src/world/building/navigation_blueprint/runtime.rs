//! Runtime navigation data derived from building navigation blueprints (NV1.3 + NV2).

use std::collections::HashMap;

use bevy::prelude::*;

use super::adapt::{floor_key_from_region_space_key, region_space_key};
use super::definition::BuildingNavigationBlueprint;
use super::id::BuildingNavigationBlueprintId;
use super::{BlueprintPortalTemplate, BlueprintSpaceTemplate, blueprint_portal_templates};
use crate::world::building::catalog::BuildingDefinition;
use crate::world::building::record::BuildingRecord;
use crate::world::space::{PortalRecord, PortalType, SpaceRecord, SpaceRegistry};
use crate::world::{
    BuildingId, ChunkLayout, SpaceId, WorldPosition, building_model_world_transform,
};

/// One navigable region registered from a blueprint.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeNavigationRegion {
    pub space_id: SpaceId,
    pub floor_id: i32,
    pub floor_key: String,
    pub region_key: String,
    pub display_label: String,
    pub elevation_meters: f32,
    pub world_outline_xz: Vec<Vec2>,
    pub world_aabb_min_xz: Vec2,
    pub world_aabb_max_xz: Vec2,
}

/// Grouping of region spaces on one authored floor.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeNavigationFloor {
    pub floor_id: i32,
    pub floor_key: String,
    pub elevation_meters: f32,
    pub visibility_group_id: u32,
    pub region_space_ids: Vec<SpaceId>,
}

/// Authoritative runtime navigation for one building instance.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildingNavigationRuntime {
    pub building_id: BuildingId,
    pub blueprint_id: BuildingNavigationBlueprintId,
    pub model_transform: Transform,
    pub space_keys: std::collections::BTreeMap<String, SpaceId>,
    pub portal_keys: std::collections::BTreeMap<String, crate::world::PortalId>,
    pub floors: Vec<RuntimeNavigationFloor>,
    pub regions: Vec<RuntimeNavigationRegion>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BuildingNavigationRuntimeStore {
    by_building: HashMap<BuildingId, BuildingNavigationRuntime>,
    space_to_building: HashMap<u32, BuildingId>,
}

impl BuildingNavigationRuntime {
    pub fn sole_region_space_for_floor(&self, floor_key: &str) -> Option<SpaceId> {
        let floor = self
            .floors
            .iter()
            .find(|floor| floor.floor_key == floor_key)?;
        if floor.region_space_ids.len() == 1 {
            Some(floor.region_space_ids[0])
        } else {
            None
        }
    }
}

impl BuildingNavigationRuntimeStore {
    pub fn insert(&mut self, runtime: BuildingNavigationRuntime) {
        for region in &runtime.regions {
            self.space_to_building
                .insert(region.space_id.raw(), runtime.building_id);
        }
        self.by_building.insert(runtime.building_id, runtime);
    }

    pub fn remove_building(&mut self, building_id: BuildingId) {
        if let Some(runtime) = self.by_building.remove(&building_id) {
            for region in &runtime.regions {
                self.space_to_building.remove(&region.space_id.raw());
            }
        }
    }

    pub fn get(&self, building_id: BuildingId) -> Option<&BuildingNavigationRuntime> {
        self.by_building.get(&building_id)
    }

    pub fn get_for_space(&self, space_id: SpaceId) -> Option<&BuildingNavigationRuntime> {
        let building_id = self.space_to_building.get(&space_id.raw())?;
        self.by_building.get(building_id)
    }

    pub fn region_for_space(&self, space_id: SpaceId) -> Option<&RuntimeNavigationRegion> {
        let runtime = self.get_for_space(space_id)?;
        runtime
            .regions
            .iter()
            .find(|region| region.space_id == space_id)
    }

    pub fn floor_for_space(&self, space_id: SpaceId) -> Option<&RuntimeNavigationFloor> {
        let runtime = self.get_for_space(space_id)?;
        let region = runtime
            .regions
            .iter()
            .find(|region| region.space_id == space_id)?;
        runtime
            .floors
            .iter()
            .find(|floor| floor.floor_key == region.floor_key)
    }

    pub fn regions_on_floor<'a>(
        &'a self,
        building_id: BuildingId,
        floor_key: &str,
    ) -> impl Iterator<Item = &'a RuntimeNavigationRegion> {
        self.get(building_id).into_iter().flat_map(move |runtime| {
            runtime
                .regions
                .iter()
                .filter(move |region| region.floor_key == floor_key)
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = &BuildingNavigationRuntime> {
        self.by_building.values()
    }
}

fn compute_aabb(points: &[Vec2]) -> (Vec2, Vec2) {
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for point in points {
        min = min.min(*point);
        max = max.max(*point);
    }
    (min, max)
}

pub fn build_navigation_runtime(
    building_id: BuildingId,
    blueprint: &BuildingNavigationBlueprint,
    model_transform: Transform,
    space_keys: &std::collections::BTreeMap<String, SpaceId>,
    portal_keys: &std::collections::BTreeMap<String, crate::world::PortalId>,
) -> BuildingNavigationRuntime {
    let mut regions = Vec::new();
    let mut floors = Vec::new();

    for floor in &blueprint.floors {
        let mut region_space_ids = Vec::new();
        for region in &floor.regions {
            let key = region_space_key(&floor.key, &region.key);
            let Some(space_id) = space_keys.get(&key).copied() else {
                continue;
            };
            let world_outline_xz = region
                .walkable_outline
                .vertices_xz
                .iter()
                .map(|&[x, z]| {
                    let local = Vec3::new(x, floor.elevation_meters, z);
                    let world = model_transform.transform_point(local);
                    Vec2::new(world.x, world.z)
                })
                .collect::<Vec<_>>();
            let (world_aabb_min_xz, world_aabb_max_xz) = compute_aabb(&world_outline_xz);
            regions.push(RuntimeNavigationRegion {
                space_id,
                floor_id: floor.floor_id,
                floor_key: floor.key.clone(),
                region_key: region.key.clone(),
                display_label: region.display_label.clone(),
                elevation_meters: floor.elevation_meters,
                world_outline_xz,
                world_aabb_min_xz,
                world_aabb_max_xz,
            });
            region_space_ids.push(space_id);
        }
        floors.push(RuntimeNavigationFloor {
            floor_id: floor.floor_id,
            floor_key: floor.key.clone(),
            elevation_meters: floor.elevation_meters,
            visibility_group_id: floor.visibility_group_id,
            region_space_ids,
        });
    }

    BuildingNavigationRuntime {
        building_id,
        blueprint_id: blueprint.id.clone(),
        model_transform,
        space_keys: space_keys.clone(),
        portal_keys: portal_keys.clone(),
        floors,
        regions,
    }
}

/// Register blueprint-derived spaces and portals using asset-transform-standardized poses.
pub fn register_building_navigation_profile(
    registry: &mut SpaceRegistry,
    building: &BuildingRecord,
    definition: &BuildingDefinition,
    layout: ChunkLayout,
    spaces: &[BlueprintSpaceTemplate],
    portals: &[BlueprintPortalTemplate],
) -> (
    std::collections::BTreeMap<String, SpaceId>,
    std::collections::BTreeMap<String, crate::world::PortalId>,
) {
    let model = building_model_world_transform(definition, &building.placement, layout);
    let floor_y_by_key: std::collections::BTreeMap<String, f32> = spaces
        .iter()
        .map(|space| {
            (
                floor_key_from_region_space_key(&space.key).to_string(),
                space.local_floor_y,
            )
        })
        .collect();

    let mut key_to_space: std::collections::BTreeMap<String, SpaceId> =
        std::collections::BTreeMap::from([("surface".to_string(), SpaceId::SURFACE)]);

    let mut space_records = Vec::new();
    for template in spaces {
        let id = registry.allocate_space_id();
        key_to_space.insert(template.key.clone(), id);
        let floor_world = model.transform_point(Vec3::new(0.0, template.local_floor_y, 0.0));
        space_records.push(SpaceRecord {
            id,
            owning_building_id: Some(building.id),
            display_floor_label: template.display_floor_label.clone(),
            visibility_group_id: template.visibility_group_id,
            reference_elevation: template.reference_elevation,
            floor_y_global: floor_world.y,
            room_tag: template.room_tag.clone(),
            enabled: true,
            walkable: true,
        });
    }

    let mut regions_per_floor: std::collections::BTreeMap<String, Vec<SpaceId>> =
        std::collections::BTreeMap::new();
    for template in spaces {
        let floor_key = floor_key_from_region_space_key(&template.key).to_string();
        if let Some(id) = key_to_space.get(&template.key) {
            regions_per_floor.entry(floor_key).or_default().push(*id);
        }
    }
    for (floor_key, ids) in regions_per_floor {
        if ids.len() == 1 {
            key_to_space.insert(floor_key, ids[0]);
        }
    }

    let mut portal_records = Vec::new();
    let mut portal_key_to_id: std::collections::BTreeMap<String, crate::world::PortalId> =
        std::collections::BTreeMap::new();
    for template in portals {
        let from_space = *key_to_space
            .get(&template.from_space_key)
            .unwrap_or_else(|| {
                panic!(
                    "missing from space `{}` for portal `{}`",
                    template.from_space_key, template.key
                )
            });
        let to_space = *key_to_space.get(&template.to_space_key).unwrap_or_else(|| {
            panic!(
                "missing to space `{}` for portal `{}`",
                template.to_space_key, template.key
            )
        });
        let from_floor_y = floor_y_by_key
            .get(floor_key_from_region_space_key(&template.from_space_key))
            .copied()
            .unwrap_or(0.0);
        let from_local = Vec3::new(
            template.from_local_xz.x,
            from_floor_y,
            template.from_local_xz.y,
        );
        let from_global = model.transform_point(from_local);
        let to_global = model.transform_point(template.to_local_position);
        let portal_id = registry.allocate_portal_id();
        portal_key_to_id.insert(template.key.clone(), portal_id);
        portal_records.push(PortalRecord {
            id: portal_id,
            portal_type: template.portal_type,
            from_space,
            to_space,
            from_center_global_xz: Vec2::new(from_global.x, from_global.z),
            from_radius_meters: template.from_radius_meters,
            to_position: WorldPosition::from_global(to_global, layout),
            traversal_cost: 1.0,
            bidirectional: template.bidirectional,
            enabled: template.enabled,
            owning_building_id: Some(building.id),
        });
    }

    registry.register_building_spaces(building.id, space_records, portal_records);
    (key_to_space, portal_key_to_id)
}

/// Point-in-polygon test for world XZ outlines (ray casting).
pub fn point_in_polygon_xz(polygon: &[Vec2], point: Vec2) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = polygon.len() - 1;
    for (i, vertex) in polygon.iter().enumerate() {
        let vi = *vertex;
        let vj = polygon[j];
        if ((vi.y > point.y) != (vj.y > point.y))
            && (point.x < (vj.x - vi.x) * (point.y - vi.y) / (vj.y - vi.y + f32::EPSILON) + vi.x)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Minimum unsigned distance from `point` to any polygon edge (meters).
pub fn min_edge_clearance_meters(point: Vec2, polygon: &[Vec2]) -> f32 {
    if polygon.len() < 3 {
        return f32::INFINITY;
    }
    let inside = point_in_polygon_xz(polygon, point);
    let mut min_dist = f32::INFINITY;
    let count = polygon.len();
    for index in 0..count {
        let a = polygon[index];
        let b = polygon[(index + 1) % count];
        let edge = b - a;
        let len_sq = edge.length_squared();
        if len_sq <= f32::EPSILON {
            continue;
        }
        let t = ((point - a).dot(edge) / len_sq).clamp(0.0, 1.0);
        let closest = a + edge * t;
        min_dist = min_dist.min(point.distance(closest));
    }
    if inside { min_dist } else { min_dist }
}

/// Whether `position` is inside the region and has at least `agent_radius_meters` edge clearance.
pub fn interior_agent_fits_region(
    store: &BuildingNavigationRuntimeStore,
    space_registry: &SpaceRegistry,
    layout: ChunkLayout,
    position: WorldPosition,
    space_id: SpaceId,
    agent_radius_meters: f32,
) -> bool {
    if !interior_position_walkable(store, space_registry, layout, position, space_id) {
        return false;
    }
    let Some(region) = store.region_for_space(space_id) else {
        return false;
    };
    let xz = position.to_global(layout).xz();
    min_edge_clearance_meters(xz, &region.world_outline_xz) >= agent_radius_meters
}

fn point_in_region_aabb(region: &RuntimeNavigationRegion, point: Vec2) -> bool {
    point.x >= region.world_aabb_min_xz.x
        && point.x <= region.world_aabb_max_xz.x
        && point.y >= region.world_aabb_min_xz.y
        && point.y <= region.world_aabb_max_xz.y
}

/// Resolve which navigable region space contains a world position.
pub fn resolve_navigation_space_at_position(
    store: &BuildingNavigationRuntimeStore,
    space_registry: &SpaceRegistry,
    layout: ChunkLayout,
    position: WorldPosition,
) -> SpaceId {
    let global = position.to_global(layout);
    let point = Vec2::new(global.x, global.z);
    let mut best: Option<(SpaceId, f32)> = None;
    for runtime in store.iter() {
        for region in &runtime.regions {
            if !point_in_region_aabb(region, point) {
                continue;
            }
            if !point_in_polygon_xz(&region.world_outline_xz, point) {
                continue;
            }
            let floor_y = space_registry
                .get_space(region.space_id)
                .map(|space| space.floor_y_global)
                .unwrap_or(region.elevation_meters);
            let y_delta = (global.y - floor_y).abs();
            if best.is_none_or(|(_, best_delta)| y_delta < best_delta) {
                best = Some((region.space_id, y_delta));
            }
        }
    }
    best.map(|(space, _)| space).unwrap_or(SpaceId::SURFACE)
}

/// Maximum vertical distance from a floor's authored elevation to treat a unit as on that floor.
const FLOOR_ELEVATION_TOLERANCE_METERS: f32 = 1.5;

/// Resolve the start space for pathfinding, reconciling tracked state with position (NV2).
pub fn resolve_navigation_start_space(
    store: &BuildingNavigationRuntimeStore,
    space_registry: &SpaceRegistry,
    layout: ChunkLayout,
    position: WorldPosition,
    tracked_space: SpaceId,
) -> SpaceId {
    if tracked_space.is_surface() {
        return SpaceId::SURFACE;
    }
    let global = position.to_global(layout);
    if let Some(space) = space_registry.get_space(tracked_space) {
        if (global.y - space.floor_y_global).abs() <= FLOOR_ELEVATION_TOLERANCE_METERS {
            // Keep interior authority while on the authored floor even when the unit is
            // outside the region polygon — passability and segment checks block at the boundary
            // instead of falling back to surface footprint blocking (IN-11e).
            return tracked_space;
        }
    } else {
        return tracked_space;
    }
    resolve_navigation_space_at_position(store, space_registry, layout, position)
}

/// Whether an open segment crosses any polygon edge (excluding shared endpoints).
pub fn segment_crosses_polygon_boundary(from: Vec2, to: Vec2, polygon: &[Vec2]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    for index in 0..polygon.len() {
        let a = polygon[index];
        let b = polygon[(index + 1) % polygon.len()];
        if segments_cross_in_open_interval(from, to, a, b) {
            return true;
        }
    }
    false
}

fn segments_cross_in_open_interval(a0: Vec2, a1: Vec2, b0: Vec2, b1: Vec2) -> bool {
    fn cross(v: Vec2, w: Vec2) -> f32 {
        v.x * w.y - v.y * w.x
    }
    fn on_segment(p: Vec2, a: Vec2, b: Vec2) -> bool {
        p.x <= a.x.max(b.x) + 1e-4
            && p.x + 1e-4 >= a.x.min(b.x)
            && p.y <= a.y.max(b.y) + 1e-4
            && p.y + 1e-4 >= a.y.min(b.y)
    }
    const EPS: f32 = 1e-5;
    let d1 = cross(a1 - a0, b0 - a0);
    let d2 = cross(a1 - a0, b1 - a0);
    let d3 = cross(b1 - b0, a0 - b0);
    let d4 = cross(b1 - b0, a1 - b0);
    if ((d1 > EPS && d2 < -EPS) || (d1 < -EPS && d2 > EPS))
        && ((d3 > EPS && d4 < -EPS) || (d3 < -EPS && d4 > EPS))
    {
        return true;
    }
    if d1.abs() < EPS && on_segment(b0, a0, a1) && b0.distance(a0) > EPS && b0.distance(a1) > EPS {
        return true;
    }
    if d2.abs() < EPS && on_segment(b1, a0, a1) && b1.distance(a0) > EPS && b1.distance(a1) > EPS {
        return true;
    }
    if d3.abs() < EPS && on_segment(a0, b0, b1) && a0.distance(b0) > EPS && a0.distance(b1) > EPS {
        return true;
    }
    if d4.abs() < EPS && on_segment(a1, b0, b1) && a1.distance(b0) > EPS && a1.distance(b1) > EPS {
        return true;
    }
    false
}

/// Continuous interior segment validation: endpoints inside, no boundary crossing, agent clearance.
pub fn interior_segment_respects_region_boundary(
    store: &BuildingNavigationRuntimeStore,
    space_registry: &SpaceRegistry,
    layout: ChunkLayout,
    from: WorldPosition,
    to: WorldPosition,
    space_id: SpaceId,
    agent_radius_meters: f32,
) -> bool {
    let Some(region) = store.region_for_space(space_id) else {
        return false;
    };
    let from_xz = from.to_global(layout).xz();
    let to_xz = to.to_global(layout).xz();
    if !point_in_polygon_xz(&region.world_outline_xz, from_xz)
        || !point_in_polygon_xz(&region.world_outline_xz, to_xz)
    {
        return false;
    }
    if segment_crosses_polygon_boundary(from_xz, to_xz, &region.world_outline_xz) {
        return false;
    }
    let distance = from_xz.distance(to_xz);
    if distance <= 1e-4 {
        return interior_agent_fits_region(
            store,
            space_registry,
            layout,
            from,
            space_id,
            agent_radius_meters,
        );
    }
    let sample_spacing = agent_radius_meters * 0.5;
    let steps = ((distance / sample_spacing).ceil() as usize).max(1);
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let sample = from_xz.lerp(to_xz, t);
        let position = WorldPosition::from_global(Vec3::new(sample.x, 0.0, sample.y), layout);
        if !interior_agent_fits_region(
            store,
            space_registry,
            layout,
            position,
            space_id,
            agent_radius_meters,
        ) {
            return false;
        }
    }
    true
}

/// Whether a position lies inside an exterior-entrance portal trigger on the surface (NV2).
pub fn position_in_surface_entrance_portal(
    space_registry: &SpaceRegistry,
    layout: ChunkLayout,
    position: WorldPosition,
) -> bool {
    let agent_xz = {
        let global = position.to_global(layout);
        Vec2::new(global.x, global.z)
    };
    for portal_id in space_registry.portals_from_space(SpaceId::SURFACE) {
        let Some(portal) = space_registry.get_portal(*portal_id) else {
            continue;
        };
        if portal.portal_type != PortalType::ExteriorEntrance {
            continue;
        }
        if portal.from_space == SpaceId::SURFACE
            && portal.contains_agent_in_space(agent_xz, SpaceId::SURFACE, layout)
        {
            return true;
        }
    }
    false
}

/// Whether a grounded interior position lies inside the region polygon for its space (NV2).
pub fn interior_position_walkable(
    store: &BuildingNavigationRuntimeStore,
    space_registry: &SpaceRegistry,
    layout: ChunkLayout,
    position: WorldPosition,
    space_id: SpaceId,
) -> bool {
    if space_id.is_surface() {
        return true;
    }
    if let Some(region) = store.region_for_space(space_id) {
        let global = position.to_global(layout);
        return point_in_polygon_xz(&region.world_outline_xz, Vec2::new(global.x, global.z));
    }
    if let Some(space) = space_registry.get_space(space_id) {
        if let Some(building_id) = space.owning_building_id {
            if store.get(building_id).is_some() {
                return false;
            }
        }
    }
    true
}

/// Whether a grounded player click should command movement into an interior navigation region.
///
/// Goal-space resolution only — does not change unit current space.
pub fn interior_navigation_move_target_at_position(
    store: &BuildingNavigationRuntimeStore,
    space_registry: &SpaceRegistry,
    layout: ChunkLayout,
    position: WorldPosition,
) -> Option<SpaceId> {
    let space = resolve_navigation_space_at_position(store, space_registry, layout, position);
    if space.is_surface() {
        return None;
    }
    if !interior_position_walkable(store, space_registry, layout, position, space) {
        return None;
    }
    let global = position.to_global(layout);
    let floor_y = space_registry
        .get_space(space)
        .map(|space| space.floor_y_global)
        .unwrap_or(0.0);
    if (global.y - floor_y).abs() > FLOOR_ELEVATION_TOLERANCE_METERS {
        return None;
    }
    Some(space)
}

/// Resolve goal space and floor-grounded position for a move order (IN-11eR).
///
/// When the unit already occupies an interior space, same-region clicks keep that
/// space instead of falling back to surface terrain grounding.
pub fn resolve_move_goal_space(
    world: &crate::world::WorldData,
    start_space: SpaceId,
    target: WorldPosition,
) -> (SpaceId, WorldPosition) {
    let runtime = world.building_navigation_runtime();
    let registry = world.space_registry();
    let layout = world.layout();

    if !start_space.is_surface() {
        let interior_grounded =
            crate::world::ground_position_in_space(world, registry, start_space, target)
                .unwrap_or(target);
        if interior_position_walkable(runtime, registry, layout, interior_grounded, start_space) {
            return (start_space, interior_grounded);
        }
    }

    let mut goal_space = resolve_navigation_space_at_position(runtime, registry, layout, target);
    let mut grounded_goal =
        crate::world::ground_position_in_space(world, registry, goal_space, target)
            .unwrap_or(target);

    if !start_space.is_surface() && goal_space.is_surface() {
        if let Some(floor_grounded) =
            crate::world::ground_position_in_space(world, registry, start_space, target)
        {
            if let Some(interior_space) = interior_navigation_move_target_at_position(
                runtime,
                registry,
                layout,
                floor_grounded,
            ) {
                goal_space = interior_space;
                grounded_goal =
                    crate::world::ground_position_in_space(world, registry, interior_space, target)
                        .unwrap_or(floor_grounded);
            }
        }
    }

    (goal_space, grounded_goal)
}

/// Rebuild cached runtime outlines and portal poses after building placement changes (NV2).
pub fn reposition_building_navigation_runtime(
    world: &mut crate::world::WorldData,
    building_catalog: &super::super::catalog::BuildingCatalog,
    nav_catalog: &super::catalog::BuildingNavigationBlueprintCatalog,
    building_id: BuildingId,
) -> Result<(), String> {
    let record = world
        .get_building(building_id)
        .ok_or_else(|| format!("building #{} not found", building_id.raw()))?
        .clone();
    let runtime = world
        .building_navigation_runtime()
        .get(building_id)
        .ok_or_else(|| format!("building #{} has no navigation runtime", building_id.raw()))?
        .clone();
    let definition = building_catalog
        .get(&record.definition_id)
        .ok_or_else(|| format!("definition {} missing", record.definition_id.as_str()))?;
    let blueprint = nav_catalog.get(&runtime.blueprint_id).ok_or_else(|| {
        format!(
            "navigation blueprint {} missing",
            runtime.blueprint_id.as_str()
        )
    })?;

    let layout = world.layout();
    let model = building_model_world_transform(definition, &record.placement, layout);

    world
        .building_navigation_runtime_mut()
        .insert(build_navigation_runtime(
            building_id,
            blueprint,
            model,
            &runtime.space_keys,
            &runtime.portal_keys,
        ));

    let portals = blueprint_portal_templates(blueprint);
    let floor_y_by_key: std::collections::BTreeMap<String, f32> = blueprint
        .floors
        .iter()
        .map(|floor| (floor.key.clone(), floor.elevation_meters))
        .collect();

    for template in &portals {
        let Some(portal_id) = runtime.portal_keys.get(&template.key) else {
            continue;
        };
        let Some(portal) = world.space_registry_mut().get_portal_mut(*portal_id) else {
            continue;
        };
        let from_floor_y = floor_y_by_key
            .get(floor_key_from_region_space_key(&template.from_space_key))
            .copied()
            .unwrap_or(0.0);
        let from_local = Vec3::new(
            template.from_local_xz.x,
            from_floor_y,
            template.from_local_xz.y,
        );
        let from_global = model.transform_point(from_local);
        let to_global = model.transform_point(template.to_local_position);
        portal.from_center_global_xz = Vec2::new(from_global.x, from_global.z);
        portal.from_radius_meters = template.from_radius_meters;
        portal.to_position = WorldPosition::from_global(to_global, layout);
        portal.enabled = template.enabled;
    }

    for floor in &blueprint.floors {
        for region in &floor.regions {
            let key = region_space_key(&floor.key, &region.key);
            let Some(space_id) = runtime.space_keys.get(&key) else {
                continue;
            };
            let floor_world = model.transform_point(Vec3::new(0.0, floor.elevation_meters, 0.0));
            if let Some(space) = world.space_registry_mut().get_space_mut(*space_id) {
                space.reference_elevation = floor.elevation_meters;
                space.floor_y_global = floor_world.y;
                space.display_floor_label = floor.display_label.clone();
                space.visibility_group_id = floor.visibility_group_id;
                space.room_tag = region.room_tag.clone().or_else(|| floor.room_tag.clone());
            }
        }
    }

    crate::world::DoorStore::sync_building_door_portals(world, building_id)
        .map_err(|error| error.to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::building::navigation_blueprint::adapt::{
        blueprint_portal_templates, blueprint_space_templates,
    };
    use crate::world::building::navigation_blueprint::fixtures::{
        dual_doorway_navigation_blueprint, two_room_hut_navigation_blueprint,
    };
    use crate::world::building::navigation_blueprint::starter::two_story_hut_navigation_blueprint;
    use crate::world::{BuildingOwnership, BuildingSource};

    fn hut_runtime() -> BuildingNavigationRuntime {
        let blueprint = two_story_hut_navigation_blueprint();
        let spaces = blueprint_space_templates(&blueprint);
        let portals = blueprint_portal_templates(&blueprint);
        let mut registry = SpaceRegistry::new();
        let building = crate::world::BuildingRecord::new(
            BuildingId::new(1),
            crate::world::BuildingDefinitionId::new("hut"),
            crate::world::BuildingPlacement::new(
                WorldPosition::new(
                    crate::world::ChunkCoord::new(0, 0),
                    crate::world::LocalPosition::new(Vec3::ZERO),
                ),
                Quat::IDENTITY,
            ),
            BuildingOwnership::neutral(),
            100,
            BuildingSource::Authored,
        );
        let definition = crate::world::BuildingCatalog::default()
            .get(&crate::world::BuildingDefinitionId::new("hut"))
            .expect("hut definition")
            .clone();
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let (space_keys, portal_keys) = register_building_navigation_profile(
            &mut registry,
            &building,
            &definition,
            layout,
            &spaces,
            &portals,
        );
        build_navigation_runtime(
            BuildingId::new(1),
            &blueprint,
            Transform::from_translation(Vec3::new(20.0, 0.0, 20.0)),
            &space_keys,
            &portal_keys,
        )
    }

    #[test]
    fn point_inside_hut_ground_region() {
        let runtime = hut_runtime();
        let region = runtime
            .regions
            .iter()
            .find(|region| region.floor_key == "ground_interior")
            .expect("ground");
        let center = region.world_outline_xz[0]
            + (region.world_outline_xz[2] - region.world_outline_xz[0]) * 0.5;
        assert!(point_in_polygon_xz(&region.world_outline_xz, center));
    }

    #[test]
    fn resolve_space_picks_interior_over_surface() {
        let mut store = BuildingNavigationRuntimeStore::default();
        let runtime = hut_runtime();
        let region_space = runtime
            .regions
            .iter()
            .find(|region| region.floor_key == "ground_interior")
            .map(|region| (region.space_id, region.world_outline_xz.clone()))
            .expect("ground");
        store.insert(runtime);
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let centroid =
            region_space.1.iter().fold(Vec2::ZERO, |acc, v| acc + *v) / region_space.1.len() as f32;
        let position = WorldPosition::from_global(Vec3::new(centroid.x, 0.0, centroid.y), layout);
        let space =
            resolve_navigation_space_at_position(&store, &SpaceRegistry::new(), layout, position);
        assert_eq!(space, region_space.0);
    }

    #[test]
    fn resolve_start_space_uses_floor_elevation_for_overlapping_outlines() {
        let mut store = BuildingNavigationRuntimeStore::default();
        let runtime = hut_runtime();
        let ground_region = runtime
            .regions
            .iter()
            .find(|region| region.floor_key == "ground_interior")
            .map(|region| (region.space_id, region.world_outline_xz.clone()))
            .expect("ground");
        let upper = runtime
            .regions
            .iter()
            .find(|region| region.floor_key == "upper_interior")
            .map(|region| region.space_id)
            .expect("upper");
        store.insert(runtime);
        let mut registry = SpaceRegistry::new();
        let ground = ground_region.0;
        registry.insert_space(SpaceRecord {
            id: ground,
            owning_building_id: None,
            display_floor_label: "Ground".into(),
            visibility_group_id: 1,
            reference_elevation: 0.0,
            floor_y_global: 0.0,
            room_tag: None,
            enabled: true,
            walkable: true,
        });
        registry.insert_space(SpaceRecord {
            id: upper,
            owning_building_id: None,
            display_floor_label: "Upper".into(),
            visibility_group_id: 2,
            reference_elevation: 4.0,
            floor_y_global: 4.0,
            room_tag: None,
            enabled: true,
            walkable: true,
        });
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let shared_xz = Vec2::new(22.0, 22.0);
        let upper_pos =
            WorldPosition::from_global(Vec3::new(shared_xz.x, 4.0, shared_xz.y), layout);
        let ground_pos =
            WorldPosition::from_global(Vec3::new(shared_xz.x, 0.0, shared_xz.y), layout);
        assert_eq!(
            resolve_navigation_space_at_position(&store, &registry, layout, upper_pos),
            upper
        );

        assert_eq!(
            resolve_navigation_start_space(&store, &registry, layout, upper_pos, upper),
            upper
        );
        assert_eq!(
            resolve_navigation_start_space(&store, &registry, layout, ground_pos, ground),
            ground
        );
        assert_eq!(
            resolve_navigation_start_space(&store, &registry, layout, upper_pos, ground),
            upper
        );
    }

    #[test]
    fn sole_region_floor_alias_resolves() {
        let runtime = hut_runtime();
        assert!(runtime.space_keys.contains_key("ground_interior"));
        assert!(runtime.space_keys.contains_key("ground_interior/main"));
        assert_eq!(
            runtime.space_keys.get("ground_interior"),
            runtime.space_keys.get("ground_interior/main")
        );
    }

    #[test]
    fn dual_doorway_portals_reposition_by_key_independently() {
        let blueprint = dual_doorway_navigation_blueprint();
        let spaces = blueprint_space_templates(&blueprint);
        let portals = blueprint_portal_templates(&blueprint);
        let mut registry = SpaceRegistry::new();
        let building = crate::world::BuildingRecord::new(
            BuildingId::new(9),
            crate::world::BuildingDefinitionId::new("hut"),
            crate::world::BuildingPlacement::new(
                WorldPosition::new(
                    crate::world::ChunkCoord::new(0, 0),
                    crate::world::LocalPosition::new(Vec3::ZERO),
                ),
                Quat::IDENTITY,
            ),
            BuildingOwnership::neutral(),
            100,
            BuildingSource::Authored,
        );
        let definition = crate::world::BuildingCatalog::default()
            .get(&crate::world::BuildingDefinitionId::new("hut"))
            .expect("hut definition")
            .clone();
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let (space_keys, portal_keys) = register_building_navigation_profile(
            &mut registry,
            &building,
            &definition,
            layout,
            &spaces,
            &portals,
        );
        assert_eq!(portal_keys.len(), 2);
        let north = portal_keys.get("door_north").copied().expect("north");
        let south = portal_keys.get("door_south").copied().expect("south");
        assert_ne!(north, south);
        let runtime = build_navigation_runtime(
            BuildingId::new(9),
            &blueprint,
            Transform::IDENTITY,
            &space_keys,
            &portal_keys,
        );
        assert_eq!(runtime.portal_keys.len(), 2);
    }

    #[test]
    fn two_room_hut_registers_two_regions() {
        let blueprint = two_room_hut_navigation_blueprint();
        let spaces = blueprint_space_templates(&blueprint);
        assert_eq!(spaces.len(), 2);
    }

    #[test]
    fn concave_shortcut_crosses_polygon_boundary() {
        let outline = vec![
            Vec2::new(20.0, 20.0),
            Vec2::new(30.0, 20.0),
            Vec2::new(30.0, 30.0),
            Vec2::new(20.0, 30.0),
            Vec2::new(20.0, 26.0),
            Vec2::new(24.0, 26.0),
            Vec2::new(24.0, 24.0),
            Vec2::new(20.0, 24.0),
        ];
        let inside_a = Vec2::new(22.0, 22.0);
        let inside_b = Vec2::new(28.0, 28.0);
        assert!(point_in_polygon_xz(&outline, inside_a));
        assert!(point_in_polygon_xz(&outline, inside_b));
        assert!(segment_crosses_polygon_boundary(
            inside_a, inside_b, &outline
        ));
    }

    #[test]
    fn tracked_interior_stays_on_floor_without_polygon_fallback() {
        let mut store = BuildingNavigationRuntimeStore::default();
        let runtime = hut_runtime();
        let ground = runtime
            .regions
            .iter()
            .find(|region| region.floor_key == "ground_interior")
            .map(|region| region.space_id)
            .expect("ground");
        store.insert(runtime);
        let mut registry = SpaceRegistry::new();
        let floor_y = 0.0;
        registry.insert_space(SpaceRecord {
            id: ground,
            owning_building_id: Some(BuildingId::new(1)),
            display_floor_label: "Ground".into(),
            visibility_group_id: 1,
            reference_elevation: floor_y,
            floor_y_global: floor_y,
            room_tag: None,
            enabled: true,
            walkable: true,
        });
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let outside_polygon = WorldPosition::from_global(Vec3::new(5.0, floor_y, 5.0), layout);
        assert_eq!(
            resolve_navigation_start_space(&store, &registry, layout, outside_polygon, ground),
            ground,
            "interior authority must not fall back to surface merely for being outside polygon"
        );
    }
}
