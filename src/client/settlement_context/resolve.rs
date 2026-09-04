//! Pure settlement focus resolution for camera-derived UI context.

use bevy::prelude::Resource;

use crate::world::{
    Affiliation, SettlementId, SettlementRecord, WorldData, WorldPosition, xz_distance,
};

/// Hysteresis and lookahead tuning for camera settlement focus.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct SettlementFocusConfig {
    /// Extra meters beyond boundary where an exterior camera can still "look at" a settlement.
    pub exterior_lookahead_meters: f32,
    /// Another settlement must beat the current one by at least this margin to steal focus.
    pub switch_distance_margin_meters: f32,
}

impl Default for SettlementFocusConfig {
    fn default() -> Self {
        Self {
            exterior_lookahead_meters: 48.0,
            switch_distance_margin_meters: 16.0,
        }
    }
}

impl SettlementFocusConfig {
    fn effective_exterior_radius(&self, settlement: &SettlementRecord) -> f32 {
        settlement.boundary_radius_meters + self.exterior_lookahead_meters
    }
}

/// Whether this settlement is eligible for player management UI context.
pub fn is_player_manageable_settlement(settlement: &SettlementRecord) -> bool {
    settlement.ownership.affiliation == Affiliation::Player
}

/// Resolve which player settlement the camera focus point belongs to.
pub fn resolve_focused_player_settlement(
    world: &WorldData,
    focus: WorldPosition,
    previous: Option<SettlementId>,
    config: &SettlementFocusConfig,
) -> Option<SettlementId> {
    let layout = world.layout();
    let mut player_settlements: Vec<&SettlementRecord> = world
        .settlement_store()
        .sorted_settlement_ids()
        .into_iter()
        .filter_map(|id| world.settlement_store().get_settlement(id))
        .filter(|settlement| is_player_manageable_settlement(settlement))
        .collect();

    if player_settlements.is_empty() {
        return None;
    }

    player_settlements.sort_by_key(|settlement| settlement.id);

    if let Some(contained) = containing_player_settlement(&player_settlements, focus, layout) {
        return Some(contained);
    }

    let mut nearest: Option<(SettlementId, f32)> = None;
    for settlement in &player_settlements {
        let distance = xz_distance(focus, settlement.center, layout);
        let effective = config.effective_exterior_radius(settlement);
        if distance <= effective {
            if nearest.map(|(_, best)| distance < best).unwrap_or(true) {
                nearest = Some((settlement.id, distance));
            }
        }
    }

    if let Some(previous_id) = previous {
        if let Some(previous_settlement) = player_settlements
            .iter()
            .find(|settlement| settlement.id == previous_id)
        {
            let previous_distance = xz_distance(focus, previous_settlement.center, layout);
            let previous_effective = config.effective_exterior_radius(previous_settlement);
            if previous_distance <= previous_effective {
                if let Some((candidate_id, candidate_distance)) = nearest {
                    if candidate_id == previous_id {
                        return Some(previous_id);
                    }
                    if candidate_distance + config.switch_distance_margin_meters < previous_distance
                    {
                        return Some(candidate_id);
                    }
                }
                return Some(previous_id);
            }
        }
    }

    nearest.map(|(id, _)| id)
}

