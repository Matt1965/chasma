//! Presentation-only hit reaction detection (A3).

use bevy::prelude::*;

use crate::units::components::UnitRenderEntity;
use crate::world::{UnitId, UnitState, WorldData};

use super::UnitAnimationStateIndex;
use super::components::{DeathPresentation, HitReactionActive, HitReactionRequested};
use super::lod::{AnimationLod, AnimationLodSettings};
use super::presentation_time::presentation_advance_seconds;
use super::settings::UnitAnimationSettings;
use super::sync_timing::is_attack_animation_phase;
use crate::simulation::SimulationControlState;

/// Last observed HP per unit for presentation hit reactions (A3).
#[derive(Resource, Default, Debug)]
pub struct UnitHpPresentationCache {
    pub last_hp: std::collections::HashMap<UnitId, u32>,
}

/// Detect HP drops and request lightweight hit reactions (A3).
pub fn detect_unit_hit_reactions(
    world: Res<WorldData>,
    settings: Res<UnitAnimationSettings>,
    lod_settings: Res<AnimationLodSettings>,
    state_index: Res<UnitAnimationStateIndex>,
    mut hp_cache: ResMut<UnitHpPresentationCache>,
    mut commands: Commands,
    units: Query<
        (Entity, &UnitRenderEntity),
        (
            Without<DeathPresentation>,
            Without<HitReactionRequested>,
            Without<HitReactionActive>,
        ),
    >,
) {
    if !settings.enabled {
        return;
    }

    let live_ids: std::collections::HashSet<UnitId> = world.sorted_unit_ids().into_iter().collect();
    hp_cache.last_hp.retain(|id, _| live_ids.contains(id));

    for (entity, marker) in &units {
        let Some(record) = world.get_unit(marker.unit_id) else {
            continue;
        };
        if matches!(record.state, UnitState::Dead) {
            continue;
        }

        let current_hp = record.vitals.current_hp;
        let previous = hp_cache.last_hp.insert(marker.unit_id, current_hp);

        let Some(previous_hp) = previous else {
            continue;
        };
        if current_hp >= previous_hp {
            continue;
        }

        let attacking = record
            .attack_cycle
            .as_ref()
            .is_some_and(|cycle| is_attack_animation_phase(cycle.phase));
        if attacking {
            continue;
        }

        if lod_settings.enabled {
            if let Some(state) = state_index.states.get(&marker.unit_id) {
                if state.lod.lod == AnimationLod::Frozen {
                    continue;
                }
            }
        }

        commands.entity(entity).insert(HitReactionRequested);
    }
}

/// Tick active hit-reaction timers (A3).
pub fn tick_hit_reactions(
    mut commands: Commands,
    time: Res<Time>,
    control: Res<SimulationControlState>,
    mut active: Query<(Entity, &mut HitReactionActive)>,
) {
    for (entity, mut reaction) in &mut active {
        if reaction.remaining_seconds <= 0.0 {
            commands.entity(entity).remove::<HitReactionActive>();
            continue;
        }
        let delta = presentation_advance_seconds(&control, time.delta_secs());
        if delta <= 0.0 {
            continue;
        }
        reaction.remaining_seconds -= delta;
        if reaction.remaining_seconds <= 0.0 {
            commands.entity(entity).remove::<HitReactionActive>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::{SIMULATION_TICK_SECONDS, SimulationControlState};

    #[test]
    fn hp_cache_detects_drop() {
        let mut cache = UnitHpPresentationCache::default();
        let id = UnitId::new(1);
        cache.last_hp.insert(id, 10);
        let previous = cache.last_hp.insert(id, 7);
        assert_eq!(previous, Some(10));
        assert!(previous.unwrap() > 7);
    }

    #[test]
    fn hit_timer_freezes_while_paused() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(SimulationControlState {
            paused: true,
            step_once: false,
            current_tick: 0,
        });
        app.add_systems(Update, tick_hit_reactions);

        let entity = app
            .world_mut()
            .spawn(HitReactionActive {
                remaining_seconds: 1.0,
            })
            .id();
        app.update();
        assert_eq!(
            app.world()
                .entity(entity)
                .get::<HitReactionActive>()
                .unwrap()
                .remaining_seconds,
            1.0
        );
    }

    #[test]
    fn hit_timer_step_once_advances_one_tick() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(SimulationControlState {
            paused: true,
            step_once: true,
            current_tick: 0,
        });
        app.add_systems(Update, tick_hit_reactions);

        let entity = app
            .world_mut()
            .spawn(HitReactionActive {
                remaining_seconds: 1.0,
            })
            .id();
        app.update();
        let remaining = app
            .world()
            .entity(entity)
            .get::<HitReactionActive>()
            .unwrap()
            .remaining_seconds;
        assert!((remaining - (1.0 - SIMULATION_TICK_SECONDS)).abs() < 1e-5);
    }
}
