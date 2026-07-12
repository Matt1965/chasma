//! Death presentation lifecycle — presentation only (A3).

use bevy::prelude::*;

use crate::units::components::{UnitRenderEntity, UnitRenderMetadata, UnitSceneRoot};
use crate::units::spawn::UnitRenderIndex;
use crate::world::{AnimationProfileCatalog, UnitCatalog};

use super::components::{AnimationPlaybackPending, DeathPresentation, UnitAnimationLayering};
use super::off_screen_death::may_begin_death_presentation_on_existing_root;
use super::presentation_time::presentation_advance_seconds;
use super::settings::UnitAnimationSettings;
use crate::simulation::SimulationControlState;

/// Begin death presentation for units removed from authoritative world data (A3).
pub fn begin_death_presentations(
    mut commands: Commands,
    world: Res<crate::world::WorldData>,
    catalog: Res<UnitCatalog>,
    profiles: Res<AnimationProfileCatalog>,
    settings: Res<UnitAnimationSettings>,
    mut index: ResMut<UnitRenderIndex>,
    roots: Query<
        (Entity, &UnitRenderEntity, &UnitRenderMetadata),
        (With<UnitSceneRoot>, Without<DeathPresentation>),
    >,
) {
    if !settings.enabled {
        return;
    }

    for (entity, marker, metadata) in &roots {
        if world.get_unit(marker.unit_id).is_some() {
            continue;
        }
        if !may_begin_death_presentation_on_existing_root(index.0.contains_key(&marker.unit_id)) {
            continue;
        }

        let Some(definition) = catalog.get(&metadata.definition_id) else {
            despawn_immediate(&mut commands, &mut index, entity, marker.unit_id);
            continue;
        };
        let Some(profile_id) = &definition.animation_profile_id else {
            despawn_immediate(&mut commands, &mut index, entity, marker.unit_id);
            continue;
        };
        let Some(profile) = profiles.get(profile_id) else {
            despawn_immediate(&mut commands, &mut index, entity, marker.unit_id);
            continue;
        };

        let has_death_clip = profile.resolve_death_clip_name().is_some();
        let hold_seconds = if has_death_clip {
            settings.death_clip_hold_seconds
        } else {
            settings.death_freeze_hold_seconds
        };

        index.0.remove(&marker.unit_id);

        commands.entity(entity).insert((
            DeathPresentation {
                definition_id: metadata.definition_id.clone(),
                profile_id: profile_id.clone(),
                remaining_seconds: hold_seconds,
                freeze_pose: !has_death_clip,
            },
            UnitAnimationLayering::full_body_exclusive(),
            AnimationPlaybackPending,
        ));
    }
}

fn despawn_immediate(
    commands: &mut Commands,
    index: &mut UnitRenderIndex,
    entity: Entity,
    unit_id: crate::world::UnitId,
) {
    index.0.remove(&unit_id);
    commands.entity(entity).despawn();
}

/// Tick death presentation timers and despawn finished corpses (A3).
pub fn tick_death_presentations(
    mut commands: Commands,
    time: Res<Time>,
    control: Res<SimulationControlState>,
    mut presentations: Query<(Entity, &mut DeathPresentation)>,
) {
    for (entity, mut presentation) in &mut presentations {
        if presentation.remaining_seconds <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let delta = presentation_advance_seconds(&control, time.delta_secs());
        if delta <= 0.0 {
            continue;
        }
        presentation.remaining_seconds -= delta;
        if presentation.remaining_seconds <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::{SIMULATION_TICK_SECONDS, SimulationControlState};
    use crate::world::{AnimationProfileId, UnitDefinitionId};

    #[test]
    fn death_presentation_despawns_after_timer() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(UnitAnimationSettings::default());
        app.insert_resource(SimulationControlState::default());
        app.add_systems(Update, tick_death_presentations);

        let entity = app
            .world_mut()
            .spawn(DeathPresentation {
                definition_id: UnitDefinitionId::new("wolf"),
                profile_id: AnimationProfileId::new("humanoid"),
                remaining_seconds: 0.05,
                freeze_pose: false,
            })
            .id();

        app.world_mut()
            .entity_mut(entity)
            .get_mut::<DeathPresentation>()
            .unwrap()
            .remaining_seconds = 0.0;
        app.update();
        assert!(app.world().get_entity(entity).is_err());
    }

    #[test]
    fn death_timer_freezes_while_paused() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(SimulationControlState {
            paused: true,
            step_once: false,
            current_tick: 0,
        });
        app.add_systems(Update, tick_death_presentations);

        let entity = app
            .world_mut()
            .spawn(DeathPresentation {
                definition_id: UnitDefinitionId::new("wolf"),
                profile_id: AnimationProfileId::new("humanoid"),
                remaining_seconds: 2.0,
                freeze_pose: false,
            })
            .id();

        app.update();
        assert_eq!(
            app.world()
                .entity(entity)
                .get::<DeathPresentation>()
                .unwrap()
                .remaining_seconds,
            2.0
        );
    }

    #[test]
    fn death_timer_step_once_advances_one_tick() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(SimulationControlState {
            paused: true,
            step_once: true,
            current_tick: 0,
        });
        app.add_systems(Update, tick_death_presentations);

        let entity = app
            .world_mut()
            .spawn(DeathPresentation {
                definition_id: UnitDefinitionId::new("wolf"),
                profile_id: AnimationProfileId::new("humanoid"),
                remaining_seconds: 2.0,
                freeze_pose: false,
            })
            .id();

        app.update();
        let remaining = app
            .world()
            .entity(entity)
            .get::<DeathPresentation>()
            .unwrap()
            .remaining_seconds;
        assert!((remaining - (2.0 - SIMULATION_TICK_SECONDS)).abs() < 1e-5);
    }
}
