//! Client-local player HUD state (P-UI1) — separate from simulation truth.

use bevy::prelude::*;

use crate::client::CommandType;
use crate::units::input::SelectedUnits;
use crate::world::UnitId;

/// Which units the squad panel lists (future Kenshi-style squad management hook).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Reflect)]
pub enum SquadFilterMode {
    /// Show the current selection only.
    #[default]
    SelectedOnly,
    /// Show all player-visible units when selection is empty.
    AvailableUnits,
}

/// Player-facing HUD presentation state — never written to [`WorldData`].
#[derive(Resource, Debug, Clone, PartialEq, Default)]
pub struct PlayerHudState {
    pub visible: bool,
    /// Primary inspect target — synced from selection each frame.
    pub primary_selected_unit: Option<UnitId>,
    pub hovered_command: Option<CommandType>,
    pub squad_filter_mode: SquadFilterMode,
    /// Move command armed via command panel (UI-only until terrain target).
    pub armed_command: Option<CommandType>,
}

impl PlayerHudState {
    pub fn new_visible() -> Self {
        Self {
            visible: true,
            ..Default::default()
        }
    }
}

/// Primary selected unit rule (P-UI1):
///
/// When one unit is selected, that unit is primary.
/// When multiple are selected, the **lowest [`UnitId`] by raw value** is primary
/// (deterministic; matches legacy `leader_unit_id` in [`super::state`]).
pub fn primary_selected_unit(selection: &SelectedUnits) -> Option<UnitId> {
    selection.iter().min_by_key(|id| id.raw())
}

/// Sync [`PlayerHudState::primary_selected_unit`] from the current selection.
pub fn sync_primary_selection(hud: &mut PlayerHudState, selection: &SelectedUnits) {
    hud.primary_selected_unit = primary_selected_unit(selection);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hud_state_defaults() {
        let hud = PlayerHudState::default();
        assert!(!hud.visible);
        assert!(hud.primary_selected_unit.is_none());
        assert!(hud.hovered_command.is_none());
        assert_eq!(hud.squad_filter_mode, SquadFilterMode::SelectedOnly);
        assert!(hud.armed_command.is_none());
    }

    #[test]
    fn primary_selected_unit_is_lowest_unit_id() {
        let mut selection = SelectedUnits::default();
        selection.replace_with([UnitId::new(9), UnitId::new(2), UnitId::new(7)]);
        assert_eq!(primary_selected_unit(&selection), Some(UnitId::new(2)));
    }

    #[test]
    fn single_selection_is_primary() {
        let mut selection = SelectedUnits::default();
        selection.set_single(UnitId::new(42));
        assert_eq!(primary_selected_unit(&selection), Some(UnitId::new(42)));
    }
}
