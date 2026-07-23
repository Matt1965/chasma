//! Runtime navigation data derived from building navigation blueprints (NV1.3).

use std::collections::HashMap;

use bevy::prelude::*;

use super::definition::BuildingNavigationBlueprint;
use super::id::BuildingNavigationBlueprintId;
use super::{BlueprintPortalTemplate, BlueprintSpaceTemplate, blueprint_portal_templates};
use crate::world::building::catalog::BuildingDefinition;
use crate::world::building::record::BuildingRecord;
use crate::world::space::{PortalRecord, PortalType, SpaceRecord, SpaceRegistry};
use crate::world::{
    BuildingId, ChunkLayout, SpaceId, WorldPosition, building_model_world_transform,
};

/// One navigable floor registered from a blueprint.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeNavigationFloor {
    pub space_id: SpaceId,
    pub floor_id: i32,
    pub floor_key: String,
    pub elevation_meters: f32,
    /// Closed polygon in world XZ (building-local outline transformed at activation).
    pub world_outline_xz: Vec<Vec2>,
}

/// Authoritative runtime navigation for one building instance.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildingNavigationRuntime {
    pub building_id: BuildingId,
    pub blueprint_id: BuildingNavigationBlueprintId,
    pub model_transform: Transform,
    pub floors: Vec<RuntimeNavigationFloor>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BuildingNavigationRuntimeStore {
    by_building: HashMap<BuildingId, BuildingNavigationRuntime>,
    space_to_building: HashMap<u32, BuildingId>,
}

impl BuildingNavigationRuntimeStore {
    pub fn insert(&mut self, runtime: BuildingNavigationRuntime) {
        for floor in &runtime.floors {
            self.space_to_building
                .insert(floor.space_id.raw(), runtime.building_id);
        }
        self.by_building.insert(runtime.building_id, runtime);
    }

