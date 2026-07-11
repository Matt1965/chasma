//! Read-only capture of interaction debug state from dispatch history (REVIEW-A6).

use bevy::prelude::*;

use crate::client::ClientIntent;
use crate::client::commands::CommandTarget;
use crate::debug::settings::{
    DebugOverlayCategory, DebugOverlaySettings, debug_interaction_overlay_enabled,
};
use crate::debug::trace::IntentDispatchHistory;
use crate::world::{
    DoodadCatalog, InteractionQueryContext, UnitCatalog, WeaponCatalog, WorldData, WorldPosition,
};

use super::interaction_snapshot::{InteractionDebugSnapshot, capture_interaction_at_position};

/// Populate [`InteractionDebugSnapshot`] from the last dispatched click (no presentation writes).
pub fn capture_interaction_debug_snapshot(
    settings: Res<DebugOverlaySettings>,
    history: Res<IntentDispatchHistory>,
    world: Res<WorldData>,
    doodad_catalog: Res<DoodadCatalog>,
    unit_catalog: Res<UnitCatalog>,
    weapon_catalog: Res<WeaponCatalog>,
    mut snapshot: ResMut<InteractionDebugSnapshot>,
) {
    if !settings.category_enabled(DebugOverlayCategory::Interaction) {
        snapshot.clear();
        return;
    }

    let Some(position) = last_command_target_position(&history, &world) else {
        snapshot.clear();
        return;
    };

    let ctx = InteractionQueryContext::new(&world, &doodad_catalog, &unit_catalog, &weapon_catalog);
    capture_interaction_at_position(&mut snapshot, &ctx, position);
}

fn last_command_target_position(
    history: &IntentDispatchHistory,
    world: &WorldData,
) -> Option<WorldPosition> {
    let report = history.report.as_ref()?;
    for record in report.records.iter().rev() {
        match &record.intent {
            ClientIntent::ContextualCommand {
                target: CommandTarget::Terrain { position },
            }
            | ClientIntent::MoveCommand { target: position } => return Some(*position),
            ClientIntent::ContextualCommand {
                target: CommandTarget::Unit { unit_id },
            } => {
                return world
                    .get_unit(*unit_id)
                    .map(|record| record.placement.position);
            }
            _ => {}
        }
    }
    None
}

/// Run condition: interaction overlay category is enabled.
pub fn run_capture_interaction_debug_snapshot(settings: Res<DebugOverlaySettings>) -> bool {
    debug_interaction_overlay_enabled(&settings)
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use crate::debug::interaction_snapshot::{
        InteractionDebugSnapshot, capture_interaction_at_position,
    };
    use crate::debug::settings::{DebugOverlayCategory, DebugOverlayConfig};
    use crate::world::{
        ChunkCoord, ChunkData, ChunkId, ChunkLayout, DoodadCatalog, Heightfield,
        InteractionQueryContext, LocalPosition, UnitCatalog, WeaponCatalog, WorldData,
        WorldPosition,
    };

    fn flat_world() -> WorldData {
        let layout = ChunkLayout {
            chunk_size_meters: 256.0,
            units_per_meter: 1.0,
        };
        let mut world = WorldData::new(layout);
        let heightfield = Heightfield::from_samples(3, 128.0, vec![0.0; 9]).unwrap();
        world.insert(
            ChunkId::new(ChunkCoord::new(0, 0)),
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

    #[test]
    fn capture_populates_client_local_snapshot() {
        let world = flat_world();
        let doodads = DoodadCatalog::default();
        let units = UnitCatalog::default();
        let weapons = WeaponCatalog::default();
        let ctx = InteractionQueryContext::new(&world, &doodads, &units, &weapons);
        let mut snapshot = InteractionDebugSnapshot::default();
        capture_interaction_at_position(&mut snapshot, &ctx, pos(1.0, 2.0));
        assert!(snapshot.query.is_some());
    }

    #[test]
    fn disabled_category_clears_snapshot_on_capture_early_exit() {
        let config = DebugOverlayConfig::production();
        assert!(!config.category_enabled(DebugOverlayCategory::Interaction));
        let mut snapshot = InteractionDebugSnapshot {
            query: Some(crate::world::InteractionResult {
                interaction_type: crate::world::InteractionType::TerrainPoint,
                position: pos(0.0, 0.0),
                metadata: crate::world::InteractionMetadata::default(),
                valid: true,
                target: crate::world::InteractionTargetRef::Terrain(pos(0.0, 0.0)),
            }),
            resolved_order: None,
        };
        if !config.category_enabled(DebugOverlayCategory::Interaction) {
            snapshot.clear();
        }
        assert!(snapshot.query.is_none());
    }
}
