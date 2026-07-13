//! Client-local interior view state (ADR-083 B6).

use bevy::prelude::*;

use crate::ui::gameplay::{PlayerHudState, primary_selected_unit};
use crate::units::input::SelectedUnits;
use crate::world::{SpaceId, SpaceRegistry, WorldData};

/// Presentation-only active navigable space. Never written to [`WorldData`].
#[derive(Resource, Debug, Clone, PartialEq, Eq, Reflect)]
pub struct ActiveViewedSpace {
    pub space_id: SpaceId,
    pub visibility_group_id: u32,
    pub display_floor_label: String,
}

impl Default for ActiveViewedSpace {
    fn default() -> Self {
        Self {
            space_id: SpaceId::SURFACE,
            visibility_group_id: 0,
            display_floor_label: "Surface".to_string(),
        }
    }
}

/// When true, primary unit space changes do not update [`ActiveViewedSpace`].
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub struct ViewFollowLock {
    pub locked: bool,
}

impl ViewFollowLock {
    pub fn lock(&mut self) {
        self.locked = true;
    }

    pub fn unlock(&mut self) {
        self.locked = false;
    }
}

fn view_metadata_for_space(registry: &SpaceRegistry, space_id: SpaceId) -> (u32, String) {
    if space_id.is_surface() {
        return (0, "Surface".to_string());
    }
    registry
        .get_space(space_id)
        .map(|space| (space.visibility_group_id, space.display_floor_label.clone()))
        .unwrap_or_else(|| (0, format!("Space {}", space_id.raw())))
}

/// Follow the primary selected unit's authoritative [`SpaceId`].
pub fn sync_active_viewed_space(
    world: Res<WorldData>,
    selection: Res<SelectedUnits>,
    hud: Res<PlayerHudState>,
    lock: Res<ViewFollowLock>,
    mut active: ResMut<ActiveViewedSpace>,
) {
    if lock.locked {
        return;
    }

    let primary = hud
        .primary_selected_unit
        .or_else(|| primary_selected_unit(&selection));
    let Some(unit_id) = primary else {
        return;
    };
    let Some(record) = world.get_unit(unit_id) else {
        return;
    };

    let space_id = record.current_space_id;
    if active.space_id == space_id {
        return;
    }

    let (visibility_group_id, display_floor_label) =
        view_metadata_for_space(world.space_registry(), space_id);
    active.space_id = space_id;
    active.visibility_group_id = visibility_group_id;
    active.display_floor_label = display_floor_label;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_view_defaults_to_surface() {
        let active = ActiveViewedSpace::default();
        assert_eq!(active.space_id, SpaceId::SURFACE);
        assert_eq!(active.display_floor_label, "Surface");
    }

    #[test]
    fn lock_prevents_follow_updates() {
        assert!(!ViewFollowLock::default().locked);
        let mut lock = ViewFollowLock::default();
        lock.lock();
        assert!(lock.locked);
        lock.unlock();
        assert!(!lock.locked);
    }
}
