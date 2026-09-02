use std::collections::HashSet;

use bevy::prelude::*;

use crate::terrain::residency::ChunkResidencyTracker;
use crate::terrain::{TerrainRenderAssets, world_position_to_render_global};
use crate::world::{SettlementAnchorId, WorldData};

use super::components::SettlementAnchorRenderEntity;
use super::spawn::{
    SettlementAnchorRenderIndex, despawn_settlement_anchor_render_entities,
    spawn_settlement_anchor_render_entity,
};

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct SettlementAnchorRuntimeSystems;

pub fn sync_settlement_anchor_render_entities(
    mut commands: Commands,
    world: Res<WorldData>,
    config: Res<crate::world::WorldConfig>,
    residency: Res<ChunkResidencyTracker>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut index: ResMut<SettlementAnchorRenderIndex>,
    existing: Query<(Entity, &SettlementAnchorRenderEntity)>,
    render_assets: Option<Res<TerrainRenderAssets>>,
) {
    let vertical_scale = render_assets
        .as_ref()
        .map(|assets| assets.vertical_scale)
        .unwrap_or(1.0);
    let layout = config.chunk_layout();

    let mut visible = HashSet::new();
    for anchor_id in world.settlement_anchor_store().sorted_anchor_ids() {
        let Some(record) = world.settlement_anchor_store().get(anchor_id) else {
            continue;
        };
        let chunk = crate::world::ChunkId::new(record.position.chunk);
        if residency.is_resident(chunk) {
            visible.insert(anchor_id);
        }
    }

    let stale: Vec<SettlementAnchorId> = index
        .0
        .keys()
        .copied()
        .filter(|id| !visible.contains(id))
        .collect();
    despawn_settlement_anchor_render_entities(&mut commands, &mut index, stale);

    for anchor_id in &visible {
        let Some(record) = world.settlement_anchor_store().get(*anchor_id) else {
            continue;
        };
        let global = world_position_to_render_global(record.position, layout, vertical_scale);
        if let Some(entity) = index.0.get(anchor_id).copied() {
            commands
                .entity(entity)
                .insert(Transform::from_translation(global));
        } else {
            spawn_settlement_anchor_render_entity(
                &mut commands,
                &mut meshes,
                &mut materials,
                &mut index,
                *anchor_id,
                global,
            );
        }
    }

    for (entity, marker) in &existing {
        if !visible.contains(&marker.anchor_id) {
            commands.entity(entity).despawn();
            index.0.remove(&marker.anchor_id);
        }
    }
}
