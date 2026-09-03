//! Building menu open/close rules (BP1).

use crate::player::LocalPlayerOwnership;
use crate::world::{BuildingId, BuildingRecord, WorldData};

use super::state::BuildingPanelState;

/// Whether the local human player owns this building instance.
pub fn building_owned_by_local_player(
    building: &BuildingRecord,
    player: &LocalPlayerOwnership,
) -> bool {
    building.ownership.owner_id == Some(player.owner_id)
}

/// Open the full Building Menu when the building exists and is player-owned.
pub fn try_open_building_menu(
    panel: &mut BuildingPanelState,
    building_id: BuildingId,
    world: &WorldData,
    player: &LocalPlayerOwnership,
) -> bool {
    let Some(building) = world.get_building(building_id) else {
        return false;
    };
    if !building_owned_by_local_player(building, player) {
        return false;
    }
    panel.open(building_id);
    true
}

/// After gameplay selects a building, open its menu only when player-owned.
pub fn on_gameplay_building_selected(
    building_id: BuildingId,
    panel: &mut BuildingPanelState,
    world: &WorldData,
    player: &LocalPlayerOwnership,
) {
    let _ = try_open_building_menu(panel, building_id, world, player);
}

/// Close the menu when its target is gone or no longer player-owned.
pub fn reconcile_building_panel(
    panel: &mut BuildingPanelState,
    world: &WorldData,
    player: &LocalPlayerOwnership,
) {
    let Some(id) = panel.open_building_id else {
        return;
    };
    let should_close = world
        .get_building(id)
        .is_none_or(|building| !building_owned_by_local_player(building, player));
    if should_close {
        panel.close();
    }
}