    pub fn remove_building(&mut self, building_id: BuildingId) {
        if let Some(runtime) = self.by_building.remove(&building_id) {
            for floor in &runtime.floors {
                self.space_to_building.remove(&floor.space_id.raw());
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

    pub fn floor_for_space(&self, space_id: SpaceId) -> Option<&RuntimeNavigationFloor> {
        let runtime = self.get_for_space(space_id)?;
        runtime.floors.iter().find(|floor| floor.space_id == space_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &BuildingNavigationRuntime> {
        self.by_building.values()
    }
}

pub fn build_navigation_runtime(
    building_id: BuildingId,
    blueprint: &BuildingNavigationBlueprint,
    model_transform: Transform,
    space_keys: &std::collections::BTreeMap<String, SpaceId>,
) -> BuildingNavigationRuntime {
    let floors = blueprint
        .floors
        .iter()
        .filter_map(|floor| {
            let space_id = *space_keys.get(&floor.key)?;
            let world_outline_xz = floor
                .walkable_outline
                .vertices_xz
                .iter()
                .map(|&[x, z]| {
                    let local = Vec3::new(x, floor.elevation_meters, z);
                    let world = model_transform.transform_point(local);
                    Vec2::new(world.x, world.z)
                })
                .collect();
            Some(RuntimeNavigationFloor {
                space_id,
                floor_id: floor.floor_id,
                floor_key: floor.key.clone(),
                elevation_meters: floor.elevation_meters,
                world_outline_xz,
            })
        })
        .collect();

    BuildingNavigationRuntime {
        building_id,
        blueprint_id: blueprint.id.clone(),
        model_transform,
        floors,
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
    let floor_y_by_key: std::collections::BTreeMap<&str, f32> = spaces
        .iter()
        .map(|space| (space.key.as_str(), space.local_floor_y))
        .collect();

    let mut key_to_space: std::collections::BTreeMap<&str, SpaceId> =
        std::collections::BTreeMap::from([("surface", SpaceId::SURFACE)]);

    let mut space_records = Vec::new();
    for template in spaces {
        let id = registry.allocate_space_id();
        key_to_space.insert(template.key.as_str(), id);
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

    let mut portal_records = Vec::new();
    let mut portal_key_to_id: std::collections::BTreeMap<&str, crate::world::PortalId> =
        std::collections::BTreeMap::new();
    for template in portals {
        let from_space = *key_to_space
            .get(template.from_space_key.as_str())
            .expect("from space key");
        let to_space = *key_to_space
            .get(template.to_space_key.as_str())
            .expect("to space key");
        let from_floor_y = floor_y_by_key
            .get(template.from_space_key.as_str())
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
        portal_key_to_id.insert(template.key.as_str(), portal_id);
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
            enabled: true,
            owning_building_id: Some(building.id),
        });
    }

    registry.register_building_spaces(building.id, space_records, portal_records);
    (
        key_to_space
            .into_iter()
            .map(|(key, id)| (key.to_string(), id))
            .collect(),
        portal_key_to_id
            .into_iter()
            .map(|(key, id)| (key.to_string(), id))
            .collect(),
    )
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

/// Resolve which navigable space contains a world position (interior floors first).
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
        for floor in &runtime.floors {
            if !point_in_polygon_xz(&floor.world_outline_xz, point) {
                continue;
            }
            let floor_y = space_registry
                .get_space(floor.space_id)
                .map(|space| space.floor_y_global)
                .unwrap_or(floor.elevation_meters);
            let y_delta = (global.y - floor_y).abs();
            if best.is_none_or(|(_, best_delta)| y_delta < best_delta) {
                best = Some((floor.space_id, y_delta));
            }
        }
    }
    best.map(|(space, _)| space).unwrap_or(SpaceId::SURFACE)
}

/// Resolve the start space for pathfinding, reconciling tracked state with position (NV2).
pub fn resolve_navigation_start_space(
    store: &BuildingNavigationRuntimeStore,
    space_registry: &SpaceRegistry,
    layout: ChunkLayout,
    position: WorldPosition,
    tracked_space: SpaceId,
) -> SpaceId {
    let resolved = resolve_navigation_space_at_position(store, space_registry, layout, position);
    if !resolved.is_surface() {
        return resolved;
    }
    if tracked_space.is_surface() {
        return SpaceId::SURFACE;
    }
    if interior_position_walkable(store, space_registry, layout, position, tracked_space) {
        return tracked_space;
    }
    SpaceId::SURFACE
}

/// Whether a position lies inside an enabled exterior-entrance portal on the surface (NV2).
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
        if !portal.enabled || portal.portal_type != PortalType::ExteriorEntrance {
            continue;
        }
        if portal.from_space == SpaceId::SURFACE && portal.contains_agent_global(agent_xz) {
            return true;
        }
        if portal.bidirectional
            && portal.to_space == SpaceId::SURFACE
            && portal.contains_agent_global(agent_xz)
        {
            return true;
        }
    }
    false
}

/// Whether a grounded interior position lies inside the blueprint walkable outline (NV2).
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
    if let Some(floor) = store.floor_for_space(space_id) {
        let global = position.to_global(layout);
        return point_in_polygon_xz(&floor.world_outline_xz, Vec2::new(global.x, global.z));
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
    let blueprint = nav_catalog
        .get(&runtime.blueprint_id)
        .ok_or_else(|| {
            format!(
                "navigation blueprint {} missing",
                runtime.blueprint_id.as_str()
            )
        })?;

    let layout = world.layout();
    let model = building_model_world_transform(definition, &record.placement, layout);

    let mut space_keys = std::collections::BTreeMap::new();
    for floor in &runtime.floors {
        space_keys.insert(floor.floor_key.clone(), floor.space_id);
    }
    world
        .building_navigation_runtime_mut()
        .insert(build_navigation_runtime(
            building_id,
            blueprint,
            model,
            &space_keys,
        ));

    let portals = blueprint_portal_templates(blueprint);
    let floor_y_by_key: std::collections::BTreeMap<&str, f32> = blueprint
        .floors
        .iter()
        .map(|floor| (floor.key.as_str(), floor.elevation_meters))
        .collect();
    let key_to_space: std::collections::BTreeMap<&str, SpaceId> = space_keys
        .iter()
        .map(|(key, id)| (key.as_str(), *id))
        .chain([("surface", SpaceId::SURFACE)])
        .collect();

    let portal_ids: Vec<_> = world
        .space_registry()
        .portals()
        .filter(|(_, portal)| portal.owning_building_id == Some(building_id))
        .map(|(id, _)| *id)
        .collect();

    for template in &portals {
        let from_space = *key_to_space
            .get(template.from_space_key.as_str())
            .ok_or_else(|| format!("missing from space `{}`", template.from_space_key))?;
        let to_space = *key_to_space
            .get(template.to_space_key.as_str())
            .ok_or_else(|| format!("missing to space `{}`", template.to_space_key))?;
        let from_floor_y = floor_y_by_key
            .get(template.from_space_key.as_str())
            .copied()
            .unwrap_or(0.0);
        let from_local = Vec3::new(
            template.from_local_xz.x,
            from_floor_y,
            template.from_local_xz.y,
        );
        let from_global = model.transform_point(from_local);
        let to_global = model.transform_point(template.to_local_position);
        for portal_id in &portal_ids {
            let Some(portal) = world.space_registry_mut().get_portal_mut(*portal_id) else {
                continue;
            };
            if portal.from_space == from_space
                && portal.to_space == to_space
                && portal.portal_type == template.portal_type
            {
                portal.from_center_global_xz = Vec2::new(from_global.x, from_global.z);
                portal.from_radius_meters = template.from_radius_meters;
                portal.to_position = WorldPosition::from_global(to_global, layout);
            } else if portal.bidirectional
                && portal.from_space == to_space
                && portal.to_space == from_space
                && portal.portal_type == template.portal_type
            {
                portal.from_center_global_xz = Vec2::new(from_global.x, from_global.z);
                portal.from_radius_meters = template.from_radius_meters;
                portal.to_position = WorldPosition::from_global(to_global, layout);
            }
        }
    }

    for template in blueprint.floors.iter() {
        let Some(space_id) = space_keys.get(&template.key) else {
            continue;
        };
        let floor_world = model.transform_point(Vec3::new(0.0, template.elevation_meters, 0.0));
        if let Some(space) = world.space_registry_mut().get_space_mut(*space_id) {
            space.reference_elevation = template.elevation_meters;
            space.floor_y_global = floor_world.y;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::building::navigation_blueprint::starter::two_story_hut_navigation_blueprint;
    use crate::world::{BuildingOwnership, BuildingSource};

    fn hut_runtime() -> BuildingNavigationRuntime {
        let blueprint = two_story_hut_navigation_blueprint();
        let mut space_keys = std::collections::BTreeMap::new();
        space_keys.insert("ground_interior".to_string(), SpaceId::new(1));
        space_keys.insert("upper_interior".to_string(), SpaceId::new(2));
        build_navigation_runtime(
            BuildingId::new(1),
            &blueprint,
            Transform::from_translation(Vec3::new(20.0, 0.0, 20.0)),
            &space_keys,
        )
    }

    #[test]
    fn point_inside_hut_ground_floor() {
        let runtime = hut_runtime();
        let floor = runtime
            .floors
            .iter()
            .find(|f| f.floor_key == "ground_interior")
            .expect("ground");
        let center = floor.world_outline_xz[0]
            + (floor.world_outline_xz[2] - floor.world_outline_xz[0]) * 0.5;
        assert!(point_in_polygon_xz(&floor.world_outline_xz, center));
    }

    #[test]
    fn resolve_space_picks_interior_over_surface() {
        let mut store = BuildingNavigationRuntimeStore::default();
        store.insert(hut_runtime());
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let position = WorldPosition::new(
            crate::world::ChunkCoord::new(0, 0),
            crate::world::LocalPosition::new(Vec3::new(22.0, 0.0, 22.0)),
        );
        let space = resolve_navigation_space_at_position(&store, &SpaceRegistry::default(), layout, position);
        assert_ne!(space, SpaceId::SURFACE);
    }
}
