//! Package reload diff and selective invalidation (ADR-106 TF6).

use std::collections::BTreeSet;

use super::id::TerrainFieldId;
use super::store::TerrainFieldStore;
use super::tile::TerrainFieldTile;
use crate::world::ChunkCoord;

/// Changes between two authoritative field stores after package reload.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TerrainFieldPackageDiff {
    pub added_fields: Vec<TerrainFieldId>,
    pub removed_fields: Vec<TerrainFieldId>,
    pub changed_fields: Vec<TerrainFieldId>,
    pub added_tiles: Vec<(TerrainFieldId, ChunkCoord)>,
    pub removed_tiles: Vec<(TerrainFieldId, ChunkCoord)>,
    pub changed_tiles: Vec<(TerrainFieldId, ChunkCoord)>,
}

impl TerrainFieldPackageDiff {
    pub fn any_tile_changes(&self) -> bool {
        !self.added_tiles.is_empty()
            || !self.removed_tiles.is_empty()
            || !self.changed_tiles.is_empty()
    }

    pub fn affected_field_ids(&self) -> BTreeSet<TerrainFieldId> {
        let mut fields = BTreeSet::new();
        fields.extend(self.added_fields.iter().cloned());
        fields.extend(self.removed_fields.iter().cloned());
        fields.extend(self.changed_fields.iter().cloned());
        for (field, _) in &self.added_tiles {
            fields.insert(field.clone());
        }
        for (field, _) in &self.removed_tiles {
            fields.insert(field.clone());
        }
        for (field, _) in &self.changed_tiles {
            fields.insert(field.clone());
        }
        fields
    }
}

/// Compare two stores and produce a deterministic diff.
pub fn diff_terrain_field_stores(
    before: &TerrainFieldStore,
    after: &TerrainFieldStore,
) -> TerrainFieldPackageDiff {
    let before_ids: BTreeSet<_> = before.sorted_field_ids().into_iter().collect();
    let after_ids: BTreeSet<_> = after.sorted_field_ids().into_iter().collect();

    let added_fields: Vec<TerrainFieldId> = after_ids.difference(&before_ids).cloned().collect();
    let removed_fields: Vec<TerrainFieldId> = before_ids.difference(&after_ids).cloned().collect();

    let mut changed_fields = Vec::new();
    let mut added_tiles = Vec::new();
    let mut removed_tiles = Vec::new();
    let mut changed_tiles = Vec::new();

    for field_id in before_ids.intersection(&after_ids) {
        let Some(before_layer) = before.get_layer(field_id) else {
            continue;
        };
        let Some(after_layer) = after.get_layer(field_id) else {
            continue;
        };
        if before_layer.source_version != after_layer.source_version
            || before_layer.layer_revision != after_layer.layer_revision
        {
            changed_fields.push(field_id.clone());
        }
        let before_chunks: BTreeSet<_> = before_layer.sorted_chunk_coords().into_iter().collect();
        let after_chunks: BTreeSet<_> = after_layer.sorted_chunk_coords().into_iter().collect();
        for chunk in after_chunks.difference(&before_chunks) {
            added_tiles.push((field_id.clone(), *chunk));
        }
        for chunk in before_chunks.difference(&after_chunks) {
            removed_tiles.push((field_id.clone(), *chunk));
        }
        for chunk in before_chunks.intersection(&after_chunks) {
            let before_tile = before_layer.get_tile(*chunk).expect("tile");
            let after_tile = after_layer.get_tile(*chunk).expect("tile");
            if tiles_differ(before_tile, after_tile) {
                changed_tiles.push((field_id.clone(), *chunk));
            }
        }
    }

    for field_id in added_fields.iter() {
        if let Some(layer) = after.get_layer(field_id) {
            for chunk in layer.sorted_chunk_coords() {
                added_tiles.push((field_id.clone(), chunk));
            }
        }
    }
    for field_id in removed_fields.iter() {
        if let Some(layer) = before.get_layer(field_id) {
            for chunk in layer.sorted_chunk_coords() {
                removed_tiles.push((field_id.clone(), chunk));
            }
        }
    }

    TerrainFieldPackageDiff {
        added_fields,
        removed_fields,
        changed_fields,
        added_tiles,
        removed_tiles,
        changed_tiles,
    }
}

fn tiles_differ(left: &TerrainFieldTile, right: &TerrainFieldTile) -> bool {
    left.tile_revision != right.tile_revision
        || left.source_version != right.source_version
        || left.samples != right.samples
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::terrain_field::TerrainFieldTile;

    #[test]
    fn detects_added_and_changed_tiles() {
        let mut before = TerrainFieldStore::new();
        let mut after = TerrainFieldStore::new();
        let field = TerrainFieldId::new("water");
        let chunk = ChunkCoord::new(0, 0);
        before
            .replace_tile(
                field.clone(),
                TerrainFieldTile::new_constant(chunk, 10_000, "v1"),
                "v1",
            )
            .unwrap();
        after
            .replace_tile(
                field.clone(),
                TerrainFieldTile::new_constant(chunk, 20_000, "v2"),
                "v2",
            )
            .unwrap();
        let diff = diff_terrain_field_stores(&before, &after);
        assert!(diff.changed_fields.contains(&field) || !diff.changed_tiles.is_empty());
    }
}
