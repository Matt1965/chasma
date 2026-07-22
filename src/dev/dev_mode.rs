//! Dev mode state — client-local authoring UI (ADR-043, ADR-047). Not simulation truth.

use std::collections::HashSet;

use bevy::prelude::*;

use crate::debug::DebugOverlayConfig;
use crate::world::{
    BuildingDefinitionId, DoodadDefinitionId, InventoryId, InventoryProfileId, ItemDefinitionId,
    ItemPileId, UnitDefinitionId, WorldPosition,
};

use super::history::DevSpawnHistory;
use super::tools::{BrushSettings, PlacementRules};

/// Any dev-editable inventory-backed container (grid or world pile) — DV0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DevInventoryEndpoint {
    Grid(InventoryId),
    Pile(ItemPileId),
}

/// Which dev panel text field owns keyboard input (DV2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Reflect)]
pub enum DevTextFieldFocus {
    #[default]
    None,
    CatalogSearch,
    SceneName,
    ItemQuantity,
}

/// Items tab sub-views (DV0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Reflect)]
pub enum ItemsBrowserSubtab {
    #[default]
    Items,
    InventoryProfiles,
    InventoryManage,
}

/// Client-local dev inventory tool state (DV0).
#[derive(Debug, Clone, PartialEq)]
pub struct DevInventoryToolState {
    pub subtab: ItemsBrowserSubtab,
    pub quantity: u32,
    /// Editable quantity text while [`DevTextFieldFocus::ItemQuantity`] is active.
    pub quantity_input: String,
    pub selected_endpoint_index: usize,
    pub selected_entry_index: Option<usize>,
    pub transfer_source: Option<DevInventoryEndpoint>,
    pub transfer_dest: Option<DevInventoryEndpoint>,
    pub pile_placement_armed: bool,
    pub message: String,
}

impl Default for DevInventoryToolState {
    fn default() -> Self {
        Self {
            subtab: ItemsBrowserSubtab::Items,
            quantity: 10,
            quantity_input: "10".to_string(),
            selected_endpoint_index: 0,
            selected_entry_index: Some(0),
            transfer_source: None,
            transfer_dest: None,
            pile_placement_armed: false,
            message: String::new(),
        }
    }
}

/// Active dev panel tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Reflect)]
pub enum DevTab {
    #[default]
    Units,
    Doodads,
    Buildings,
    Items,
    Placement,
    Scenes,
    Inspector,
    Debug,
    WorldTools,
    TerrainFields,
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
    Item(ItemDefinitionId),
    InventoryProfile(InventoryProfileId),
}

impl DefinitionId {
    pub fn id_str(&self) -> &str {
        match self {
            DefinitionId::Unit(id) => id.as_str(),
            DefinitionId::Doodad(id) => id.as_str(),
            DefinitionId::Building(id) => id.as_str(),
            DefinitionId::Item(id) => id.as_str(),
            DefinitionId::InventoryProfile(id) => id.as_str(),
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
    /// Set when a catalog Place tool is armed; consumed by gizmo sync to clear world selection once.
    pub clear_world_selection_for_place: bool,
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
    /// Item / inventory dev tools (DV0).
    pub inventory: DevInventoryToolState,
    /// Legacy harness message (pile tools tab).
    pub pile_harness_message: String,
    /// Settlement treasury dev harness status (ADR-093 I7).
    pub treasury_harness_message: String,
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
            clear_world_selection_for_place: false,
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
            pile_harness_message: String::new(),
            treasury_harness_message: String::new(),
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
            inventory: DevInventoryToolState::default(),
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
        if self.text_focus == DevTextFieldFocus::ItemQuantity {
            self.apply_item_quantity_input();
        }
        self.text_focus = DevTextFieldFocus::None;
    }

    pub fn focus_catalog_search(&mut self) {
        self.text_focus = DevTextFieldFocus::CatalogSearch;
    }

    pub fn focus_scene_name(&mut self) {
        self.text_focus = DevTextFieldFocus::SceneName;
    }

    pub fn focus_item_quantity(&mut self) {
        self.inventory.quantity_input = self.inventory.quantity.to_string();
        self.text_focus = DevTextFieldFocus::ItemQuantity;
    }

    pub fn apply_item_quantity_input(&mut self) {
        let parsed = self
            .inventory
            .quantity_input
            .parse::<u32>()
            .unwrap_or(self.inventory.quantity)
            .clamp(1, 10_000);
        self.inventory.quantity = parsed;
        self.inventory.quantity_input = parsed.to_string();
    }

    pub fn bump_item_quantity(&mut self, delta: i32) {
        let current = if self.text_focus == DevTextFieldFocus::ItemQuantity {
            self.inventory
                .quantity_input
                .parse()
                .unwrap_or(self.inventory.quantity)
        } else {
            self.inventory.quantity
        };
        let next = (current as i32 + delta).clamp(1, 10_000) as u32;
        self.inventory.quantity = next;
        self.inventory.quantity_input = next.to_string();
    }

    pub fn set_item_quantity_to_max_stack(&mut self, items: &crate::world::ItemCatalog) {
        let max = selected_item_max_stack(self.selected_definition.as_ref(), items).unwrap_or(1);
        self.inventory.quantity = max.max(1);
        self.inventory.quantity_input = self.inventory.quantity.to_string();
    }

    /// Whether a placement definition is armed for terrain clicks.
    pub fn placement_tool_active(&self) -> bool {
        matches!(
            self.selected_definition,
            Some(
                DefinitionId::Unit(_)
                    | DefinitionId::Doodad(_)
                    | DefinitionId::Building(_)
            )
        )
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
            Some(DefinitionId::Item(_)) => "Item selected",
            Some(DefinitionId::InventoryProfile(_)) => "Profile selected",
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
        match &id {
            DefinitionId::Unit(_) => self.spawn_mode = SpawnMode::Unit,
            DefinitionId::Doodad(_) => self.spawn_mode = SpawnMode::Doodad,
            DefinitionId::Building(_) => self.spawn_mode = SpawnMode::Building,
            DefinitionId::Item(_) | DefinitionId::InventoryProfile(_) => {}
        }
        if matches!(
            id,
            DefinitionId::Unit(_) | DefinitionId::Doodad(_) | DefinitionId::Building(_)
        ) {
            self.clear_world_selection_for_place = true;
        }
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

pub fn selected_item_max_stack(
    selected: Option<&DefinitionId>,
    items: &crate::world::ItemCatalog,
) -> Option<u32> {
    let DefinitionId::Item(item_id) = selected? else {
        return None;
    };
    items.get(item_id).map(|item| item.max_stack)
}

/// When true, gameplay mouse input is skipped for the current frame.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DevModeInputGate {
    pub block_gameplay_mouse: bool,
    pub spawn_handled_this_frame: bool,
    /// Suppress RTS camera pan/orbit/zoom during gizmo drag (ADR-099).
    pub block_camera_input: bool,
}

impl DevModeInputGate {
    pub fn reset(&mut self) {
        self.block_gameplay_mouse = false;
        self.spawn_handled_this_frame = false;
        self.block_camera_input = false;
    }

    pub fn should_block(gate: &DevModeInputGate) -> bool {
        gate.block_gameplay_mouse
    }
}
