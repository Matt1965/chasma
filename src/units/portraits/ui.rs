//! Bind cached portrait textures into gameplay HUD widgets.

use bevy::prelude::*;
use bevy::ui::widget::{ImageNode, NodeImageMode};

use crate::client::selection::{WorldSelectionCategory, WorldSelectionState};
use crate::ui::gameplay::{
    PlayerHudState, SelectedUnitPortraitFallback, SquadEntryPortraitFallback,
    SquadEntryPortraitImage, primary_selected_unit,
};
use crate::units::input::SelectedUnits;
use crate::world::{
    AppearanceProfileCatalog, EquipmentVisualCatalog, ItemCatalog, UnitCatalog, UnitId, WorldData,
};

use super::cache::{PortraitAppearanceSignature, UnitPortraitCache};
use super::signature::portrait_signature_for_unit;
use super::diagnostics::{
    PortraitDiagnosticTrace, portrait_diagnostic_step, portrait_diagnostics_show_without_selection,
};
use super::lifecycle_probe::{PortraitLifecycleProbe, lifecycle_probe_hud_image};
use super::pipeline::{UnitPortraitCaptureState, UnitPortraitUiDemand};
use super::studio_images::UnitPortraitStudioImages;

/// Portrait image node in the selected-unit HUD plate.
#[derive(Component, Debug)]
pub struct SelectedUnitPortraitImage;

/// Portrait textures are square render targets; stretch to the (square) well
/// rather than laying out at the intrinsic 256px texture size.
fn portrait_plate_image_node(handle: Handle<Image>) -> ImageNode {
    ImageNode {
        image: handle,
        color: Color::WHITE,
        image_mode: NodeImageMode::Stretch,
        ..default()
    }
}

/// Image layer: absolutely positioned so it always covers the well's content
/// box, independent of any sibling (including a hidden fallback glyph).
fn portrait_plate_image_layout(radius: BorderRadius) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        top: Val::Px(0.0),
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        border_radius: radius,
        ..default()
    }
}

/// Fallback overlay: out of flow, so `Visibility::Hidden` text cannot shift the image.
fn portrait_fallback_overlay_layout() -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        top: Val::Px(0.0),
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    }
}

/// Spawn the two layers every HUD portrait well uses: the cached-texture image
/// and a centred letter fallback. The caller owns the well node (square, with
/// its own rim/fill styling) and the marker components; sync systems toggle the
/// markers' `Visibility` and bind the image handle.
pub fn spawn_portrait_plate_layers(
    well: &mut ChildSpawnerCommands<'_>,
    radius: BorderRadius,
    image_marker: impl Bundle,
    fallback_marker: impl Bundle,
    fallback_text: Text,
    fallback_font: TextFont,
    fallback_color: TextColor,
) {
    well.spawn((
        image_marker,
        portrait_plate_image_node(Handle::default()),
        portrait_plate_image_layout(radius),
        Visibility::Hidden,
    ));
    well.spawn(portrait_fallback_overlay_layout())
        .with_children(|overlay| {
            overlay.spawn((fallback_marker, fallback_text, fallback_font, fallback_color));
        });
}

/// Bind a texture without rewriting the node when it is unchanged.
fn bind_portrait_image(image: &mut ImageNode, handle: Handle<Image>) {
    if image.image != handle || image.image_mode != NodeImageMode::Stretch {
        *image = portrait_plate_image_node(handle);
    }
}

pub(crate) fn portrait_image_for_unit(
    step: u8,
    lifecycle: &PortraitLifecycleProbe,
    studio: &UnitPortraitStudioImages,
    cache: &UnitPortraitCache,
    capture: &UnitPortraitCaptureState,
    unit_id: Option<UnitId>,
    signature: Option<PortraitAppearanceSignature>,
) -> Option<Handle<Image>> {
    if let Some(handle) = lifecycle_probe_hud_image(lifecycle, cache, unit_id) {
        return Some(handle);
    }
    match step {
        1 => Some(studio.checkerboard.clone()),
        2..=6 => Some(studio.live_target.clone()),
        _ => unit_id.and_then(|unit_id| {
            let signature = signature?;
            if cache.entry_matches(unit_id, &signature) {
                return cache.image_for_unit(unit_id).cloned();
            }
            capture
                .active_request
                .filter(|request| request.unit_id == unit_id && request.signature == signature)
                .filter(|_| capture.camera_aligned)
                .and_then(|_| capture.target_image.clone())
        }),
    }
}

