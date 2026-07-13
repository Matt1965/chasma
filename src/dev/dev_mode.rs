//! Dev mode state — client-local authoring UI (ADR-043, ADR-047). Not simulation truth.

use std::collections::HashSet;

use bevy::prelude::*;

use crate::debug::DebugOverlayConfig;
use crate::world::{BuildingDefinitionId, DoodadDefinitionId, UnitDefinitionId, WorldPosition};

use super::history::DevSpawnHistory;
use super::tools::{BrushSettings, PlacementRules};

/// Which dev panel text field owns keyboard input (DV2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Reflect)]
pub enum DevTextFieldFocus {
    #[default]
    None,
    CatalogSearch,
    SceneName,
}

/// Active dev panel tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Reflect)]
pub enum DevTab {
    #[default]
    Units,
    Doodads,
    Buildings,
    Placement,
    Scenes,
    Inspector,
    Debug,
    WorldTools,
}

/// What the spawn tool will place at the next world click.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Reflect)]
pub enum SpawnMode {
    #[default]
    Unit,
    Doodad,
    Building,
}

/// Unified catalog selection for spawn tools.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DefinitionId {
    Unit(UnitDefinitionId),
    Doodad(DoodadDefinitionId),
    Building(BuildingDefinitionId),
}

impl DefinitionId {
    pub fn id_str(&self) -> &str {
        match self {
            DefinitionId::Unit(id) => id.as_str(),
            DefinitionId::Doodad(id) => id.as_str(),
            DefinitionId::Building(id) => id.as_str(),
        }
    }
}

/// Legacy alias — debug toggles live on [`DebugOverlayConfig`] (ADR-047).
pub type DevDebugFlags = DebugOverlayConfig;

/// Runtime dev authoring UI state (F12). Not part of [`WorldData`].
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct DevModeState {
    pub enabled: bool,
    pub active_tab: DevTab,
    /// Raw search text (persisted); filtered via [`super::catalog_cache::DevSearchDebounce`].
    pub search_query: String,
    pub selected_definition: Option<DefinitionId>,
    pub spawn_mode: SpawnMode,
    pub enabled_only: bool,
    pub debug_config: DebugOverlayConfig,
    pub brush: BrushSettings,
    pub terrain_conforming: bool,
    pub show_preview: bool,
    pub placement_rules: PlacementRules,
    pub last_line_direction: Vec2,
    pub list_scroll: usize,
    pub last_spawn_message: String,
    pub scene_name_input: String,
    pub selected_scene_id: Option<String>,
    pub last_loaded_scene_id: Option<String>,
    pub last_scene_message: String,
    pub scene_list_scroll: usize,
    pub favorites: HashSet<DefinitionId>,
    pub favorite_slots: [Option<DefinitionId>; 9],
    pub spawn_history: DevSpawnHistory,
    pub last_spawn: Option<(DefinitionId, WorldPosition)>,
    /// Affiliation assigned to the next dev unit spawn (O1).
    pub spawn_affiliation: crate::world::Affiliation,
    /// Active text-field focus — global dev shortcuts are suppressed while set (DV2).
    pub text_focus: DevTextFieldFocus,
}

impl Default for DevModeState {
    fn default() -> Self {
        Self {
            enabled: false,
            active_tab: DevTab::Units,
            search_query: String::new(),
            selected_definition: None,
            spawn_mode: SpawnMode::Unit,
            enabled_only: true,
            debug_config: DebugOverlayConfig::production(),
            brush: BrushSettings::default(),
            terrain_conforming: true,
            show_preview: true,
            placement_rules: PlacementRules::default(),
            last_line_direction: Vec2::X,
            list_scroll: 0,
            last_spawn_message: String::new(),
            scene_name_input: "Untitled Scene".to_string(),
            selected_scene_id: None,
            last_loaded_scene_id: None,
            last_scene_message: String::new(),
            scene_list_scroll: 0,
            favorites: HashSet::new(),
            favorite_slots: std::array::from_fn(|_| None),
            spawn_history: DevSpawnHistory::default(),
            last_spawn: None,
            spawn_affiliation: crate::world::Affiliation::Player,
            text_focus: DevTextFieldFocus::None,
        }
    }
}

