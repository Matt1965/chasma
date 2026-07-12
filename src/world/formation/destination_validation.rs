//! Move destination validation against occupied unit footprints (DV3).
//!
//! Projects proposed move targets to the nearest valid XZ position outside
//! other units' collision radii. Deterministic and radius-aware.

use std::collections::HashMap;

use bevy::prelude::Vec2;

use crate::world::{ChunkLayout, UnitCatalog, UnitId, WorldData, WorldPosition};

const POSITION_EPSILON_SQ: f32 = 1e-8;
const MAX_PUSH_PASSES: usize = 12;
/// Search radius for nearby occupants (meters) — covers large unit pairs.
const OCCUPANT_QUERY_PADDING_METERS: f32 = 4.0;

/// Minimum center-to-center separation for two unit footprints.
pub fn collision_separation_meters(radius_a: f32, radius_b: f32) -> f32 {
    radius_a + radius_b
}

/// Collision radius for a live unit instance.
pub fn unit_collision_radius(world: &WorldData, catalog: &UnitCatalog, unit_id: UnitId) -> f32 {
    world
        .get_unit(unit_id)
        .and_then(|record| catalog.get(&record.definition_id))
        .map(|definition| definition.collision_radius_meters)
        .unwrap_or(0.5)
}

fn xz_of(position: WorldPosition, layout: ChunkLayout) -> Vec2 {
    let global = position.to_global(layout);
    Vec2::new(global.x, global.z)
}

fn position_from_xz(xz: Vec2, reference: WorldPosition, layout: ChunkLayout) -> WorldPosition {
    let mut global = reference.to_global(layout);
    global.x = xz.x;
    global.z = xz.y;
    WorldPosition::from_global(global, layout)
}

fn escape_xz(mover_id: UnitId, occupier_id: UnitId, occupier_xz: Vec2, min_sep: f32) -> Vec2 {
    let hash = mover_id
        .raw()
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(occupier_id.raw());
    let angle = (hash as f32 / u64::MAX as f32) * std::f32::consts::TAU;
    occupier_xz + Vec2::new(angle.cos(), angle.sin()) * min_sep
}

fn occupant_position(
    occupier_id: UnitId,
    world: &WorldData,
    batch_resolved: &HashMap<UnitId, WorldPosition>,
) -> Option<WorldPosition> {
    batch_resolved.get(&occupier_id).copied().or_else(|| {
        world
            .get_unit(occupier_id)
            .map(|record| record.placement.position)
    })
}

/// Project `proposed` to the nearest valid destination outside other unit footprints.
pub fn resolve_move_destination(
    mover_id: UnitId,
    proposed: WorldPosition,
    world: &WorldData,
    catalog: &UnitCatalog,
    layout: ChunkLayout,
    batch_resolved: &HashMap<UnitId, WorldPosition>,
) -> WorldPosition {
    let mover_radius = unit_collision_radius(world, catalog, mover_id);
    let query_radius = mover_radius + OCCUPANT_QUERY_PADDING_METERS;
    let mut occupant_ids = world.query_units_in_radius(proposed, query_radius, Some(mover_id));
    for id in batch_resolved.keys().copied() {
        if id != mover_id && !occupant_ids.contains(&id) {
            occupant_ids.push(id);
        }
    }
    occupant_ids.sort_unstable();

    let mut occupants = Vec::new();
    for occupier_id in occupant_ids {
        let Some(position) = occupant_position(occupier_id, world, batch_resolved) else {
            continue;
        };
        let radius = unit_collision_radius(world, catalog, occupier_id);
        occupants.push((occupier_id, xz_of(position, layout), radius));
    }

    let mut resolved = xz_of(proposed, layout);
    for _ in 0..MAX_PUSH_PASSES {
        let mut changed = false;
        for &(occupier_id, occupier_xz, occupier_radius) in &occupants {
            let min_sep = collision_separation_meters(mover_radius, occupier_radius);
            let delta = resolved - occupier_xz;
            let dist_sq = delta.length_squared();
            if dist_sq + 1e-4 >= min_sep * min_sep {
                continue;
            }
            resolved = if dist_sq < POSITION_EPSILON_SQ {
                escape_xz(mover_id, occupier_id, occupier_xz, min_sep)
            } else {
                occupier_xz + delta.normalize() * min_sep
            };
            changed = true;
        }
        if !changed {
            break;
        }
    }

    position_from_xz(resolved, proposed, layout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, Heightfield, LocalPosition, UnitDefinitionId, UnitSource,
        create_unit,
    };
    use bevy::prelude::Vec3;

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn flat_world() -> WorldData {
        let mut world = WorldData::new(layout());
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn spawn(catalog: &UnitCatalog, world: &mut WorldData, x: f32, z: f32) -> UnitId {
        create_unit(
            catalog,
            world,
            &UnitDefinitionId::new("wolf"),
            pos(x, z),
            UnitSource::Authored,
        )
        .unwrap()
        .id
    }

    fn xz_distance(a: WorldPosition, b: WorldPosition) -> f32 {
        let la = a.to_global(layout());
        let lb = b.to_global(layout());
        Vec2::new(la.x - lb.x, la.z - lb.z).length()
    }

    #[test]
    fn move_onto_idle_unit_projects_outside_footprint() {
        let catalog = UnitCatalog::default();
        let mut world = flat_world();
        let idle = spawn(&catalog, &mut world, 20.0, 20.0);
        let mover = spawn(&catalog, &mut world, 4.0, 4.0);
        let click = world.get_unit(idle).unwrap().placement.position;
        let resolved =
            resolve_move_destination(mover, click, &world, &catalog, layout(), &HashMap::new());
        let wolf_radius = catalog
            .get(&UnitDefinitionId::new("wolf"))
            .unwrap()
            .collision_radius_meters;
        let min_sep = collision_separation_meters(wolf_radius, wolf_radius);
        assert!(xz_distance(resolved, click) + 1e-3 >= min_sep);
        assert_ne!(resolved, click);
    }

    #[test]
    fn destination_projection_is_deterministic() {
        let catalog = UnitCatalog::default();
        let mut world = flat_world();
        let idle = spawn(&catalog, &mut world, 15.0, 15.0);
        let mover = spawn(&catalog, &mut world, 2.0, 2.0);
        let click = world.get_unit(idle).unwrap().placement.position;
        let a = resolve_move_destination(mover, click, &world, &catalog, layout(), &HashMap::new());
        let b = resolve_move_destination(mover, click, &world, &catalog, layout(), &HashMap::new());
        assert_eq!(a, b);
    }

    #[test]
    fn empty_terrain_destination_is_unchanged() {
        let catalog = UnitCatalog::default();
        let world = flat_world();
        let mover = UnitId::new(99);
        let target = pos(40.0, 40.0);
        let resolved =
            resolve_move_destination(mover, target, &world, &catalog, layout(), &HashMap::new());
        assert_eq!(resolved, target);
    }
}
