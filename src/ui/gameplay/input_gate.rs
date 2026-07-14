//! Player HUD pointer capture — blocks world selection when cursor is over HUD (P-UI1).

use bevy::prelude::*;

use super::layout::PlayerHudUi;

/// Whether the player HUD is under the cursor (blocks gameplay mouse intents).
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerHudHoverState {
    pub hovered: bool,
    /// Set by dev mode when the F12 panel captures input (ADR-047).
    pub dev_panel_blocks: bool,
}

/// Track HUD hover from UI interaction states.
pub fn update_player_hud_hover_state(
    interactions: Query<&Interaction, With<PlayerHudUi>>,
    mut hover: ResMut<PlayerHudHoverState>,
) {
    hover.hovered = interactions.iter().any(|state| *state != Interaction::None);
}

/// Whether gameplay mouse intents should be suppressed this frame.
pub fn gameplay_input_blocked_by_hud(hover: &PlayerHudHoverState) -> bool {
    hover.hovered || hover.dev_panel_blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_state_defaults_to_not_blocking() {
        let hover = PlayerHudHoverState::default();
        assert!(!gameplay_input_blocked_by_hud(&hover));
    }

    #[test]
    fn hover_state_blocks_when_hovered() {
        let hover = PlayerHudHoverState {
            hovered: true,
            dev_panel_blocks: false,
        };
        assert!(gameplay_input_blocked_by_hud(&hover));
    }
}
