//! Profile-driven appearance controls for the Unit Editor (CG3).

use bevy::ecs::system::ParamSet;
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use crate::world::{AppearanceParamId, AppearanceProfile, AppearanceProfileCatalog, UnitAppearance};

use super::screen::{
    UnitEditorControlsHost, UnitEditorSliderBinding, UnitEditorSliderTrack, UnitEditorSliderValue,
};
use super::session::UnitEditorSession;

const HEIGHT_FIELD_ID: u32 = 1;

/// Ordered control descriptors derived from profile metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct AppearanceControlSpec {
    pub field_id: u32,
    pub param_id: AppearanceParamId,
    pub display_name: String,
    pub category: String,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub display_order: u32,
}

/// Build continuous parameter controls from an appearance profile.
pub fn appearance_control_specs(profile: &AppearanceProfile) -> Vec<AppearanceControlSpec> {
    let mut specs = profile
        .enabled_parameters()
        .into_iter()
        .enumerate()
        .map(|(index, parameter)| AppearanceControlSpec {
            field_id: index as u32 + 2,
            param_id: parameter.id.clone(),
            display_name: parameter.display_name.clone(),
            category: parameter.category.clone(),
            min: parameter.min,
            max: parameter.max,
            default: parameter.default,
            display_order: parameter.display_order,
        })
        .collect::<Vec<_>>();
    specs.sort_by(|left, right| {
        left.display_order
            .cmp(&right.display_order)
            .then_with(|| left.display_name.cmp(&right.display_name))
    });
    specs
}

pub fn clamp_appearance_value(profile: &AppearanceProfile, param_id: &AppearanceParamId, value: f32) -> f32 {
    let Some(parameter) = profile.parameter(param_id) else {
        return value;
    };
    value.clamp(parameter.min, parameter.max)
}

pub fn set_semantic_parameter(
    appearance: &mut UnitAppearance,
    profile: &AppearanceProfile,
    param_id: &AppearanceParamId,
    value: f32,
) {
    let clamped = clamp_appearance_value(profile, param_id, value);
    appearance.morphs.insert(param_id.clone(), clamped);
}

pub fn set_height_scale(appearance: &mut UnitAppearance, profile: &AppearanceProfile, value: f32) {
    appearance.height_scale = value.clamp(profile.height_scale_min, profile.height_scale_max);
}

#[derive(Resource, Debug, Default)]
pub struct UnitEditorSliderDragState {
    pub field_id: Option<u32>,
}

pub fn handle_unit_editor_sliders(
    session: Option<ResMut<UnitEditorSession>>,
    profiles: Res<AppearanceProfileCatalog>,
    mut drag: ResMut<UnitEditorSliderDragState>,
    mouse: Res<ButtonInput<MouseButton>>,
    bindings: Query<&UnitEditorSliderBinding>,
    mut interactions: ParamSet<(
        Query<
            (&UnitEditorSliderTrack, &Interaction, &RelativeCursorPosition),
            Without<UnitEditorSliderValue>,
        >,
    )>,
    roots: Query<(), With<UnitEditorControlsHost>>,
) {
    if roots.is_empty() {
        drag.field_id = None;
        return;
    }
    let Some(mut session) = session else {
        drag.field_id = None;
        return;
    };
    let profile_id = session.draft.appearance.profile_id.clone();
    let Some(profile) = profiles.get(&profile_id) else {
        session.error_message = Some(format!("unknown appearance profile `{}`", profile_id.as_str()));
        return;
    };

    if mouse.just_pressed(MouseButton::Left) {
        for (track, interaction, relative) in interactions.p0().iter() {
            if *interaction != Interaction::Pressed {
                continue;
            }
            drag.field_id = Some(track.field_id);
            apply_slider_drag(track.field_id, relative, &bindings, profile, &mut session);
        }
    }
    if mouse.just_released(MouseButton::Left) {
        drag.field_id = None;
    }
    let Some(field_id) = drag.field_id else {
        return;
    };
    for (track, _interaction, relative) in interactions.p0().iter() {
        if track.field_id != field_id {
            continue;
        }
        apply_slider_drag(field_id, relative, &bindings, profile, &mut session);
    }
}