fn containing_player_settlement(
    settlements: &[&SettlementRecord],
    focus: WorldPosition,
    layout: crate::world::ChunkLayout,
) -> Option<SettlementId> {
    settlements
        .iter()
        .filter(|settlement| {
            xz_distance(focus, settlement.center, layout) <= settlement.boundary_radius_meters
        })
        .min_by_key(|settlement| settlement.id)
        .map(|settlement| settlement.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{
        ChunkCoord, ChunkData, ChunkLayout, Heightfield, LocalPosition, SettlementKind,
        SettlementOwnership, WorldData, WorldPosition, create_settlement,
    };
    use bevy::prelude::Vec3;

    fn layout() -> ChunkLayout {
        ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        }
    }

    fn test_world() -> WorldData {
        let mut world = WorldData::new(layout());
        let heightfield = Heightfield::from_samples(65, 4.0, vec![0.0; 65 * 65]).unwrap();
        world.insert(
            crate::world::ChunkId::new(ChunkCoord::new(0, 0)),
            ChunkData::new(heightfield, Vec::new()),
        );
        world
    }

    fn pos(x: f32, z: f32) -> WorldPosition {
        WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, z)),
        )
    }

    fn spawn_player_settlement(
        world: &mut WorldData,
        x: f32,
        z: f32,
        radius: f32,
        name: &str,
    ) -> SettlementId {
        create_settlement(
            world,
            pos(x, z),
            name,
            SettlementOwnership::player_default(),
            SettlementKind::Town,
            Some(radius),
            None,
            0,
        )
        .expect("settlement")
        .settlement_id
    }

    fn spawn_npc_settlement(world: &mut WorldData, x: f32, z: f32, radius: f32) -> SettlementId {
        create_settlement(
            world,
            pos(x, z),
            "NPC Camp",
            SettlementOwnership {
                owner_id: None,
                team_id: None,
                affiliation: Affiliation::Neutral,
            },
            SettlementKind::Camp,
            Some(radius),
            None,
            0,
        )
        .expect("npc settlement")
        .settlement_id
    }

    const CFG: SettlementFocusConfig = SettlementFocusConfig {
        exterior_lookahead_meters: 48.0,
        switch_distance_margin_meters: 16.0,
    };

    #[test]
    fn focus_inside_settlement_a_while_camera_position_outside_still_selects_a() {
        let mut world = test_world();
        let settlement_a = spawn_player_settlement(&mut world, 64.0, 64.0, 48.0, "A");
        let focus_inside_a = pos(80.0, 64.0);
        let resolved = resolve_focused_player_settlement(&world, focus_inside_a, None, &CFG);
        assert_eq!(resolved, Some(settlement_a));
        let _camera_outside_a = pos(20.0, 64.0);
    }

    #[test]
    fn rotating_view_toward_settlement_b_changes_focus() {
        let mut world = test_world();
        let settlement_a = spawn_player_settlement(&mut world, 64.0, 64.0, 40.0, "A");
        let settlement_b = spawn_player_settlement(&mut world, 220.0, 64.0, 40.0, "B");
        let focus_on_a = pos(64.0, 64.0);
        assert_eq!(
            resolve_focused_player_settlement(&world, focus_on_a, None, &CFG),
            Some(settlement_a)
        );
        let focus_on_b = pos(220.0, 64.0);
        assert_eq!(
            resolve_focused_player_settlement(&world, focus_on_b, Some(settlement_a), &CFG),
            Some(settlement_b)
        );
    }

    #[test]
    fn focus_point_not_camera_position_determines_settlement() {
        let mut world = test_world();
        let settlement_a = spawn_player_settlement(&mut world, 64.0, 64.0, 48.0, "A");
        let settlement_b = spawn_player_settlement(&mut world, 220.0, 64.0, 48.0, "B");
        let focus_near_b = pos(210.0, 64.0);
        let resolved =
            resolve_focused_player_settlement(&world, focus_near_b, Some(settlement_a), &CFG);
        assert_eq!(resolved, Some(settlement_b));
    }

    #[test]
    fn neutral_settlement_is_not_player_focus() {
        let mut world = test_world();
        let _npc = spawn_npc_settlement(&mut world, 64.0, 64.0, 48.0);
        let focus = pos(64.0, 64.0);
        assert_eq!(
            resolve_focused_player_settlement(&world, focus, None, &CFG),
            None
        );
    }

    #[test]
    fn focus_far_from_player_settlements_is_none() {
        let mut world = test_world();
        let settlement_a = spawn_player_settlement(&mut world, 64.0, 64.0, 32.0, "A");
        let far_focus = pos(240.0, 240.0);
        assert_eq!(
            resolve_focused_player_settlement(&world, far_focus, Some(settlement_a), &CFG),
            None
        );
    }

    #[test]
    fn multiple_settlements_resolve_deterministically_by_id() {
        let mut world = test_world();
        let settlement_a = spawn_player_settlement(&mut world, 64.0, 64.0, 48.0, "A");
        let settlement_b = spawn_player_settlement(&mut world, 220.0, 220.0, 48.0, "B");
        assert!(settlement_a.raw() < settlement_b.raw());
        let focus = pos(64.0, 64.0);
        assert_eq!(
            resolve_focused_player_settlement(&world, focus, None, &CFG),
            Some(settlement_a)
        );
    }

    #[test]
    fn hysteresis_prevents_rapid_flip_near_boundary() {
        let mut world = test_world();
        let settlement_a = spawn_player_settlement(&mut world, 64.0, 64.0, 48.0, "A");
        let settlement_b = spawn_player_settlement(&mut world, 220.0, 64.0, 48.0, "B");
        let near_boundary = pos(112.0, 64.0);
        assert_eq!(
            resolve_focused_player_settlement(&world, near_boundary, Some(settlement_a), &CFG),
            Some(settlement_a)
        );
        let slightly_toward_b = pos(118.0, 64.0);
        assert_eq!(
            resolve_focused_player_settlement(&world, slightly_toward_b, Some(settlement_a), &CFG),
            Some(settlement_a)
        );
        let clearly_toward_b = pos(170.0, 64.0);
        assert_eq!(
            resolve_focused_player_settlement(&world, clearly_toward_b, Some(settlement_a), &CFG),
            Some(settlement_b)
        );
    }

    #[test]
    fn removed_focused_settlement_re_resolves() {
        let mut world = test_world();
        let settlement_a = spawn_player_settlement(&mut world, 64.0, 64.0, 48.0, "A");
        let settlement_b = spawn_player_settlement(&mut world, 220.0, 64.0, 48.0, "B");
        let focus_near_b = pos(210.0, 64.0);
        world.settlement_store_mut().remove_settlement(settlement_a);
        assert_eq!(
            resolve_focused_player_settlement(&world, focus_near_b, Some(settlement_a), &CFG),
            Some(settlement_b)
        );
    }

    #[test]
    fn removed_focused_settlement_at_orphan_focus_is_none() {
        let mut world = test_world();
        let settlement_a = spawn_player_settlement(&mut world, 64.0, 64.0, 48.0, "A");
        let _settlement_b = spawn_player_settlement(&mut world, 220.0, 64.0, 48.0, "B");
        let focus = pos(64.0, 64.0);
        world.settlement_store_mut().remove_settlement(settlement_a);
        assert_eq!(
            resolve_focused_player_settlement(&world, focus, Some(settlement_a), &CFG),
            None
        );
    }
}
