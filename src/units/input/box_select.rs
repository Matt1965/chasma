//! Screen-space drag (marquee) selection (ADR-034 U9).

use std::collections::HashSet;

use bevy::camera::Camera;
use bevy::prelude::*;

use crate::units::UnitRenderEntity;
use crate::world::{SelectionControllabilityPolicy, UnitId, WorldData, unit_is_selectable};

use super::picking::world_position_to_screen;

/// Minimum cursor travel before a left press becomes a box drag (pixels).
pub const BOX_SELECT_DRAG_THRESHOLD_PX: f32 = 4.0;

/// In-progress left-button drag for marquee selection.
#[derive(Debug, Resource, Default, Clone, Copy, PartialEq)]
pub struct BoxSelectDrag {
    pub active: bool,
    pub start: Vec2,
    pub current: Vec2,
}

impl BoxSelectDrag {
    pub fn begin(&mut self, screen: Vec2) {
        self.active = true;
        self.start = screen;
        self.current = screen;
    }

    pub fn update(&mut self, screen: Vec2) {
        if self.active {
            self.current = screen;
        }
    }

    pub fn reset(&mut self) {
        self.active = false;
    }

    pub fn is_box_drag(&self) -> bool {
        self.active && (self.current - self.start).length() >= BOX_SELECT_DRAG_THRESHOLD_PX
    }
}

/// Normalized axis-aligned screen rectangle from two corner points.
pub fn normalized_screen_rect(a: Vec2, b: Vec2) -> (Vec2, Vec2) {
    (a.min(b), a.max(b))
}

pub fn screen_point_in_rect(min: Vec2, max: Vec2, point: Vec2) -> bool {
    point.x >= min.x && point.x <= max.x && point.y >= min.y && point.y <= max.y
}

/// Pure selection test used by [`collect_units_in_screen_rect`] and unit tests.
pub fn unit_ids_in_screen_rect(
    rect_min: Vec2,
    rect_max: Vec2,
    projected_units: &[(UnitId, Vec2)],
) -> HashSet<UnitId> {
    projected_units
        .iter()
        .filter(|(_, screen)| screen_point_in_rect(rect_min, rect_max, *screen))
        .map(|(id, _)| *id)
        .collect()
}

/// Select units whose render positions project inside the screen rectangle.
///
/// Iterates visible render entities only — no full [`WorldData`] scan.
pub fn collect_units_in_screen_rect(
    rect_min: Vec2,
    rect_max: Vec2,
    camera: &Camera,
    camera_transform: &GlobalTransform,
    world: &WorldData,
    units: &Query<(&UnitRenderEntity, &GlobalTransform)>,
    policy: SelectionControllabilityPolicy,
) -> HashSet<UnitId> {
    let mut projected = Vec::new();
    projected.reserve(units.iter().len());

    for (marker, transform) in units {
        let Some(record) = world.get_unit(marker.unit_id) else {
            continue;
        };
        if !unit_is_selectable(record, policy) {
            continue;
        }
        let Some(screen) =
            world_position_to_screen(transform.translation(), camera, camera_transform)
        else {
            continue;
        };
        projected.push((marker.unit_id, screen));
    }

    unit_ids_in_screen_rect(rect_min, rect_max, &projected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_select_respects_screen_projection_bounds() {
        let min = Vec2::new(10.0, 10.0);
        let max = Vec2::new(100.0, 100.0);
        let units = [
            (UnitId::new(1), Vec2::new(50.0, 50.0)),
            (UnitId::new(2), Vec2::new(5.0, 50.0)),
            (UnitId::new(3), Vec2::new(150.0, 50.0)),
            (UnitId::new(4), Vec2::new(50.0, 50.0)), // duplicate id — set dedupes
        ];
        let selected = unit_ids_in_screen_rect(min, max, &units);
        assert_eq!(selected.len(), 2);
        assert!(selected.contains(&UnitId::new(1)));
        assert!(selected.contains(&UnitId::new(4)));
        assert!(!selected.contains(&UnitId::new(2)));
        assert!(!selected.contains(&UnitId::new(3)));
    }

    #[test]
    fn normalized_screen_rect_orders_corners() {
        let (min, max) = normalized_screen_rect(Vec2::new(80.0, 20.0), Vec2::new(10.0, 90.0));
        assert_eq!(min, Vec2::new(10.0, 20.0));
        assert_eq!(max, Vec2::new(80.0, 90.0));
    }

    #[test]
    fn drag_threshold_detects_box_vs_click() {
        let mut drag = BoxSelectDrag::default();
        drag.begin(Vec2::ZERO);
        drag.update(Vec2::new(2.0, 0.0));
        assert!(!drag.is_box_drag());
        drag.update(Vec2::new(5.0, 0.0));
        assert!(drag.is_box_drag());
    }
}
