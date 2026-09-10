//! Player-facing field overlay selection (ADR-103 TF3).
//!
//! Gameplay HUD routes field picks through [`TerrainOverlayState::set_manual_field`].
//! Dev auxiliary overlays and the Terrain Analysis panel are separate paths.

use crate::world::{TerrainFieldCatalog, TerrainFieldId};

use super::TerrainOverlayState;

/// Field ids exposed in the compact player Fields menu.
pub const PLAYER_FIELD_MENU_IDS: [&str; 4] = ["water", "iron", "copper", "stone"];

/// Toggle or select a player field overlay using existing overlay semantics.
///
/// Selecting the active field again clears the overlay. Opacity follows each
/// field's catalog default until the player overrides it elsewhere.
pub fn apply_player_field_overlay_selection(
    overlay_state: &mut TerrainOverlayState,
    catalog: &TerrainFieldCatalog,
    field_id: TerrainFieldId,
) {
    if !catalog
        .get(&field_id)
        .is_some_and(|def| def.enabled && def.overlay_style.enabled)
    {
        return;
    }

    if overlay_state.selection.manual.as_ref() == Some(&field_id) {
        overlay_state.set_manual_field(None);
        return;
    }

    if !overlay_state.opacity_user_override {
        if let Some(def) = catalog.get(&field_id) {
            overlay_state.opacity_basis_points = TerrainOverlayState::clamp_opacity(
                (def.overlay_style.default_opacity * 10_000.0) as u16,
            );
        }
    }
    overlay_state.set_manual_field(Some(field_id));
}
