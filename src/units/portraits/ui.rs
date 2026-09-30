//! Bind cached portrait textures into gameplay HUD widgets.

use bevy::prelude::*;
use bevy::ui::widget::ImageNode;

use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::ui::gameplay::{SelectedUnitPortraitFallback, primary_selected_unit};
use crate::units::input::SelectedUnits;

use super::cache::UnitPortraitCache;
use super::pipeline::UnitPortraitUiDemand;

/// Portrait image node in the selected-unit HUD plate.
#[derive(Component, Debug)]
pub struct SelectedUnitPortraitImage;

/// Sync UI demand and portrait plate presentation.
pub fn sync_selected_unit_portrait_ui(
    world_selection: Res<WorldSelectionState>,
    selection: Res<SelectedUnits>,
    cache: Res<UnitPortraitCache>,
    mut demand: ResMut<UnitPortraitUiDemand>,
    mut portrait_image: Query<(&mut ImageNode, &mut Visibility), With<SelectedUnitPortraitImage>>,
    mut fallback: Query<&mut Visibility, With<SelectedUnitPortraitFallback>>,
) {
    let primary = match world_selection.category {
        WorldSelectionCategory::Units => primary_selected_unit(&selection),
        _ => None,
    };
    demand.primary_unit = primary;

    let Ok((mut image, mut image_visibility)) = portrait_image.single_mut() else {
        return;
    };
    let Ok(mut fallback_visibility) = fallback.single_mut() else {
        return;
    };

    if let Some(unit_id) = primary {
        if let Some(handle) = cache.image_for_unit(unit_id) {
            *image = ImageNode::new(handle.clone());
            *image_visibility = Visibility::Visible;
            *fallback_visibility = Visibility::Hidden;
            return;
        }
    }

    *image_visibility = Visibility::Hidden;
    *fallback_visibility = Visibility::Visible;
}