fn apply_slider_drag(
    field_id: u32,
    relative: &RelativeCursorPosition,
    bindings: &Query<&UnitEditorSliderBinding>,
    profile: &AppearanceProfile,
    session: &mut UnitEditorSession,
) {
    let Some(norm) = slider_normalized_x(relative) else {
        return;
    };
    let binding = bindings.iter().find(|value| value.field_id == field_id);
    let Some(binding) = binding else {
        return;
    };
    if binding.is_height {
        let value = normalized_to_value(norm, profile.height_scale_min, profile.height_scale_max);
        set_height_scale(&mut session.draft.appearance, profile, value);
    } else if let Some(param_id) = &binding.param_id {
        let parameter = profile.parameter(param_id);
        let min = parameter.map(|p| p.min).unwrap_or(0.0);
        let max = parameter.map(|p| p.max).unwrap_or(1.0);
        let value = normalized_to_value(norm, min, max);
        set_semantic_parameter(&mut session.draft.appearance, profile, param_id, value);
    }
    session.recompute_dirty();
}

pub fn sync_unit_editor_control_values(
    session: Option<Res<UnitEditorSession>>,
    profiles: Res<AppearanceProfileCatalog>,
    bindings: Query<&UnitEditorSliderBinding>,
    mut tracks: Query<(&UnitEditorSliderTrack, &Children)>,
    mut fills: Query<&mut Node, Without<UnitEditorSliderTrack>>,
    mut values: Query<(&UnitEditorSliderValue, &mut Text)>,
    roots: Query<(), With<UnitEditorControlsHost>>,
) {
    if roots.is_empty() {
        return;
    }
    let Some(session) = session else {
        return;
    };
    let profile_id = session.draft.appearance.profile_id.clone();
    let Some(profile) = profiles.get(&profile_id) else {
        return;
    };

    sync_slider_fill(
        HEIGHT_FIELD_ID,
        value_to_normalized(
            session.draft.appearance.height_scale,
            profile.height_scale_min,
            profile.height_scale_max,
        ),
        &format!("{:.2}", session.draft.appearance.height_scale),
        &mut tracks,
        &mut fills,
        &mut values,
    );

    for binding in &bindings {
        if binding.is_height {
            continue;
        }
        let Some(param_id) = &binding.param_id else {
            continue;
        };
        let parameter = profile.parameter(param_id);
        let min = parameter.map(|p| p.min).unwrap_or(0.0);
        let max = parameter.map(|p| p.max).unwrap_or(1.0);
        let value = session
            .draft
            .appearance
            .semantic_value(param_id)
            .unwrap_or_else(|| parameter.map(|p| p.default).unwrap_or(0.0));
        sync_slider_fill(
            binding.field_id,
            value_to_normalized(value, min, max),
            &format!("{:.2}", value),
            &mut tracks,
            &mut fills,
            &mut values,
        );
    }
}

fn slider_normalized_x(relative: &RelativeCursorPosition) -> Option<f32> {
    relative
        .normalized
        .map(|position| (position.x + 0.5).clamp(0.0, 1.0))
}

fn value_to_normalized(value: f32, min: f32, max: f32) -> f32 {
    if (max - min).abs() < f32::EPSILON {
        return 0.0;
    }
    ((value - min) / (max - min)).clamp(0.0, 1.0)
}

fn normalized_to_value(normalized: f32, min: f32, max: f32) -> f32 {
    min + normalized.clamp(0.0, 1.0) * (max - min)
}

fn sync_slider_fill(
    field_id: u32,
    normalized: f32,
    display: &str,
    tracks: &mut Query<(&UnitEditorSliderTrack, &Children)>,
    fills: &mut Query<&mut Node, Without<UnitEditorSliderTrack>>,
    values: &mut Query<(&UnitEditorSliderValue, &mut Text)>,
) {
    let t = normalized.clamp(0.0, 1.0);
    for (track, children) in tracks.iter() {
        if track.field_id != field_id {
            continue;
        }
        if let Some(&child) = children.first() {
            if let Ok(mut node) = fills.get_mut(child) {
                node.width = Val::Percent(t * 100.0);
            }
        }
    }
    for (value, mut text) in values.iter_mut() {
        if value.field_id == field_id {
            **text = display.to_string();
        }
    }
}

pub fn height_field_id() -> u32 {
    HEIGHT_FIELD_ID
}
