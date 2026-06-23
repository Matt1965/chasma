//! Contextual cursor mode from selection and hover (ADR-040 U-UI4).
//!
//! Drives logical cursor mode from intent-layer inputs (selection + terrain hover).
//! OS cursor icon wiring is deferred to a future Bevy window API hook.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::RtsCamera;
use crate::terrain::TerrainRenderAssets;
use crate::units::input::{
    cursor_world_ray, pick_unit_along_ray, terrain_click_to_world_position, SelectedUnits,
};
use crate::world::{UnitCatalog, WorldConfig, WorldData};

use super::state::{derive_cursor_mode, CommandHoverContext, GameplayCursorMode, GameplayUiState};

/// Last published gameplay cursor mode (for tests and future OS cursor sync).
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GameplayCursorPresentation {
    pub mode: GameplayCursorMode,
}

/// Sample hover context after intent collection; update cursor mode (read-only).
pub fn sample_gameplay_cursor_context(
    selection: Res<SelectedUnits>,
    camera: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    unit_catalog: Res<UnitCatalog>,
    units: Query<(&crate::units::UnitRenderEntity, &GlobalTransform)>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    mut ui_state: ResMut<GameplayUiState>,
    mut cursor: ResMut<GameplayCursorPresentation>,
) {
    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);

    let hover = cursor_world_ray(&windows, &camera).map_or(CommandHoverContext::None, |ray| {
        if pick_unit_along_ray(&ray, &world, &unit_catalog, &units).is_some() {
            CommandHoverContext::Unit
        } else if terrain_click_to_world_position(&ray, &world, layout, vertical_scale).is_some() {
            CommandHoverContext::Terrain
        } else {
            CommandHoverContext::None
        }
    });

    let cursor_mode = derive_cursor_mode(!selection.is_empty(), hover);
    cursor.mode = cursor_mode;
    if ui_state.snapshot.cursor_mode != cursor_mode {
        ui_state.snapshot.cursor_mode = cursor_mode;
        ui_state.hud_dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use super::super::state::{derive_cursor_mode, CommandHoverContext, GameplayCursorMode};

    #[test]
    fn cursor_state_changes_with_intent_context() {
        assert_eq!(
            derive_cursor_mode(true, CommandHoverContext::Terrain),
            GameplayCursorMode::Move
        );
        assert_eq!(
            derive_cursor_mode(true, CommandHoverContext::Unit),
            GameplayCursorMode::Move
        );
        assert_eq!(
            derive_cursor_mode(false, CommandHoverContext::Terrain),
            GameplayCursorMode::Default
        );
    }
}
