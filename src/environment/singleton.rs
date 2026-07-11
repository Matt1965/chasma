//! Environment singleton resolution (REVIEW-B5, ADR-068).

use bevy::prelude::*;

use super::lighting::EnvironmentDirectionalLight;

/// Result of resolving the environment-owned directional light.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvironmentDirectionalLightResolution {
    /// Exactly one environment directional light — safe to update.
    Single,
    /// No environment directional light entity exists.
    Missing,
    /// More than one environment directional light entity exists.
    Duplicate { count: usize },
}

/// Resolve how many environment directional lights are present.
pub fn resolve_environment_directional_light(
    lights: &Query<(), With<EnvironmentDirectionalLight>>,
) -> EnvironmentDirectionalLightResolution {
    let count = lights.iter().count();
    match count {
        0 => EnvironmentDirectionalLightResolution::Missing,
        1 => EnvironmentDirectionalLightResolution::Single,
        n => EnvironmentDirectionalLightResolution::Duplicate { count: n },
    }
}

/// Apply a mutation only when exactly one environment directional light exists.
pub fn update_environment_directional_light(
    resolution: EnvironmentDirectionalLightResolution,
    mut lights: Query<(&mut DirectionalLight, &mut Transform), With<EnvironmentDirectionalLight>>,
    mut apply: impl FnMut(&mut DirectionalLight, &mut Transform),
) -> EnvironmentDirectionalLightResolution {
    if !matches!(resolution, EnvironmentDirectionalLightResolution::Single) {
        return resolution;
    }
    if let Ok((mut light, mut transform)) = lights.single_mut() {
        apply(&mut light, &mut transform);
    }
    resolution
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::app::App;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn zero_environment_lights_reports_missing() {
        let mut app = App::new();
        let resolution = app
            .world_mut()
            .run_system_once(|lights: Query<(), With<EnvironmentDirectionalLight>>| {
                resolve_environment_directional_light(&lights)
            })
            .unwrap();
        assert_eq!(resolution, EnvironmentDirectionalLightResolution::Missing);
    }

    #[test]
    fn duplicate_environment_lights_reports_duplicate() {
        let mut app = App::new();
        app.world_mut().spawn(EnvironmentDirectionalLight);
        app.world_mut().spawn(EnvironmentDirectionalLight);
        let resolution = app
            .world_mut()
            .run_system_once(|lights: Query<(), With<EnvironmentDirectionalLight>>| {
                resolve_environment_directional_light(&lights)
            })
            .unwrap();
        assert_eq!(
            resolution,
            EnvironmentDirectionalLightResolution::Duplicate { count: 2 }
        );
    }

    #[test]
    fn update_skips_when_not_single() {
        let mut app = App::new();
        app.world_mut().spawn((
            DirectionalLight {
                illuminance: 1.0,
                ..default()
            },
            Transform::default(),
            EnvironmentDirectionalLight,
        ));
        app.world_mut().spawn((
            DirectionalLight {
                illuminance: 2.0,
                ..default()
            },
            Transform::default(),
            EnvironmentDirectionalLight,
        ));
        let resolution = app
            .world_mut()
            .run_system_once(|lights: Query<(), With<EnvironmentDirectionalLight>>| {
                resolve_environment_directional_light(&lights)
            })
            .unwrap();
        app.world_mut()
            .run_system_once(
                move |lights: Query<
                    (&mut DirectionalLight, &mut Transform),
                    With<EnvironmentDirectionalLight>,
                >| {
                    update_environment_directional_light(resolution, lights, |light, _| {
                        light.illuminance = 99_999.0;
                    });
                },
            )
            .unwrap();
        let illuminances: Vec<f32> = app
            .world_mut()
            .query::<&DirectionalLight>()
            .iter(app.world_mut())
            .map(|light| light.illuminance)
            .collect();
        assert_eq!(illuminances, vec![1.0, 2.0]);
    }
}
