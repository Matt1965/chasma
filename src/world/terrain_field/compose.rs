//! Future modifier composition seam (ADR-101 TF1, ADR-106 TF6).

use super::TerrainFieldId;
use super::modifier::{TerrainFieldModifierKind, TerrainFieldModifierStore};
use crate::world::ChunkCoord;

/// Compose base field value with optional authored override and runtime modifiers.
///
/// TF6 applies modifiers in fixed order when present:
/// 1. base tile value
/// 2. runtime modifier (empty in TF6)
/// 3. clamp to `u16`
///
/// Authored override package sections are reserved for a future layer between base and runtime.
pub fn compose_terrain_field_value(
    base: u16,
    field_id: &TerrainFieldId,
    chunk: ChunkCoord,
    modifiers: &TerrainFieldModifierStore,
) -> u16 {
    let Some(entry) = modifiers.get(field_id, chunk) else {
        return base;
    };
    match entry.kind {
        TerrainFieldModifierKind::AdditiveDelta => base.saturating_add(entry.value),
        TerrainFieldModifierKind::MultiplicativeFactor => {
            let scaled = (base as u32 * entry.value as u32) / 10_000;
            scaled.min(u16::MAX as u32) as u16
        }
        TerrainFieldModifierKind::Override => entry.value,
        TerrainFieldModifierKind::Clamp => base.clamp(0, entry.value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::terrain_field::modifier::TerrainFieldModifierEntry;

    #[test]
    fn empty_modifiers_preserve_base_exactly() {
        let modifiers = TerrainFieldModifierStore::default();
        let field = TerrainFieldId::new("iron");
        assert_eq!(
            compose_terrain_field_value(12_345, &field, ChunkCoord::new(0, 0), &modifiers),
            12_345
        );
    }

    #[test]
    fn additive_modifier_applies_when_present() {
        let mut modifiers = TerrainFieldModifierStore::default();
        let field = TerrainFieldId::new("iron");
        modifiers.set(
            field.clone(),
            ChunkCoord::new(0, 0),
            TerrainFieldModifierEntry {
                kind: TerrainFieldModifierKind::AdditiveDelta,
                value: 100,
            },
        );
        assert_eq!(
            compose_terrain_field_value(1_000, &field, ChunkCoord::new(0, 0), &modifiers),
            1_100
        );
    }
}
