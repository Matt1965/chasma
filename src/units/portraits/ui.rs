//! Bind cached portrait textures into gameplay HUD widgets.

use bevy::prelude::*;
use bevy::ui::widget::ImageNode;

use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::ui::gameplay::{SelectedUnitPortraitFallback, primary_selected_unit};
use crate::units::input::SelectedUnits;

use super::cache::UnitPortraitCache;
use super::diagnostics::{
    PortraitDiagnosticTrace, portrait_diagnostic_step, portrait_diagnostics_show_without_selection,
};
use super::pipeline::{UnitPortraitCaptureState, UnitPortraitUiDemand};
use super::studio_images::UnitPortraitStudioImages;

/// Portrait image node in the selected-unit HUD plate.
#[derive(Component, Debug)]
pub struct SelectedUnitPortraitImage;

fn portrait_slot_image(
    step: u8,
    studio: &UnitPortraitStudioImages,
    cache: &UnitPortraitCache,
    capture: &UnitPortraitCaptureState,
    primary: Option<crate::world::UnitId>,
) -> Option<Handle<Image>> {
    match step {
        1 => Some(studio.checkerboard.clone()),
        2..=6 => Some(studio.live_target.clone()),
        _ => primary.and_then(|unit_id| {
            if let Some(handle) = cache.image_for_unit(unit_id) {
                return Some(handle.clone());
            }
            capture
                .active_request
                .filter(|request| request.unit_id == unit_id)
                .filter(|_| capture.camera_aligned)
                .and_then(|_| capture.target_image.clone())
        }),
    }
}

/// Sync UI demand and portrait plate presentation.
pub fn sync_selected_unit_portrait_ui(
    world_selection: Res<WorldSelectionState>,
    selection: Res<SelectedUnits>,
    cache: Res<UnitPortraitCache>,
    capture: Res<UnitPortraitCaptureState>,
    studio: Res<UnitPortraitStudioImages>,
    mut demand: ResMut<UnitPortraitUiDemand>,
    mut trace: ResMut<PortraitDiagnosticTrace>,
    mut widgets: ParamSet<(
        Query<(&mut ImageNode, &mut Visibility), With<SelectedUnitPortraitImage>>,
        Query<&mut Visibility, With<SelectedUnitPortraitFallback>>,
    )>,
) {
    let primary = match world_selection.category {
        WorldSelectionCategory::Units => primary_selected_unit(&selection),
        _ => None,
    };
    demand.primary_unit = primary;

    let step = portrait_diagnostic_step();
    let cached_image = portrait_slot_image(step, &studio, &cache, &capture, primary);
    let show_portrait = cached_image.is_some()
        && (primary.is_some() || portrait_diagnostics_show_without_selection(step));

    trace.ui_image_id = cached_image.as_ref().map(|handle| handle.id());

    {
        let mut image_query = widgets.p0();
        let Ok((mut image, mut image_visibility)) = image_query.single_mut() else {
            return;
        };
        if show_portrait {
            let handle = cached_image.unwrap();
            *image = ImageNode {
                image: handle,
                color: Color::WHITE,
                ..default()
            };
            *image_visibility = Visibility::Visible;
        } else {
            *image_visibility = Visibility::Hidden;
        }
    }
    {
        let mut fallback_query = widgets.p1();
        let Ok(mut fallback_visibility) = fallback_query.single_mut() else {
            return;
        };
        *fallback_visibility = if show_portrait {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }
}
