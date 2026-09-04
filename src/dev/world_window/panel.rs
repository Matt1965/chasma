//! World window panel — environment authoring, harness status (Slice 8 / 11).

use bevy::ecs::system::ParamSet;
use bevy::prelude::*;

use crate::dev::dev_mode::DevModeState;
use crate::dev::input::DevPanelUi;
use crate::dev::tooltip::DevTooltipTarget;
use crate::dev::widgets::DevCollapsibleSectionId;
use crate::dev::window::{DevWindowBody, DevWindowId, DevWindowRegistry, DevWindowUi};
use crate::dev::world_environment::DevWorldEnvironmentSection;

#[derive(Component, Debug)]
pub(crate) struct DevWorldWindowUi;

#[derive(Component, Debug)]
pub(crate) struct DevWorldHarnessText;

/// Hide world-window chrome when dev mode is off or the window is closed.
pub fn sync_dev_world_panel_visibility(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    mut visibility: ParamSet<(
        Query<&mut Visibility, With<DevWorldWindowUi>>,
        Query<&mut Visibility, With<DevWorldEnvironmentSection>>,
        Query<&mut Visibility, With<crate::dev::widgets::DevWidgetBadge>>,
    )>,
) {
    let visible = dev_state.enabled && registry.is_visible(DevWindowId::World);
    for mut vis in visibility.p0().iter_mut() {
        *vis = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    for mut vis in visibility.p1().iter_mut() {
        *vis = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    for mut vis in visibility.p2().iter_mut() {
        *vis = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

pub fn setup_world_window_panel(mut commands: Commands, bodies: Query<(Entity, &DevWindowBody)>) {
    for (entity, body) in &bodies {
        if body.id != DevWindowId::World {
            continue;
        }
        commands.entity(entity).with_children(|panel| {
            panel
                .spawn((
                    DevWorldWindowUi,
                    DevPanelUi,
                    DevWindowUi,
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(8.0),
                        ..default()
                    },
                ))
                .with_children(|root| {
                    root.spawn((
                        DevPanelUi,
                        DevTooltipTarget::new(
                            "Environment and time-of-day authoring with project-default \
                             persistence. Closing this window does not stop the cycle or reset \
                             lighting.",
                        ),
                        Text::new("Environment"),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.8, 0.88, 0.95, 1.0)),
                    ));
                    crate::dev::world_environment::spawn_environment_controls(root);
                });
        });
        return;
    }
}