impl DevModeState {
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
        if !self.enabled {
            self.last_spawn_message.clear();
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected_definition = None;
    }

    pub fn has_text_focus(&self) -> bool {
        self.text_focus != DevTextFieldFocus::None
    }

    pub fn clear_text_focus(&mut self) {
        self.text_focus = DevTextFieldFocus::None;
    }

    pub fn focus_catalog_search(&mut self) {
        self.text_focus = DevTextFieldFocus::CatalogSearch;
    }

    pub fn focus_scene_name(&mut self) {
        self.text_focus = DevTextFieldFocus::SceneName;
    }

    /// Whether a placement definition is armed for terrain clicks.
    pub fn placement_tool_active(&self) -> bool {
        self.selected_definition.is_some()
    }

    /// Clear armed placement selection (preview cleared separately).
    pub fn cancel_placement_tool(&mut self) {
        self.clear_selection();
        self.last_spawn_message.clear();
    }

    /// Multi-line active-tool summary for the dev panel (DV2).
    pub fn tool_status_text(&self) -> String {
        let tool = match &self.selected_definition {
            None => "none",
            Some(DefinitionId::Unit(_)) => "Place Unit",
            Some(DefinitionId::Doodad(_)) => "Place Doodad",
            Some(DefinitionId::Building(_)) => "Place Building",
        };
        let selection = self
            .selected_definition
            .as_ref()
            .map(DefinitionId::id_str)
            .unwrap_or("none");
        format!(
            "Tool: {tool}\nSelection: {selection}\nTeam: {}\nBrush: {}",
            self.spawn_team_label(),
            self.brush.mode.label(),
        )
    }

    pub fn select_definition(&mut self, id: DefinitionId) {
        self.spawn_mode = match &id {
            DefinitionId::Unit(_) => SpawnMode::Unit,
            DefinitionId::Doodad(_) => SpawnMode::Doodad,
            DefinitionId::Building(_) => SpawnMode::Building,
        };
        self.selected_definition = Some(id);
    }

    pub fn cycle_spawn_affiliation(&mut self) {
        self.spawn_affiliation = match self.spawn_affiliation {
            crate::world::Affiliation::Player => crate::world::Affiliation::Wildlife,
            crate::world::Affiliation::Wildlife => crate::world::Affiliation::Player,
            _ => crate::world::Affiliation::Player,
        };
    }

    pub fn spawn_team_label(&self) -> &'static str {
        match self.spawn_affiliation {
            crate::world::Affiliation::Player => "Player",
            crate::world::Affiliation::Wildlife => "Wilds",
            other => other.label(),
        }
    }

    pub fn toggle_favorite(&mut self, id: DefinitionId) {
        if self.favorites.contains(&id) {
            self.favorites.remove(&id);
        } else {
            self.favorites.insert(id);
        }
    }

    pub fn assign_favorite_slot(&mut self, slot: usize, id: DefinitionId) {
        if slot < self.favorite_slots.len() {
            self.favorite_slots[slot] = Some(id);
        }
    }

    pub fn favorite_slot(&self, slot: usize) -> Option<&DefinitionId> {
        self.favorite_slots.get(slot).and_then(|slot| slot.as_ref())
    }

    /// Reset dev tool UI state (explicit user action or full reload).
    pub fn reset_tool_state(&mut self) {
        let enabled = self.enabled;
        *self = Self::default();
        self.enabled = enabled;
    }
}

/// When true, gameplay mouse input is skipped for the current frame.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DevModeInputGate {
    pub block_gameplay_mouse: bool,
    pub spawn_handled_this_frame: bool,
}

impl DevModeInputGate {
    pub fn reset(&mut self) {
        self.block_gameplay_mouse = false;
        self.spawn_handled_this_frame = false;
    }

    pub fn should_block(gate: &DevModeInputGate) -> bool {
        gate.block_gameplay_mouse
    }
}