fn portrait_signature_for_unit_id(
    world: &WorldData,
    unit_id: UnitId,
    unit_catalog: &UnitCatalog,
    appearance_profiles: &AppearanceProfileCatalog,
    items: &ItemCatalog,
    visuals: &EquipmentVisualCatalog,
) -> Option<PortraitAppearanceSignature> {
    let unit = world.get_unit(unit_id)?;
    portrait_signature_for_unit(
        world,
        unit,
        unit_catalog,
        appearance_profiles,
        items,
        visuals,
    )
}

/// Shown portrait layers use [`Visibility::Inherited`]: [`Visibility::Visible`] would
/// ignore a hidden [`crate::ui::gameplay::GameplayHudRoot`] ancestor (menu / pause).
pub fn sync_selected_unit_portrait_ui(
    hud: Res<PlayerHudState>,
    world: Res<WorldData>,
    unit_catalog: Res<UnitCatalog>,
    appearance_profiles: Res<AppearanceProfileCatalog>,
    items: Res<ItemCatalog>,
    equipment_visuals: Res<EquipmentVisualCatalog>,
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
    let signature = primary.and_then(|unit_id| {
        portrait_signature_for_unit_id(
            &world,
            unit_id,
            &unit_catalog,
            &appearance_profiles,
            &items,
            &equipment_visuals,
        )
    });
    let cached_image = portrait_image_for_unit(
        step,
        &lifecycle,
        &studio,
        &cache,
        &capture,
        primary,
        signature,
    );
    let show_portrait = hud.visible
        && cached_image.is_some()
        && (primary.is_some() || portrait_diagnostics_show_without_selection(step));

    trace.ui_image_id = cached_image.as_ref().map(|handle| handle.id());

    {
        let mut image_query = widgets.p0();
        let Ok((mut image, mut image_visibility)) = image_query.single_mut() else {
            return;
        };
        if show_portrait {
            let handle = cached_image.unwrap();
            bind_portrait_image(&mut image, handle);
            *image_visibility = Visibility::Inherited;
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
        } else if hud.visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// Bind cached portraits onto squad roster card plates.
///
/// Roster portraits describe owned units, not the current selection, so they
/// are never gated on what (if anything) is selected.
pub fn sync_squad_portrait_ui(
    hud: Res<PlayerHudState>,
    world: Res<WorldData>,
    unit_catalog: Res<UnitCatalog>,
    appearance_profiles: Res<AppearanceProfileCatalog>,
    items: Res<ItemCatalog>,
    equipment_visuals: Res<EquipmentVisualCatalog>,
    cache: Res<UnitPortraitCache>,
    capture: Res<UnitPortraitCaptureState>,
    lifecycle: Res<PortraitLifecycleProbe>,
    studio: Res<UnitPortraitStudioImages>,
    mut widgets: ParamSet<(
        Query<(&SquadEntryPortraitImage, &mut ImageNode, &mut Visibility)>,
        Query<(&SquadEntryPortraitFallback, &mut Visibility)>,
    )>,
) {
    let step = portrait_diagnostic_step();
    for (slot, mut image, mut image_visibility) in widgets.p0().iter_mut() {
        let signature = portrait_signature_for_unit_id(
            &world,
            slot.unit_id,
            &unit_catalog,
            &appearance_profiles,
            &items,
            &equipment_visuals,
        );
        let handle = portrait_image_for_unit(
            step,
            &lifecycle,
            &studio,
            &cache,
            &capture,
            Some(slot.unit_id),
            signature,
        );
        if hud.visible {
            if let Some(handle) = handle {
                bind_portrait_image(&mut image, handle);
                *image_visibility = Visibility::Inherited;
            } else {
                *image_visibility = Visibility::Hidden;
            }
        } else {
            *image_visibility = Visibility::Hidden;
        }
    }

    for (slot, mut fallback_visibility) in widgets.p1().iter_mut() {
        let show_letter = hud.visible
            && portrait_image_for_unit(
                step,
                &lifecycle,
                &studio,
                &cache,
                &capture,
                Some(slot.unit_id),
                portrait_signature_for_unit_id(
                    &world,
                    slot.unit_id,
                    &unit_catalog,
                    &appearance_profiles,
                    &items,
                    &equipment_visuals,
                ),
            )
            .is_none();
        *fallback_visibility = if show_letter {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

