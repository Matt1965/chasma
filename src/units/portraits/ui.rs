//! Bind cached portrait textures into gameplay HUD widgets.

use bevy::prelude::*;
use bevy::ui::widget::{ImageNode, NodeImageMode};

use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::ui::gameplay::{
    SelectedUnitPortraitFallback, SquadEntryPortraitFallback, SquadEntryPortraitImage,
    primary_selected_unit,
};
use crate::units::input::SelectedUnits;

use super::cache::UnitPortraitCache;
use super::diagnostics::{
    PortraitDiagnosticTrace, portrait_diagnostic_step, portrait_diagnostics_show_without_selection,
};
use super::lifecycle_probe::{PortraitLifecycleProbe, lifecycle_probe_hud_image};
use super::pipeline::{UnitPortraitCaptureState, UnitPortraitUiDemand};
use super::studio_images::UnitPortraitStudioImages;

/// Portrait image node in the selected-unit HUD plate.
#[derive(Component, Debug)]
pub struct SelectedUnitPortraitImage;

/// HUD portrait wells use render-target handles; stretch to the inset instead of intrinsic 256px layout.
pub fn portrait_plate_image_node(handle: Handle<Image>) -> ImageNode {
    ImageNode {
        image: handle,
        color: Color::WHITE,
        image_mode: NodeImageMode::Stretch,
        ..default()
    }
}

/// Square inset that preserves 1:1 portrait RTT aspect inside a non-square HUD well.
pub fn portrait_square_inset_layout() -> Node {
    Node {
        width: Val::Percent(100.0),
        max_width: Val::Percent(100.0),
        aspect_ratio: Some(1.0),
        align_self: AlignSelf::Center,
        position_type: PositionType::Relative,
        overflow: Overflow::clip(),
        flex_shrink: 0.0,
        ..default()
    }
}

pub fn portrait_plate_image_node_layout() -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        top: Val::Px(0.0),
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        overflow: Overflow::clip(),
        ..default()
    }
}

pub(crate) fn portrait_image_for_unit(
    step: u8,
    lifecycle: &PortraitLifecycleProbe,
    studio: &UnitPortraitStudioImages,
    cache: &UnitPortraitCache,
    capture: &UnitPortraitCaptureState,
    unit_id: Option<crate::world::UnitId>,
) -> Option<Handle<Image>> {
    if let Some(handle) = lifecycle_probe_hud_image(lifecycle, cache, unit_id) {
        return Some(handle);
    }
    match step {
        1 => Some(studio.checkerboard.clone()),
        2..=6 => Some(studio.live_target.clone()),
        _ => unit_id.and_then(|unit_id| {
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
    lifecycle: Res<PortraitLifecycleProbe>,
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
    let cached_image = portrait_image_for_unit(
        step,
        &lifecycle,
        &studio,
        &cache,
        &capture,
        primary,
    );
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
            *image = portrait_plate_image_node(handle);
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

/// Bind cached portraits onto squad roster card plates.
pub fn sync_squad_portrait_ui(
    world_selection: Res<WorldSelectionState>,
    cache: Res<UnitPortraitCache>,
    capture: Res<UnitPortraitCaptureState>,
    lifecycle: Res<PortraitLifecycleProbe>,
    studio: Res<UnitPortraitStudioImages>,
    mut widgets: ParamSet<(
        Query<(&SquadEntryPortraitImage, &mut ImageNode, &mut Visibility)>,
        Query<(&SquadEntryPortraitFallback, &mut Visibility)>,
    )>,
) {
    if world_selection.category != WorldSelectionCategory::Units {
        for (_, _, mut visibility) in widgets.p0().iter_mut() {
            *visibility = Visibility::Hidden;
        }
        for (_, mut visibility) in widgets.p1().iter_mut() {
            *visibility = Visibility::Visible;
        }
        return;
    }

    let step = portrait_diagnostic_step();
    for (slot, mut image, mut image_visibility) in widgets.p0().iter_mut() {
        let handle = portrait_image_for_unit(
            step,
            &lifecycle,
            &studio,
            &cache,
            &capture,
            Some(slot.unit_id),
        );
        if let Some(handle) = handle {
            *image = portrait_plate_image_node(handle);
            *image_visibility = Visibility::Visible;
        } else {
            *image_visibility = Visibility::Hidden;
        }
    }

    for (slot, mut fallback_visibility) in widgets.p1().iter_mut() {
        let show_letter = portrait_image_for_unit(
            step,
            &lifecycle,
            &studio,
            &cache,
            &capture,
            Some(slot.unit_id),
        )
        .is_none();
        *fallback_visibility = if show_letter {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

