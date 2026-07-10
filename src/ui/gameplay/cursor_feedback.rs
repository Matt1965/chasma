//! Contextual cursor mode from selection and hover (ADR-040 U-UI4, ADR-061 C8).

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::RtsCamera;
use crate::terrain::TerrainRenderAssets;
use crate::units::input::{
    cursor_world_ray, pick_unit_along_ray, terrain_click_to_world_position, SelectedUnits,
};
use crate::world::{
    is_valid_attack_target, AttackTargetingPolicy, UnitCatalog, WeaponCatalog, WorldConfig,
    WorldData,
};

use super::player_hud_state::PlayerHudState;
use super::state::{derive_cursor_mode, CommandHoverContext, GameplayCursorMode, GameplayUiState};

/// Last published gameplay cursor mode (for tests and future OS cursor sync).
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GameplayCursorPresentation {
    pub mode: GameplayCursorMode,
}

/// Unit under the gameplay cursor (read-only hover target for health bars).
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GameplayHoveredUnit {
    pub unit_id: Option<crate::world::UnitId>,
}

/// Sample hover context after intent collection; update cursor mode (read-only).
pub fn sample_gameplay_cursor_context(
    selection: Res<SelectedUnits>,
    hud: Res<PlayerHudState>,
    camera: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    world: Res<WorldData>,
    config: Res<WorldConfig>,
    unit_catalog: Res<UnitCatalog>,
    weapon_catalog: Res<WeaponCatalog>,
    units: Query<(&crate::units::UnitRenderEntity, &GlobalTransform)>,
    render_assets: Option<Res<TerrainRenderAssets>>,
    mut ui_state: ResMut<GameplayUiState>,
    mut cursor: ResMut<GameplayCursorPresentation>,
    mut hovered: ResMut<GameplayHoveredUnit>,
) {
    let layout = config.chunk_layout();
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let policy = AttackTargetingPolicy::default();

    let (hover, hover_attackable, hovered_unit) =
        cursor_world_ray(&windows, &camera).map_or((CommandHoverContext::None, false, None), |ray| {
            if let Some(unit_id) = pick_unit_along_ray(
                &ray,
                &world,
                &unit_catalog,
                &units,
                crate::world::SelectionControllabilityPolicy::gameplay_default(),
            ) {
                let attackable = selection.iter().any(|attacker| {
                    is_valid_attack_target(
                        &world,
                        attacker,
                        unit_id,
                        &weapon_catalog,
                        &unit_catalog,
                        policy,
                    )
                });
                (CommandHoverContext::Unit, attackable, Some(unit_id))
            } else if terrain_click_to_world_position(&ray, &world, layout, vertical_scale).is_some()
            {
                (CommandHoverContext::Terrain, false, None)
            } else {
                (CommandHoverContext::None, false, None)
            }
        });

    hovered.unit_id = hovered_unit;

    let cursor_mode = derive_cursor_mode(
        !selection.is_empty(),
        hover,
        hud.armed_command,
        hover_attackable,
    );
    cursor.mode = cursor_mode;
    if ui_state.snapshot.cursor_mode != cursor_mode {
        ui_state.snapshot.cursor_mode = cursor_mode;
        ui_state.hud_dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use super::super::state::{derive_cursor_mode, CommandHoverContext, GameplayCursorMode};
    use crate::client::CommandType;

    #[test]
    fn cursor_state_changes_with_intent_context() {
        assert_eq!(
            derive_cursor_mode(true, CommandHoverContext::Terrain, None, false),
            GameplayCursorMode::Move
        );
        assert_eq!(
            derive_cursor_mode(
                true,
                CommandHoverContext::Unit,
                Some(CommandType::Attack),
                true
            ),
            GameplayCursorMode::Attack
        );
        assert_eq!(
            derive_cursor_mode(false, CommandHoverContext::Terrain, None, false),
            GameplayCursorMode::Default
        );
    }
}
