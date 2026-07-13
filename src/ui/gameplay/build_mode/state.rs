//! Client-local player build mode state (ADR-081 B4).

use bevy::prelude::*;

use crate::world::{BuildingCategoryId, BuildingDefinitionId, BuildingPlacementValidation};

/// Client-only build mode phases — never stored on [`WorldData`].
#[derive(Debug, Clone, PartialEq, Default)]
pub enum BuildModePhase {
    #[default]
    Inactive,
    /// Catalog visible; no armed ghost yet.
    CatalogOpen,
    /// Armed placement ghost following terrain cursor.
    GhostPlacing {
        definition_id: BuildingDefinitionId,
        rotation_quadrants: u8,
    },
}

/// Player build mode presentation and input state.
#[derive(Resource, Debug, Clone, PartialEq, Default)]
pub struct BuildModeState {
    pub phase: BuildModePhase,
    pub search_query: String,
    pub selected_category: Option<BuildingCategoryId>,
    pub search_focused: bool,
    pub last_validation: Option<BuildingPlacementValidation>,
}

impl BuildModeState {
    pub fn is_active(&self) -> bool {
        !matches!(self.phase, BuildModePhase::Inactive)
    }

    pub fn is_ghost_placing(&self) -> bool {
        matches!(self.phase, BuildModePhase::GhostPlacing { .. })
    }

    pub fn blocks_gameplay_world_intents(&self) -> bool {
        self.is_ghost_placing()
    }

    pub fn enter_catalog(&mut self) {
        self.phase = BuildModePhase::CatalogOpen;
        self.last_validation = None;
    }

    pub fn exit(&mut self) {
        self.phase = BuildModePhase::Inactive;
        self.search_focused = false;
        self.last_validation = None;
    }

    pub fn cancel_ghost(&mut self) {
        if self.is_ghost_placing() {
            self.phase = BuildModePhase::CatalogOpen;
            self.last_validation = None;
        }
    }

    pub fn arm_definition(&mut self, definition_id: BuildingDefinitionId) {
        self.phase = BuildModePhase::GhostPlacing {
            definition_id,
            rotation_quadrants: 0,
        };
        self.last_validation = None;
    }

    pub fn rotate_ghost(&mut self) {
        if let BuildModePhase::GhostPlacing {
            rotation_quadrants, ..
        } = &mut self.phase
        {
            *rotation_quadrants = rotation_quadrants.wrapping_add(1) % 4;
            self.last_validation = None;
        }
    }

    pub fn ghost_definition_id(&self) -> Option<&BuildingDefinitionId> {
        match &self.phase {
            BuildModePhase::GhostPlacing { definition_id, .. } => Some(definition_id),
            _ => None,
        }
    }

    pub fn ghost_rotation_quadrants(&self) -> u8 {
        match &self.phase {
            BuildModePhase::GhostPlacing {
                rotation_quadrants, ..
            } => *rotation_quadrants,
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn b_toggles_enter_and_exit() {
        let mut state = BuildModeState::default();
        assert!(!state.is_active());
        state.enter_catalog();
        assert!(state.is_active());
        state.exit();
        assert!(!state.is_active());
    }

    #[test]
    fn esc_cancels_ghost_to_catalog() {
        let mut state = BuildModeState::default();
        state.arm_definition(BuildingDefinitionId::new("hut"));
        assert!(state.is_ghost_placing());
        state.cancel_ghost();
        assert!(!state.is_ghost_placing());
        assert!(matches!(state.phase, BuildModePhase::CatalogOpen));
    }

    #[test]
    fn r_rotates_deterministically() {
        let mut state = BuildModeState::default();
        state.arm_definition(BuildingDefinitionId::new("hut"));
        assert_eq!(state.ghost_rotation_quadrants(), 0);
        state.rotate_ghost();
        assert_eq!(state.ghost_rotation_quadrants(), 1);
        state.rotate_ghost();
        state.rotate_ghost();
        state.rotate_ghost();
        assert_eq!(state.ghost_rotation_quadrants(), 0);
    }

    #[test]
    fn ghost_placing_blocks_gameplay_intents() {
        let mut state = BuildModeState::default();
        assert!(!state.blocks_gameplay_world_intents());
        state.arm_definition(BuildingDefinitionId::new("hut"));
        assert!(state.blocks_gameplay_world_intents());
    }
}
