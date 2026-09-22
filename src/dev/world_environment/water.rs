//! World window water controls (WATER-UI-1 / WATER-UI-1F).

use bevy::ecs::system::ParamSet;
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use crate::dev::dev_mode::DevModeState;
use crate::dev::widgets::{
    DevSliderDragState, DevWidgetSliderTrack, DevWidgetSliderValue, DevWidgetToggle,
    DevWidgetToggleMark, format_numeric_display, normalized_to_value, slider_normalized_x,
    sync_slider_fill, toggle_button_bg, value_to_normalized,
};
use crate::dev::window::{DevWindowId, DevWindowRegistry};
use crate::environment::WaterSettings;
use crate::terrain::TerrainRenderAssets;
use crate::world::WorldData;

use super::state::DevWorldWaterEnabledToggle;

pub const WORLD_WATER_LEVEL_FIELD_ID: u32 = 800;
pub const WATER_LEVEL_MIN: f32 = -100.0;
pub const WATER_LEVEL_MAX: f32 = 100.0;
pub const WATER_LEVEL_PRECISION: usize = 1;

/// Clamp and snap slider output to 0.1 m increments.
pub fn snap_world_water_level(value: f32) -> f32 {
    let clamped = value.clamp(WATER_LEVEL_MIN, WATER_LEVEL_MAX);
    (clamped * 10.0).round() / 10.0
}

pub fn sync_world_water_level_slider(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    settings: Res<WaterSettings>,
    mut tracks: Query<(&DevWidgetSliderTrack, &Children)>,
    mut fills: Query<&mut Node, Without<DevWidgetSliderTrack>>,
    mut values: Query<(&DevWidgetSliderValue, &mut Text)>,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::World) {
        return;
    }

    let level = settings.water_level;
    let display = format_numeric_display(level, WATER_LEVEL_PRECISION);
    sync_slider_fill(
        WORLD_WATER_LEVEL_FIELD_ID,
        value_to_normalized(level, WATER_LEVEL_MIN, WATER_LEVEL_MAX),
        &display,
        &mut tracks,
        &mut fills,
        &mut values,
    );
}

pub fn apply_dev_presentation_water_level(
    world: &mut WorldData,
    settings: &mut WaterSettings,
    presentation_y: f32,
    vertical_scale: f32,
) {
    let snapped = snap_world_water_level(presentation_y);
    world
        .water_mut()
        .set_surface_from_presentation(snapped, vertical_scale);
    settings.water_level = world.water().presentation_surface_y(vertical_scale);
}

fn current_vertical_scale(render: Option<&TerrainRenderAssets>) -> f32 {
    render
        .map(|assets| assets.vertical_scale)
        .filter(|scale| scale.is_finite() && scale.abs() > 1e-8)
        .unwrap_or(1.0)
}

fn write_dev_water_level(
    world: Option<&mut WorldData>,
    settings: &mut WaterSettings,
    presentation_y: f32,
    render: Option<&TerrainRenderAssets>,
) {
    let snapped = snap_world_water_level(presentation_y);
    if let Some(world) = world {
        apply_dev_presentation_water_level(
            world,
            settings,
            snapped,
            current_vertical_scale(render),
        );
    } else {
        settings.water_level = snapped;
    }
}

pub fn handle_world_water_level_slider(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    mut world: Option<ResMut<WorldData>>,
    render: Option<Res<TerrainRenderAssets>>,
    mut settings: ResMut<WaterSettings>,
    mut drag: ResMut<DevSliderDragState>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut gate: ResMut<crate::dev::DevModeInputGate>,
    mut interactions: ParamSet<(
        Query<
            (&Interaction, &DevWidgetSliderValue),
            (Changed<Interaction>, Without<DevWidgetSliderTrack>),
        >,
        Query<
            (&DevWidgetSliderTrack, &Interaction, &RelativeCursorPosition),
            Without<DevWidgetSliderValue>,
        >,
    )>,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::World) {
        if drag.field_id == Some(WORLD_WATER_LEVEL_FIELD_ID) {
            drag.field_id = None;
        }
        return;
    }

    for (interaction, value_btn) in interactions.p0().iter() {
        if value_btn.id != WORLD_WATER_LEVEL_FIELD_ID {
            continue;
        }
        if *interaction == Interaction::Pressed {
            gate.block_gameplay_mouse = true;
        }
    }

    if mouse.just_pressed(MouseButton::Left) {
        for (track, interaction, relative) in interactions.p1().iter() {
            if track.id != WORLD_WATER_LEVEL_FIELD_ID {
                continue;
            }
            if *interaction != Interaction::Pressed {
                continue;
            }
            drag.field_id = Some(track.id);
            gate.block_gameplay_mouse = true;
            gate.block_camera_input = true;
            if let Some(norm) = slider_normalized_x(relative) {
                write_dev_water_level(
                    world.as_deref_mut(),
                    settings.as_mut(),
                    normalized_to_value(norm, WATER_LEVEL_MIN, WATER_LEVEL_MAX),
                    render.as_deref(),
                );
            }
        }
    }

    if mouse.just_released(MouseButton::Left) {
        drag.field_id = None;
    }

    let Some(field_id) = drag.field_id else {
        return;
    };
    if field_id != WORLD_WATER_LEVEL_FIELD_ID {
        return;
    }

    for (track, _interaction, relative) in interactions.p1().iter() {
        if track.id != field_id {
            continue;
        }
        let Some(norm) = slider_normalized_x(relative) else {
            continue;
        };
        write_dev_water_level(
            world.as_deref_mut(),
            settings.as_mut(),
            normalized_to_value(norm, WATER_LEVEL_MIN, WATER_LEVEL_MAX),
            render.as_deref(),
        );
        gate.block_camera_input = true;
        gate.block_gameplay_mouse = true;
    }
}

