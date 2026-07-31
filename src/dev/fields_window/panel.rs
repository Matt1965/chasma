//! Fields window panel — terrain field build, probe, and overlays (Slice 8).

use bevy::prelude::*;

use crate::dev::input::DevPanelUi;
use crate::dev::tooltip::DevTooltipContent;
use crate::dev::widgets::{DevCollapsibleSectionId, spawn_collapsible_section};
use crate::dev::window::{DevWindowBody, DevWindowId, DevWindowUi};

#[derive(Component, Debug)]
pub(crate) struct DevFieldsWindowUi;

pub fn setup_fields_window_panel(mut commands: Commands, bodies: Query<(Entity, &DevWindowBody)>) {
    for (entity, body) in &bodies {
        if body.id != DevWindowId::Fields {
            continue;
        }
        commands.entity(entity).with_children(|panel| {
            panel
                .spawn((
                    DevFieldsWindowUi,
                    DevPanelUi,
                    DevWindowUi,
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        ..default()
                    },
                ))
                .with_children(|root| {
                    spawn_collapsible_section(
                        root,
                        DevCollapsibleSectionId::FieldsBuild,
                        "Build and validate",
                        Some(DevTooltipContent::new(
                            "Build terrain field packages from authored sources, validate \
                             manifests, and reload catalogs. Build-all can be expensive on large \
                             worlds — results appear in the status line below.",
                        )),
                        |body| {
                            crate::dev::terrain_field::spawn_terrain_field_section(body);
                        },
                    );
                });
        });
        return;
    }
}
