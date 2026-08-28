//! Presentation-only visual yaw interpolation toward authoritative facing (UNIT-TURN-1).

use bevy::prelude::*;

use crate::simulation::SimulationControlState;
use crate::units::animation::presentation_advance_seconds;
use crate::world::{
    UnitCatalog, UnitState, WorldData, step_rotation_yaw_toward, unit_visual_rotation,
    yaw_radians_from_rotation,
};

use super::components::{UnitRenderEntity, UnitRenderMetadata, UnitVisualFacing};

/// Smooth render yaw toward authoritative [`UnitPlacement`](crate::world::UnitPlacement) facing.
pub fn update_unit_visual_facing(
    time: Res<Time>,
    control: Res<SimulationControlState>,
    world: Res<WorldData>,
    catalog: Res<UnitCatalog>,
    mut query: Query<(
        &UnitRenderEntity,
        &UnitRenderMetadata,
        &mut UnitVisualFacing,
        &mut Transform,
    )>,
) {
    let delta = presentation_advance_seconds(&control, time.delta_secs());
    if delta <= 0.0 {
        return;
    }

    for (marker, metadata, mut visual, mut transform) in &mut query {
        let Some(record) = world.get_unit(marker.unit_id) else {
            continue;
        };
        if matches!(record.state, UnitState::Dead) {
            continue;
        }
        let Some(definition) = catalog.get(&metadata.definition_id) else {
            continue;
        };

        let turn_speed_rad = definition.turn_speed_degrees_per_second.to_radians();
        visual.rotation = step_rotation_yaw_toward(
            visual.rotation,
            record.placement.rotation,
            turn_speed_rad,
            delta,
        );
        transform.rotation = unit_visual_rotation(definition, visual.rotation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::SimulationControlState;
    use crate::terrain::residency::ChunkResidencyTracker;
    use crate::units::UnitSceneAssets;
    use crate::units::spawn::UnitRenderIndex;
    use crate::units::sync::{UnitSyncOverrides, sync_unit_render_entities};
    use crate::world::authoring_transform::QuantizedOrientation;
    use crate::world::facing_rotation_from_direction_xz;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, Heightfield, LocalPosition, UnitCatalog,
        UnitDefinition, UnitDefinitionId, UnitId, UnitRenderKey, UnitSource, WeaponDefinitionId,
        WorldConfig, WorldData, WorldPosition, create_unit, model_forward_xz,
    };
    use bevy::asset::AssetPlugin;
    use bevy::prelude::{App, MinimalPlugins, Quat, StandardMaterial, Update, Vec2, Vec3};
    use std::collections::HashMap;
    use std::f32::consts::FRAC_PI_2;

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn sample_definition() -> UnitDefinition {
        UnitDefinition::new_test(
            UnitDefinitionId::new("wolf"),
            "Wolf",
            "Wild",
            2,
            5,
            5,
            4,
            6,
            3,
            7,
            2,
            3,
            26.5,
            "Elite",
            4.0,
            0.6,
            40.0,
            WeaponDefinitionId::new("weapon_wolf_bite"),
            true,
            UnitRenderKey::reserved("wolf"),
        )
    }

    fn setup_app(definition: UnitDefinition) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_resource::<WorldConfig>();
        app.init_resource::<WorldData>();
        app.init_resource::<UnitCatalog>();
        app.init_resource::<ChunkResidencyTracker>();
        app.init_resource::<UnitRenderIndex>();
        app.init_resource::<Assets<Scene>>();
        app.init_resource::<Assets<StandardMaterial>>();
        app.init_resource::<SimulationControlState>();
        app.insert_resource(UnitCatalog::from_definitions(vec![definition]).unwrap());
        app.insert_resource(UnitSyncOverrides {
            treat_scenes_loaded: true,
        });
        app.add_systems(
            Update,
            (sync_unit_render_entities, update_unit_visual_facing).chain(),
        );
        app
    }

    fn insert_terrain(world: &mut WorldData, x: i32, z: i32) {
        let samples = vec![8.0; 9];
        let heightfield = Heightfield::from_samples(3, 128.0, samples).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(x, z)),
            ChunkData::new(heightfield, Vec::new()),
        );
    }

    fn prepare_unit(app: &mut App, x: i32, z: i32) -> UnitId {
        let chunk = ChunkId::new(ChunkCoord::new(x, z));
        let scene = {
            let mut scenes = app.world_mut().resource_mut::<Assets<Scene>>();
            scenes.add(Scene::new(World::new()))
        };
        app.insert_resource(UnitSceneAssets::from_test_scenes(HashMap::from([(
            UnitDefinitionId::new("wolf"),
            scene,
        )])));
        let catalog = app.world().resource::<UnitCatalog>().clone();
        let id = {
            let mut world = app.world_mut().resource_mut::<WorldData>();
            insert_terrain(&mut world, x, z);
            create_unit(
                &catalog,
                &mut world,
                &UnitDefinitionId::new("wolf"),
                WorldPosition::new(
                    ChunkCoord::new(x, z),
                    LocalPosition::new(Vec3::new(20.0, 8.0, 30.0)),
                ),
                UnitSource::Authored,
            )
            .unwrap()
            .id
        };
        app.world_mut()
            .resource_mut::<ChunkResidencyTracker>()
            .mark_resident(chunk);
        id
    }

    #[test]
    fn spawn_initializes_visual_facing_to_authoritative() {
        let mut app = setup_app(sample_definition());
        let unit_id = prepare_unit(&mut app, 0, 0);
        app.update();
        let entity = app.world().resource::<UnitRenderIndex>().0[&unit_id];
        let record = app
            .world()
            .resource::<WorldData>()
            .get_unit(unit_id)
            .unwrap()
            .clone();
        let visual = app
            .world()
            .entity(entity)
            .get::<UnitVisualFacing>()
            .unwrap();
        assert_eq!(visual.rotation, record.placement.rotation);
    }

    fn step_presentation(app: &mut App) {
        {
            let mut control = app.world_mut().resource_mut::<SimulationControlState>();
            control.paused = true;
            control.step_once = true;
        }
        app.update();
        {
            let mut control = app.world_mut().resource_mut::<SimulationControlState>();
            control.step_once = false;
        }
    }

    fn update_with_delta(app: &mut App, delta_seconds: f32) {
        let steps =
            ((delta_seconds / crate::simulation::SIMULATION_TICK_SECONDS).ceil() as u32).max(1);
        for _ in 0..steps {
            step_presentation(app);
        }
    }

    fn set_authoritative_facing_east(app: &mut App, unit_id: UnitId) {
        let rotation = facing_rotation_from_direction_xz(Vec2::new(1.0, 0.0));
        app.world_mut()
            .resource_mut::<WorldData>()
            .set_unit_facing_for_test(unit_id, rotation)
            .unwrap();
    }

    fn set_authoritative_facing_south(app: &mut App, unit_id: UnitId) {
        let rotation = facing_rotation_from_direction_xz(Vec2::new(0.0, 1.0));
        app.world_mut()
            .resource_mut::<WorldData>()
            .set_unit_facing_for_test(unit_id, rotation)
            .unwrap();
    }

    #[test]
    fn visual_catches_up_without_mutating_placement() {
        let mut definition = sample_definition();
        definition.turn_speed_degrees_per_second = 360.0;
        let mut app = setup_app(definition);
        let unit_id = prepare_unit(&mut app, 0, 0);
        app.update();
        let entity = app.world().resource::<UnitRenderIndex>().0[&unit_id];
        {
            let mut visual = app.world_mut().entity_mut(entity);
            visual.get_mut::<UnitVisualFacing>().unwrap().rotation = Quat::IDENTITY;
        }
        set_authoritative_facing_east(&mut app, unit_id);
        update_with_delta(&mut app, 0.3);
        let placement = app
            .world()
            .resource::<WorldData>()
            .get_unit(unit_id)
            .unwrap()
            .placement
            .rotation;
        let visual = app
            .world()
            .entity(entity)
            .get::<UnitVisualFacing>()
            .unwrap();
        let target_forward = model_forward_xz(placement);
        assert!((target_forward - Vec2::new(1.0, 0.0)).length() < 1e-3);
        assert!(
            (model_forward_xz(visual.rotation) - target_forward).length() < 0.05,
            "visual should catch up toward authoritative facing"
        );
    }

    #[test]
    fn idle_finishes_presentation_catch_up() {
        let mut definition = sample_definition();
        definition.turn_speed_degrees_per_second = 720.0;
        let mut app = setup_app(definition);
        let unit_id = prepare_unit(&mut app, 0, 0);
        app.update();
        let entity = app.world().resource::<UnitRenderIndex>().0[&unit_id];
        set_authoritative_facing_south(&mut app, unit_id);
        {
            let mut visual = app.world_mut().entity_mut(entity);
            visual.get_mut::<UnitVisualFacing>().unwrap().rotation = Quat::IDENTITY;
        }
        for _ in 0..12 {
            update_with_delta(&mut app, 0.05);
        }
        let visual = app
            .world()
            .entity(entity)
            .get::<UnitVisualFacing>()
            .unwrap();
        let target = app
            .world()
            .resource::<WorldData>()
            .get_unit(unit_id)
            .unwrap()
            .placement
            .rotation;
        let target_forward = model_forward_xz(target);
        assert!(
            (model_forward_xz(visual.rotation) - target_forward).length() < 0.05,
            "idle should allow visual catch-up"
        );
    }

    #[test]
    fn dead_unit_does_not_continue_turning() {
        let mut definition = sample_definition();
        definition.turn_speed_degrees_per_second = 720.0;
        let mut app = setup_app(definition);
        let unit_id = prepare_unit(&mut app, 0, 0);
        app.update();
        let entity = app.world().resource::<UnitRenderIndex>().0[&unit_id];
        let frozen = Quat::from_rotation_y(0.25);
        set_authoritative_facing_south(&mut app, unit_id);
        {
            let mut visual = app.world_mut().entity_mut(entity);
            visual.get_mut::<UnitVisualFacing>().unwrap().rotation = frozen;
        }
        app.world_mut()
            .resource_mut::<WorldData>()
            .set_unit_state(unit_id, UnitState::Dead)
            .unwrap();
        update_with_delta(&mut app, 0.3);
        let visual = app
            .world()
            .entity(entity)
            .get::<UnitVisualFacing>()
            .unwrap();
        assert_eq!(visual.rotation, frozen);
    }

    #[test]
    fn robot_correction_stays_upright_while_turning() {
        let mut definition = sample_definition();
        definition.turn_speed_degrees_per_second = 540.0;
        definition.asset_sizing.rotation_correction =
            QuantizedOrientation::from_degrees(90.0, 0.0, 0.0).unwrap();
        let mut app = setup_app(definition);
        let unit_id = prepare_unit(&mut app, 0, 0);
        app.update();
        let entity = app.world().resource::<UnitRenderIndex>().0[&unit_id];
        set_authoritative_facing_east(&mut app, unit_id);
        for _ in 0..6 {
            update_with_delta(&mut app, 0.08);
        }
        let transform = app.world().entity(entity).get::<Transform>().unwrap();
        let up = transform.rotation * Vec3::Y;
        assert!(
            up.dot(Vec3::Y) > 0.99,
            "model correction must stay upright while visual yaw turns, got up={up:?}"
        );
    }

    #[test]
    fn smoothed_forward_follows_visual_not_instant_snap() {
        let mut definition = sample_definition();
        definition.turn_speed_degrees_per_second = 90.0;
        let mut app = setup_app(definition);
        let unit_id = prepare_unit(&mut app, 0, 0);
        app.update();
        let entity = app.world().resource::<UnitRenderIndex>().0[&unit_id];
        {
            let mut visual = app.world_mut().entity_mut(entity);
            visual.get_mut::<UnitVisualFacing>().unwrap().rotation = Quat::IDENTITY;
        }
        set_authoritative_facing_east(&mut app, unit_id);
        update_with_delta(&mut app, 0.05);
        let visual = app
            .world()
            .entity(entity)
            .get::<UnitVisualFacing>()
            .unwrap();
        let forward = model_forward_xz(visual.rotation);
        assert!(
            forward.x.abs() < 0.8,
            "visual forward should not snap instantly to east, got {forward:?}"
        );
    }
}