pub fn sync_world_water_enabled_toggle(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    settings: Res<WaterSettings>,
    mut toggles: Query<
        (
            &DevWorldWaterEnabledToggle,
            &DevWidgetToggle,
            &mut BackgroundColor,
            &Children,
        ),
        Without<crate::dev::world_environment::state::DevWorldCycleToggle>,
    >,
    mut marks: Query<&mut Visibility, With<DevWidgetToggleMark>>,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::World) {
        return;
    }
    for (_marker, toggle, mut bg, children) in toggles.iter_mut() {
        let on = settings.enabled;
        *bg = toggle_button_bg(&Interaction::None, on, toggle.disabled);
        sync_water_toggle_mark(children, on && !toggle.disabled, &mut marks);
    }
}

fn sync_water_toggle_mark(
    children: &Children,
    show: bool,
    marks: &mut Query<&mut Visibility, With<DevWidgetToggleMark>>,
) {
    for child in children.iter() {
        if let Ok(mut visibility) = marks.get_mut(child) {
            *visibility = if show {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
}

pub fn handle_world_water_enabled_toggle(
    dev_state: Res<DevModeState>,
    registry: Res<DevWindowRegistry>,
    mut world: Option<ResMut<WorldData>>,
    mut gate: ResMut<crate::dev::DevModeInputGate>,
    mut settings: ResMut<WaterSettings>,
    buttons: Query<
        (&Interaction, &DevWorldWaterEnabledToggle),
        (Changed<Interaction>, With<Button>),
    >,
) {
    if !registry.window_active(dev_state.enabled, DevWindowId::World) {
        return;
    }
    for (interaction, _marker) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        gate.block_gameplay_mouse = true;
        let enabled = if let Some(world) = world.as_deref_mut() {
            let next = !world.water().enabled;
            world.water_mut().set_enabled(next);
            next
        } else {
            !settings.enabled
        };
        settings.enabled = enabled;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn water_level_slider_range_is_valid() {
        assert!(WATER_LEVEL_MAX > WATER_LEVEL_MIN);
        assert!(WATER_LEVEL_MIN <= WaterSettings::default().water_level);
        assert!(WATER_LEVEL_MAX >= WaterSettings::default().water_level);
    }

    #[test]
    fn snap_world_water_level_clamps_and_quantizes() {
        assert!((snap_world_water_level(17.34) - 17.3).abs() < f32::EPSILON);
        assert!((snap_world_water_level(-150.0) - WATER_LEVEL_MIN).abs() < f32::EPSILON);
        assert!((snap_world_water_level(150.0) - WATER_LEVEL_MAX).abs() < f32::EPSILON);
    }

    #[test]
    fn slider_mutates_authoritative_water_settings() {
        let mut settings = WaterSettings::default();
        let mut world = WorldData::new(crate::world::ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        });
        apply_dev_presentation_water_level(&mut world, &mut settings, 23.7, 1.0);
        assert!((settings.water_level - 23.7).abs() < f32::EPSILON);
        assert!((world.water().surface_y_sim - 23.7).abs() < 1e-4);
    }

    #[test]
    fn water_enabled_toggle_mutates_authoritative_settings() {
        let mut settings = WaterSettings::default();
        settings.enabled = false;
        assert!(!settings.enabled);
        settings.enabled = true;
        assert!(settings.enabled);
    }

    #[test]
    fn world_water_ui_spawn_contract() {
        use super::super::state::{DevWorldWaterEnabledToggle, DevWorldWaterSection};

        assert_eq!(WORLD_WATER_LEVEL_FIELD_ID, 800);
        assert!(
            std::any::type_name::<DevWorldWaterSection>().contains("DevWorldWaterSection"),
            "water section marker must exist"
        );
        assert!(
            std::any::type_name::<DevWorldWaterEnabledToggle>()
                .contains("DevWorldWaterEnabledToggle"),
            "water enabled toggle marker must exist"
        );
    }

    #[test]
    fn world_water_section_defaults_expanded() {
        use crate::dev::widgets::DevCollapsibleSectionId;

        assert!(DevCollapsibleSectionId::WorldWater.default_expanded());
    }
}
