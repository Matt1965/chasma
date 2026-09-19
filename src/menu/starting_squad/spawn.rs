//! Convert a starting squad draft into authoritative gameplay units (CG8).

use bevy::prelude::*;

use crate::world::{
    Affiliation, AppearanceProfileCatalog, ChunkCoord, LocalPosition, UnitCatalog, UnitOwnership,
    UnitSource, WorldData, WorldPosition, create_unit_with_ownership_and_appearance,
};

use super::draft::StartingSquadDraft;

const SPAWN_SPACING: f32 = 2.0;
const SPAWN_BASE_X: f32 = 32.0;
const SPAWN_BASE_Z: f32 = 32.0;

/// Spawn configured player units from the active draft at a simple formation anchor.
pub fn spawn_starting_squad_from_draft(
    world: &mut WorldData,
    unit_catalog: &UnitCatalog,
    appearance_profiles: &AppearanceProfileCatalog,
    draft: &StartingSquadDraft,
) -> Result<Vec<crate::world::UnitId>, String> {
    let count = draft.members.len();
    if count == 0 {
        return Err("starting squad draft has no members".to_string());
    }
    let center = (count.saturating_sub(1) as f32) * 0.5;
    let ownership = UnitOwnership::with_affiliation(Affiliation::Player);
    let mut spawned = Vec::with_capacity(count);
    for (index, member) in draft.members.iter().enumerate() {
        let x = SPAWN_BASE_X + (index as f32 - center) * SPAWN_SPACING;
        let position = WorldPosition::new(
            ChunkCoord::new(0, 0),
            LocalPosition::new(Vec3::new(x, 0.0, SPAWN_BASE_Z)),
        );
        let record = create_unit_with_ownership_and_appearance(
            unit_catalog,
            appearance_profiles,
            world,
            &member.definition_id,
            position,
            UnitSource::Authored,
            ownership.clone(),
            member.appearance.appearance.clone(),
        )
        .map_err(|error| format!("{error:?}"))?;
        spawned.push(record.id);
    }
    Ok(spawned)
}
